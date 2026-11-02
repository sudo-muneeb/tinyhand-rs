use crate::config::{Config, STYLES};
use crate::ffi::*;
use crate::ink::{Ink, Point};
use crate::settings::{self, SettingsUi};
mod hand;
mod settings_panel;
use crate::motion::{HandGeometry, HandMotion, HandPose};
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    unsafe fn clip_path(self, c: *mut Cairo) {
        // Fractional clip edges partially clear/repaint the same pixel on
        // each frame, leaving seams over opaque UI. Damage must cover whole
        // pixels, including antialiased shape edges.
        let x = self.x.floor();
        let y = self.y.floor();
        cairo_rectangle(
            c,
            x,
            y,
            (self.x + self.w).ceil() - x,
            (self.y + self.h).ceil() - y,
        );
    }
    fn around(p: Point, radius: f64) -> Self {
        Self {
            x: p.x - radius,
            y: p.y - radius,
            w: radius * 2.0,
            h: radius * 2.0,
        }
    }

    fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            w: (self.x + self.w).max(other.x + other.w) - x,
            h: (self.y + self.h).max(other.y + other.h) - y,
        }
    }
}

struct Frame {
    pointer: Rect,
    ink: Option<Rect>,
    ui: Rect,
    config: Config,
    settings: SettingsUi,
    page: usize,
}

pub struct Renderer {
    pub surface: *mut Surface,
    display: *mut Display,
    window: Window,
    pixmap: Pixmap,
    gc: *mut std::ffi::c_void,
    visual: *mut Visual,
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
    last_frame: Option<Frame>,
    pub settings_x: f64,
    pub settings_y: f64,
    pub settings_scale: f64,
    previews: Vec<*mut Surface>,
    custom_loaded: String,
    geometry: HandGeometry,
    motion: HandMotion,
    monitor: (i32, i32, i32, i32),
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
        let pixmap = XCreatePixmap(display, window, width as u32, height as u32, 32);
        let gc = XCreateGC(display, window, 0, std::ptr::null_mut());
        XSetGraphicsExposures(display, gc, 0);
        let surface = cairo_xlib_surface_create(display, pixmap, visual, width, height);
        if cairo_surface_status(surface) != 0 {
            cairo_surface_destroy(surface);
            XFreeGC(display, gc);
            XFreePixmap(display, pixmap);
            return Err("Cairo could not create X11 surface".into());
        }
        let mut this = Self {
            surface,
            display,
            window,
            pixmap,
            gc,
            visual,
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
            last_frame: None,
            settings_x: 60.0,
            settings_y: 56.0,
            settings_scale: 1.0,
            previews: Vec::new(),
            custom_loaded: String::new(),
            geometry: HandGeometry::default(),
            motion: HandMotion::default(),
            monitor: (0, 0, width, height),
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
            self.geometry = hand::measure_hand(self.hand);
            self.tip = self.geometry.tip;
        }
        self.loaded = style;
        Ok(())
    }

    /// # Safety
    /// Must run on the renderer's X11 thread with its display still live.
    pub unsafe fn select_config(&mut self, config: &Config) -> Result<(), String> {
        if !config.custom_enabled {
            self.custom_loaded.clear();
            return self.select(config.style);
        }
        if self.custom_loaded == config.custom_path && self.loaded == 37 {
            return Ok(());
        }
        let path = Path::new(&config.custom_path);
        use std::io::Read;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
            return Err("Choose a PNG smaller than 16 MB".into());
        }
        let mut bytes = [0_u8; 24];
        file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        if &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
            return Err("Choose a transparent PNG smaller than 16 MB".into());
        }
        let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        if w == 0 || h == 0 || w > 2048 || h > 2048 {
            return Err("The PNG must be at most 2048 x 2048 pixels".into());
        }
        let name = CString::new(config.custom_path.as_bytes()).map_err(|e| e.to_string())?;
        let hand = cairo_image_surface_create_from_png(name.as_ptr());
        if cairo_surface_status(hand) != 0 {
            cairo_surface_destroy(hand);
            return Err("Cannot read this PNG".into());
        }
        if !self.hand.is_null() {
            cairo_surface_destroy(self.hand);
        }
        self.hand = hand;
        self.geometry = hand::measure_hand(hand);
        self.tip = self.geometry.tip;
        self.loaded = 37;
        self.custom_loaded = config.custom_path.clone();
        self.invalidate();
        Ok(())
    }

    /// # Safety
    /// The underlying X11 drawable must have been resized first.
    pub unsafe fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        if !self.display.is_null() {
            cairo_surface_destroy(self.surface);
            XFreePixmap(self.display, self.pixmap);
            self.pixmap = XCreatePixmap(self.display, self.window, width as u32, height as u32, 32);
            self.surface =
                cairo_xlib_surface_create(self.display, self.pixmap, self.visual, width, height);
        } else {
            cairo_xlib_surface_set_size(self.surface, width, height);
        }
        self.invalidate();
    }

    pub fn pointer_down(&mut self) {
        self.motion.press();
    }
    pub fn pointer_up(&mut self) {
        self.motion.release();
    }
    pub fn reset_motion(&mut self) {
        self.motion.reset();
    }
    pub fn motion_animating(&self) -> bool {
        self.motion.is_animating()
    }

    pub fn invalidate(&mut self) {
        self.last_frame = None;
    }

    pub fn place_bar(&mut self, monitor: (i32, i32, i32, i32)) {
        self.monitor = monitor;
        let (x, y, w, h) = monitor;
        self.ui_scale = self
            .requested_scale
            .min((w as f64 - 16.0) / 488.0)
            .clamp(0.5, 3.0);
        self.bar_x = (x as f64 + (w as f64 - 488.0 * self.ui_scale) / 2.0).max(x as f64);
        self.bar_y = (y + h - 18) as f64 - 54.0 * self.ui_scale;
        self.monitor_h = h as f64;
        self.settings_scale = self
            .requested_scale
            .min((w as f64 - 32.0) / settings::WIDTH)
            .min((h as f64 - 110.0) / settings::HEIGHT)
            .clamp(0.4, 3.0);
        self.settings_x = x as f64 + (w as f64 - settings::WIDTH * self.settings_scale) / 2.0;
        self.settings_y = y as f64 + (h as f64 - settings::HEIGHT * self.settings_scale) / 2.0;
        self.settings_y = self
            .settings_y
            .min(self.bar_y - 16.0 - settings::HEIGHT * self.settings_scale)
            .max(y as f64 + 8.0);
    }

    /// # Safety
    /// The X11 display and drawable must still be live and used on this thread.
    pub unsafe fn paint(
        &mut self,
        config: &Config,
        ink: &Ink,
        cursor: Point,
        ui: (SettingsUi, usize),
        pressed: Option<Instant>,
        now: Instant,
    ) {
        let laser = config.style == 36 && !config.custom_enabled;
        let (grip, press) = self.motion.advance(cursor, self.monitor, now, !laser);
        let pose = HandPose::new(
            self.geometry,
            cursor,
            grip,
            self.monitor_h,
            config.size,
            config.angle,
            press,
        );
        let pointer = self.hand_bounds(config, cursor, pose);
        let ink_bounds = ink_bounds(ink, config.thickness);
        let ui_bounds = self.ui_bounds(ui);
        let mut damage = Rect {
            x: 0.0,
            y: 0.0,
            w: self.width as f64,
            h: self.height as f64,
        };
        let c = cairo_create(self.surface);
        if let Some(previous) = &self.last_frame {
            damage = previous.pointer.union(pointer);
            cairo_new_path(c);
            for rect in [
                Some(previous.pointer),
                Some(pointer),
                previous.ink,
                ink_bounds,
            ]
            .into_iter()
            .flatten()
            {
                damage = damage.union(rect);
                rect.clip_path(c);
            }
            // Repaint the modal panel as a coherent region.
            if ui.0.open
                || previous.ui != ui_bounds
                || previous.config != *config
                || previous.settings != ui.0
                || previous.page != ui.1
            {
                for rect in [previous.ui, ui_bounds] {
                    damage = damage.union(rect);
                    rect.clip_path(c);
                }
            }
            cairo_clip(c);
        }
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
        cairo_save(c);
        cairo_translate(c, self.bar_x, self.bar_y);
        cairo_scale(c, self.ui_scale, self.ui_scale);
        self.toolbar(c, config, ui.0.open, ui.1);
        cairo_restore(c);
        if ui.0.open {
            self.settings_panel(c, config, ui.0);
        }
        // The pointer and tap pulse are always the final layers, including
        // when their damage region intersects a toolbar or settings button.
        let pulse = pressed.map_or(0.0, |t| {
            (1.0 - now.saturating_duration_since(t).as_secs_f64() / 0.28).max(0.0)
        });
        if pulse > 0.0 {
            cairo_new_path(c);
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
        if config.style == 36 && !config.custom_enabled {
            cairo_set_source_rgba(c, 1.0, 0.2, 0.15, 0.22);
            cairo_new_path(c);
            cairo_arc(c, cursor.x, cursor.y, 20.0, 0.0, std::f64::consts::TAU);
            cairo_fill(c);
            cairo_set_source_rgba(c, 1.0, 0.2, 0.15, 1.0);
            cairo_new_path(c);
            cairo_arc(
                c,
                cursor.x,
                cursor.y,
                7.5 * (1.0 + 0.2 * press),
                0.0,
                std::f64::consts::TAU,
            );
            cairo_fill(c);
            cairo_set_source_rgba(c, 1.0, 0.85, 0.8, 1.0);
            cairo_new_path(c);
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
            self.paint_hand(c, config, cursor, pose);
        }
        cairo_destroy(c);
        if !self.display.is_null() {
            // Flush all Cairo work to the pixmap before publishing a complete
            // region. Drawing directly into the window exposes partial frames.
            cairo_surface_flush(self.surface);
            let x = damage.x.floor().clamp(0.0, self.width as f64) as i32;
            let y = damage.y.floor().clamp(0.0, self.height as f64) as i32;
            let right = (damage.x + damage.w)
                .ceil()
                .clamp(x as f64, self.width as f64) as i32;
            let bottom = (damage.y + damage.h)
                .ceil()
                .clamp(y as f64, self.height as f64) as i32;
            if right > x && bottom > y {
                XCopyArea(
                    self.display,
                    self.pixmap,
                    self.window,
                    self.gc,
                    x,
                    y,
                    (right - x) as u32,
                    (bottom - y) as u32,
                    x,
                    y,
                );
            }
        }
        self.last_frame = Some(Frame {
            pointer,
            ink: ink_bounds,
            ui: ui_bounds,
            config: config.clone(),
            settings: ui.0,
            page: ui.1,
        });
    }

    /// Release X11 resources before the owning display connection closes.
    /// # Safety
    /// The display must still be live; call only on the rendering thread.
    pub unsafe fn release_x11(&mut self) {
        if !self.surface.is_null() {
            cairo_surface_flush(self.surface);
            cairo_surface_destroy(self.surface);
            self.surface = std::ptr::null_mut();
        }
        for preview in self.previews.drain(..) {
            if !preview.is_null() {
                cairo_surface_destroy(preview);
            }
        }
        if !self.hand.is_null() {
            cairo_surface_destroy(self.hand);
            self.hand = std::ptr::null_mut();
        }
        if !self.display.is_null() {
            XFreeGC(self.display, self.gc);
            XFreePixmap(self.display, self.pixmap);
            self.display = std::ptr::null_mut();
        }
    }

    fn ui_bounds(&self, ui: (SettingsUi, usize)) -> Rect {
        let top = if !ui.0.open && ui.1 > 0 { -336.0 } else { 0.0 };
        let toolbar = Rect {
            x: self.bar_x - 2.0,
            y: self.bar_y + top * self.ui_scale - 2.0,
            w: 492.0 * self.ui_scale,
            h: (54.0 - top) * self.ui_scale + 4.0,
        };
        if ui.0.open {
            toolbar.union(Rect {
                x: self.settings_x - 16.0,
                y: self.settings_y - 16.0,
                w: settings::WIDTH * self.settings_scale + 32.0,
                h: settings::HEIGHT * self.settings_scale + 32.0,
            })
        } else {
            toolbar
        }
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
    }
}

fn ink_bounds(ink: &Ink, thickness: f64) -> Option<Rect> {
    let radius = thickness * 0.5 + 3.0;
    ink.strokes
        .iter()
        .flat_map(|stroke| stroke.points.iter())
        .fold(None, |bounds: Option<Rect>, point| {
            let next = Rect::around(*point, radius);
            Some(bounds.map_or(next, |rect| rect.union(next)))
        })
}

unsafe fn set_text(c: *mut Cairo, x: f64, y: f64, size: f64, s: &str) {
    cairo_set_source_rgba(c, 0.97, 0.96, 1.0, 1.0);
    cairo_select_font_face(c, c"Sans".as_ptr(), 0, 0);
    cairo_set_font_size(c, size);
    cairo_new_path(c);
    cairo_move_to(c, x, y);
    if let Ok(t) = CString::new(s) {
        cairo_show_text(c, t.as_ptr());
    }
    cairo_new_path(c);
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
            self.release_x11();
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
                display: std::ptr::null_mut(),
                window: 0,
                pixmap: 0,
                gc: std::ptr::null_mut(),
                visual: std::ptr::null_mut(),
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
                last_frame: None,
                settings_x: 60.0,
                settings_y: 56.0,
                settings_scale: 1.0,
                previews: Vec::new(),
                custom_loaded: String::new(),
                geometry: HandGeometry::default(),
                motion: HandMotion::default(),
                monitor: (0, 0, 800, 600),
            };
            r.select(0).unwrap();
            r.place_bar((0, 0, 800, 600));
            let mut ink = Ink::new();
            let now = Instant::now();
            ink.down(Point { x: 100.0, y: 100.0 });
            ink.move_to(Point { x: 150.0, y: 130.0 });
            ink.end(now);
            r.paint(
                &Config::default(),
                &ink,
                Point { x: 400.0, y: 300.0 },
                (SettingsUi::default(), 0),
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
            r.paint(
                &Config::default(),
                &ink,
                Point { x: 650.0, y: 300.0 },
                (SettingsUi::default(), 0),
                None,
                now,
            );
            cairo_surface_flush(surface);
            assert_eq!(
                alpha(400, 300),
                0,
                "the previous pointer position must be cleared"
            );
            assert!(alpha(650, 300) > 0, "the moved pointer must be visible");
            assert!(
                alpha(400, 550) > 0,
                "the toolbar must survive a clipped repaint"
            );
            let file = std::env::temp_dir().join("tinyhand-render-test.png");
            let p = CString::new(file.to_string_lossy().as_bytes()).unwrap();
            assert_eq!(cairo_surface_write_to_png(surface, p.as_ptr()), 0);
            // A bright laser pixel must remain above a toolbar button, and
            // a later clipped repaint must restore that button underneath it.
            let config = Config {
                style: 36,
                ..Config::default()
            };
            r.select(36).unwrap();
            r.paint(
                &config,
                &ink,
                Point { x: 400.0, y: 550.0 },
                (SettingsUi::default(), 0),
                None,
                now,
            );
            cairo_surface_flush(surface);
            let red = |x: usize, y: usize| *data.add(y * stride + x * 4 + 2);
            assert!(red(400, 550) > 200, "pointer must be above toolbar buttons");
            for tab in 0..6 {
                let ui = SettingsUi {
                    open: true,
                    tab,
                    ..SettingsUi::default()
                };
                r.paint(
                    &config,
                    &ink,
                    Point { x: 760.0, y: 30.0 },
                    (ui, 0),
                    None,
                    now,
                );
                let path = CString::new(format!("/tmp/tinyhand-settings-{tab}.png")).unwrap();
                assert_eq!(cairo_surface_write_to_png(surface, path.as_ptr()), 0);
            }
            let ui = SettingsUi {
                open: true,
                tab: 1,
                custom: true,
                ..SettingsUi::default()
            };
            r.paint(
                &config,
                &ink,
                Point { x: 760.0, y: 30.0 },
                (ui, 0),
                None,
                now,
            );
            let path = CString::new("/tmp/tinyhand-settings-custom.png").unwrap();
            assert_eq!(cairo_surface_write_to_png(surface, path.as_ptr()), 0);
            r.paint(
                &config,
                &ink,
                Point { x: 600.0, y: 190.0 },
                (ui, 0),
                None,
                now,
            );
            cairo_surface_flush(surface);
            assert!(
                red(600, 190) > 200,
                "pointer must be above settings controls"
            );
            // Fading ink underneath an opaque settings window must not
            // alter its pixels or leave rectangular damage-edge seams.
            let ui = SettingsUi {
                open: true,
                ..SettingsUi::default()
            };
            r.paint(
                &config,
                &ink,
                Point { x: 760.0, y: 30.0 },
                (ui, 0),
                None,
                now,
            );
            cairo_surface_flush(surface);
            let sample = || {
                (80..200)
                    .flat_map(|y| {
                        (80..220).flat_map(move |x| {
                            let pixel = data.add(y * stride + x * 4);
                            std::slice::from_raw_parts(pixel, 4).to_vec()
                        })
                    })
                    .collect::<Vec<_>>()
            };
            let before = sample();
            for millis in [100, 200, 400] {
                r.paint(
                    &config,
                    &ink,
                    Point { x: 760.0, y: 30.0 },
                    (ui, 0),
                    None,
                    now + Duration::from_millis(millis),
                );
                cairo_surface_flush(surface);
                assert_eq!(
                    sample(),
                    before,
                    "partial repaint must not leave seams in settings"
                );
            }
            let empty = Ink::new();
            r.paint(
                &config,
                &empty,
                Point { x: 160.0, y: 180.0 },
                (SettingsUi::default(), 0),
                Some(now),
                now,
            );
            cairo_surface_flush(surface);
            let stray = (330..440).any(|y| (330..460).any(|x| alpha(x, y) > 0));
            assert!(
                !stray,
                "a tap circle must not connect to the last toolbar label"
            );
            // Keep comparable rest, press, and release images for visual QA.
            r.select(0).unwrap();
            r.reset_motion();
            let hand = Config::default();
            for (name, millis, down) in [
                ("rest", 0, false),
                ("pressed", 16, true),
                ("released", 170, false),
            ] {
                if down {
                    r.pointer_down();
                } else {
                    r.pointer_up();
                }
                r.paint(
                    &hand,
                    &empty,
                    Point { x: 160.0, y: 180.0 },
                    (SettingsUi::default(), 0),
                    None,
                    now + Duration::from_millis(millis),
                );
                let path = CString::new(format!("/tmp/tinyhand-tap-{name}.png")).unwrap();
                assert_eq!(cairo_surface_write_to_png(surface, path.as_ptr()), 0);
            }
        }
    }
}
