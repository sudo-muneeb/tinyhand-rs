use crate::config::{Config, STYLES};
use crate::ffi::*;
use crate::ink::{Ink, Point};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const COLORS: [(f64, f64, f64); 8] = [
    (0.59, 0.35, 0.91),
    (1.0, 0.28, 0.3),
    (1.0, 0.55, 0.0),
    (0.15, 0.77, 0.35),
    (0.04, 0.52, 1.0),
    (0.55, 0.33, 0.95),
    (1.0, 1.0, 1.0),
    (0.1, 0.1, 0.12),
];

pub struct Renderer {
    pub surface: *mut Surface,
    hand: *mut Surface,
    tip: Point,
    pub width: i32,
    pub height: i32,
    pub bar_x: f64,
    pub bar_y: f64,
    pub monitor_h: f64,
    pub ui_scale: f64,
    requested_scale: f64,
    asset_dir: PathBuf,
    loaded: usize,
}

impl Renderer {
    /// # Safety
    /// `display`, `window`, and `visual` must belong to a live X11 connection and
    /// remain valid until the renderer is dropped. Access only from its X11 thread.
    pub unsafe fn new(
        display: *mut Display,
        window: Window,
        visual: *mut Visual,
        width: i32,
        height: i32,
        asset_dir: PathBuf,
        style: usize,
    ) -> Result<Self, String> {
        let surface = cairo_xlib_surface_create(display, window, visual, width, height);
        if cairo_surface_status(surface) != 0 {
            return Err("Cairo could not create X11 surface".into());
        }
        let mut this = Self {
            surface,
            hand: std::ptr::null_mut(),
            tip: Point { x: 0.0, y: 0.0 },
            width,
            height,
            bar_x: width as f64 / 2.0 - 244.0,
            bar_y: height as f64 - 72.0,
            monitor_h: height as f64,
            ui_scale: 1.0,
            requested_scale: std::env::var("GDK_SCALE")
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
                .or_else(|| {
                    std::env::var("QT_SCALE_FACTOR")
                        .ok()
                        .and_then(|s| s.parse::<f64>().ok())
                })
                .unwrap_or(1.0)
                .clamp(1.0, 3.0),
            asset_dir,
            loaded: 36,
        };
        this.select(style)?;
        Ok(this)
    }

    /// # Safety
    /// The renderer's Cairo surface must be valid and used on its X11 thread.
    pub unsafe fn select(&mut self, style: usize) -> Result<(), String> {
        if style == self.loaded {
            return Ok(());
        }
        if !self.hand.is_null() {
            cairo_surface_destroy(self.hand);
            self.hand = std::ptr::null_mut();
        }
        if style < 36 {
            let path = self.asset_dir.join(format!("hand_{}.png", STYLES[style]));
            let c = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
            self.hand = cairo_image_surface_create_from_png(c.as_ptr());
            if cairo_surface_status(self.hand) != 0 {
                return Err(format!("Could not load {}", path.display()));
            }
            self.tip = measure_tip(self.hand);
        }
        self.loaded = style;
        Ok(())
    }

    /// # Safety
    /// The underlying X11 drawable must have been resized first.
    pub unsafe fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        cairo_xlib_surface_set_size(self.surface, width, height);
    }

    pub fn place_bar(&mut self, monitor: (i32, i32, i32, i32)) {
        let (x, y, w, h) = monitor;
        self.ui_scale = self
            .requested_scale
            .min((w as f64 - 16.0) / 488.0)
            .clamp(0.5, 3.0);
        self.bar_x = (x as f64 + (w as f64 - 488.0 * self.ui_scale) / 2.0).max(x as f64);
        self.bar_y = (y + h - 18) as f64 - 54.0 * self.ui_scale;
        self.monitor_h = h as f64;
    }

    /// # Safety
    /// The X11 display and drawable must still be live and used on this thread.
    pub unsafe fn paint(
        &self,
        config: &Config,
        ink: &Ink,
        cursor: Point,
        ui: (bool, usize),
        pressed: Option<Instant>,
        now: Instant,
    ) {
        let c = cairo_create(self.surface);
        cairo_set_operator(c, 0); // CLEAR
        cairo_paint(c);
        cairo_set_operator(c, 2); // OVER
        cairo_set_line_cap(c, 1);
        cairo_set_line_join(c, 1);
        let (r, g, b) = COLORS[config.color];
        let fade = Duration::from_secs_f64(config.fade);
        for stroke in &ink.strokes {
            if stroke.points.len() < 2 {
                continue;
            }
            let alpha = stroke.released.map_or(1.0, |t| {
                1.0 - now.saturating_duration_since(t).as_secs_f64() / fade.as_secs_f64()
            });
            cairo_set_source_rgba(c, r, g, b, alpha.max(0.0) * 0.8);
            cairo_set_line_width(c, config.thickness);
            cairo_move_to(c, stroke.points[0].x, stroke.points[0].y);
            for p in &stroke.points[1..] {
                cairo_line_to(c, p.x, p.y);
            }
            cairo_stroke(c);
        }
        let pulse = pressed.map_or(0.0, |t| {
            (1.0 - now.saturating_duration_since(t).as_secs_f64() / 0.28).max(0.0)
        });
        if pulse > 0.0 {
            cairo_set_source_rgba(c, r, g, b, pulse * 0.55);
            cairo_set_line_width(c, 2.5);
            cairo_arc(
                c,
                cursor.x,
                cursor.y,
                9.0 + 20.0 * (1.0 - pulse),
                0.0,
                std::f64::consts::TAU,
            );
            cairo_stroke(c);
        }
        if config.style == 36 {
            cairo_set_source_rgba(c, 1.0, 0.2, 0.15, 0.22);
            cairo_arc(c, cursor.x, cursor.y, 20.0, 0.0, std::f64::consts::TAU);
            cairo_fill(c);
            cairo_set_source_rgba(c, 1.0, 0.2, 0.15, 1.0);
            cairo_arc(c, cursor.x, cursor.y, 7.5, 0.0, std::f64::consts::TAU);
            cairo_fill(c);
            cairo_set_source_rgba(c, 1.0, 0.85, 0.8, 1.0);
            cairo_arc(
                c,
                cursor.x - 1.5,
                cursor.y - 1.5,
                2.2,
                0.0,
                std::f64::consts::TAU,
            );
            cairo_fill(c);
        } else if !self.hand.is_null() {
            let ih = cairo_image_surface_get_height(self.hand) as f64;
            let target = (config.size * self.monitor_h).clamp(45.0, 250.0);
            let scale = target / (ih * 0.48);
            cairo_save(c);
            cairo_translate(c, cursor.x, cursor.y);
            // Artwork's index finger is at the top, slightly left of the rod.
            cairo_rotate(c, -0.16);
            cairo_scale(c, scale, scale);
            cairo_set_source_surface(c, self.hand, -self.tip.x, -self.tip.y);
            cairo_paint(c);
            cairo_restore(c);
        }
        cairo_save(c);
        cairo_translate(c, self.bar_x, self.bar_y);
        cairo_scale(c, self.ui_scale, self.ui_scale);
        self.toolbar(c, config, ui.0, ui.1);
        cairo_restore(c);
        cairo_destroy(c);
        cairo_surface_flush(self.surface);
    }

    unsafe fn toolbar(&self, c: *mut Cairo, config: &Config, settings: bool, page: usize) {
        let x = 0.0;
        let y = 0.0;
        rounded(c, x, y, 488.0, 54.0, 16.0);
        cairo_set_source_rgba(c, 0.10, 0.11, 0.15, 0.92);
        cairo_fill(c);
        let labels = [
            "<",
            ">",
            "Hands",
            if config.sound { "Sound on" } else { "Muted" },
            "Clear",
            "Settings",
            "Exit",
        ];
        let widths = [42.0, 42.0, 86.0, 95.0, 66.0, 96.0, 61.0];
        let mut bx = x;
        for (i, (&label, width)) in labels.iter().zip(widths).enumerate() {
            if i == 2 {
                cairo_set_source_rgba(c, 0.58, 0.4, 0.89, 0.42);
                rounded(c, bx + 2.0, y + 6.0, width - 4.0, 42.0, 10.0);
                cairo_fill(c);
            }
            set_text(c, bx + 10.0, y + 33.0, 12.0, label);
            bx += width;
        }
        if page > 0 && !settings {
            let rows = 6;
            let h = 6.0 * 48.0 + 38.0;
            let py = y - h - 10.0;
            rounded(c, x, py, 488.0, h, 16.0);
            cairo_set_source_rgba(c, 0.1, 0.11, 0.15, 0.96);
            cairo_fill(c);
            set_text(c, x + 16.0, py + 26.0, 15.0, "Choose a pointer");
            for i in 0..rows {
                for j in 0..6 {
                    let index = (page - 1) * 36 + i * 6 + j;
                    if index >= STYLES.len() {
                        break;
                    }
                    let cx = x + 17.0 + j as f64 * 78.0;
                    let cy = py + 42.0 + i as f64 * 48.0;
                    if index == config.style {
                        rounded(c, cx - 4.0, cy, 77.0, 42.0, 9.0);
                        cairo_set_source_rgba(c, 0.55, 0.38, 0.8, 0.65);
                        cairo_fill(c);
                    }
                    let label = STYLES[index];
                    set_text(
                        c,
                        cx,
                        cy + 24.0,
                        if label.len() > 8 { 10.0 } else { 12.0 },
                        label,
                    );
                }
            }
        }
        if settings {
            let py = y - 374.0;
            rounded(c, x, py, 488.0, 362.0, 17.0);
            cairo_set_source_rgba(c, 0.10, 0.11, 0.15, 0.97);
            cairo_fill(c);
            set_text(c, x + 18.0, py + 30.0, 18.0, "TinyHand settings");
            let lines = [
                format!(
                    "Pointer size             -      {:.0}%      +",
                    config.size * 100.0
                ),
                format!(
                    "Drawing thickness       -      {:.0} px     +",
                    config.thickness
                ),
                format!(
                    "Drawing color            <       {}       >",
                    color_name(config.color)
                ),
                format!(
                    "Fade duration            -      {:.1} s      +",
                    config.fade
                ),
                format!(
                    "Sound volume             -      {:.0}%      +",
                    config.volume * 100.0
                ),
                format!(
                    "Start at login                      {}",
                    if config.startup { "On" } else { "Off" }
                ),
                format!("Toggle shortcut: {}", config.toggle),
                format!("Clear shortcut: {}", config.clear),
                "Esc exits presentation mode".to_string(),
            ];
            for (i, line) in lines.iter().enumerate() {
                set_text(c, x + 20.0, py + 65.0 + i as f64 * 31.0, 13.0, line);
            }
        }
    }
}

fn color_name(n: usize) -> &'static str {
    [
        "Purple", "Red", "Orange", "Green", "Blue", "Violet", "White", "Black",
    ][n]
}

unsafe fn measure_tip(surface: *mut Surface) -> Point {
    cairo_surface_flush(surface);
    let w = cairo_image_surface_get_width(surface) as usize;
    let h = cairo_image_surface_get_height(surface) as usize;
    let stride = cairo_image_surface_get_stride(surface) as usize;
    let data = cairo_image_surface_get_data(surface);
    if data.is_null() {
        return Point {
            x: w as f64 * 0.5,
            y: 0.0,
        };
    }
    for y in 0..h {
        let xs = (0..w)
            .filter(|&x| *data.add(y * stride + x * 4 + 3) > 128)
            .collect::<Vec<_>>();
        if !xs.is_empty() {
            return Point {
                x: (xs[0] + xs[xs.len() - 1]) as f64 / 2.0,
                y: y as f64,
            };
        }
    }
    Point {
        x: w as f64 * 0.5,
        y: 0.0,
    }
}

unsafe fn set_text(c: *mut Cairo, x: f64, y: f64, size: f64, s: &str) {
    cairo_set_source_rgba(c, 0.97, 0.96, 1.0, 1.0);
    cairo_select_font_face(c, b"Sans\0".as_ptr().cast(), 0, 0);
    cairo_set_font_size(c, size);
    cairo_move_to(c, x, y);
    if let Ok(t) = CString::new(s) {
        cairo_show_text(c, t.as_ptr());
    }
}

unsafe fn rounded(c: *mut Cairo, x: f64, y: f64, w: f64, h: f64, r: f64) {
    cairo_new_path(c);
    cairo_arc(c, x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cairo_arc(c, x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cairo_arc(
        c,
        x + r,
        y + h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    cairo_arc(
        c,
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        std::f64::consts::PI * 1.5,
    );
    cairo_close_path(c);
}

pub fn asset_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut paths = vec![];
    if let Some(p) = std::env::var_os("TINYHAND_ASSETS") {
        paths.push(PathBuf::from(p));
    }
    if let Some(parent) = exe.parent() {
        paths.push(parent.join("assets"));
        paths.push(parent.join("../share/tinyhand-rs/assets"));
    }
    paths.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"));
    paths
        .into_iter()
        .find(|p| p.join("hand_lavender.png").exists())
        .ok_or_else(|| "Assets missing: set TINYHAND_ASSETS to the assets directory".into())
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            if !self.hand.is_null() {
                cairo_surface_destroy(self.hand);
            }
            if !self.surface.is_null() {
                cairo_surface_destroy(self.surface);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transparent_canvas_hand_toolbar_and_ink() {
        unsafe {
            let surface = cairo_image_surface_create(0, 800, 600);
            let mut r = Renderer {
                surface,
                hand: std::ptr::null_mut(),
                tip: Point { x: 0.0, y: 0.0 },
                width: 800,
                height: 600,
                bar_x: 156.0,
                bar_y: 528.0,
                monitor_h: 600.0,
                ui_scale: 1.0,
                requested_scale: 1.0,
                asset_dir: asset_dir().unwrap(),
                loaded: 36,
            };
            r.select(0).unwrap();
            let mut ink = Ink::new();
            let now = Instant::now();
            ink.down(Point { x: 100.0, y: 100.0 });
            ink.move_to(Point { x: 150.0, y: 130.0 });
            ink.end(now);
            r.paint(
                &Config::default(),
                &ink,
                Point { x: 400.0, y: 300.0 },
                (false, 0),
                None,
                now,
            );
            cairo_surface_flush(surface);
            let data = cairo_image_surface_get_data(surface);
            let stride = cairo_image_surface_get_stride(surface) as usize;
            let alpha = |x: usize, y: usize| *data.add(y * stride + x * 4 + 3);
            assert_eq!(alpha(0, 0), 0, "the overlay background must be transparent");
            assert!(
                alpha(400, 300) > 0,
                "the pointer must be visible at the cursor"
            );
            assert!(alpha(400, 550) > 0, "the toolbar must be visible");
            assert!(alpha(125, 115) > 0, "the drag line must be visible");
            let file = std::env::temp_dir().join("tinyhand-render-test.png");
            let p = CString::new(file.to_string_lossy().as_bytes()).unwrap();
            assert_eq!(cairo_surface_write_to_png(surface, p.as_ptr()), 0);
        }
    }
}
