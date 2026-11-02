use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use tinyhand_rs::config::{self, Config};
use tinyhand_rs::ffi::*;
use tinyhand_rs::ink::{Brush, Ink, Point};
use tinyhand_rs::render::{asset_dir, Renderer};
use tinyhand_rs::settings::{self, Action, SettingsUi};
use tinyhand_rs::sound::Sound;

static STOP: AtomicBool = AtomicBool::new(false);
static LAST_X_ERROR: AtomicU8 = AtomicU8::new(0);

fn cinnamon_session() -> bool {
    is_cinnamon_desktop(
        &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        &std::env::var("DESKTOP_SESSION").unwrap_or_default(),
    )
}

fn is_cinnamon_desktop(current: &str, session: &str) -> bool {
    current.split(':').any(|desktop| {
        desktop.eq_ignore_ascii_case("cinnamon") || desktop.eq_ignore_ascii_case("x-cinnamon")
    }) || session.eq_ignore_ascii_case("cinnamon")
}

fn is_muffin_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("muffin") || name.contains("cinnamon")
}

// Check the actual X11 window manager too: desktop environment variables can
// be absent or stale in terminals, launchers, and nested sessions.
unsafe fn muffin_window_manager(display: *mut Display, root: Window) -> bool {
    let mut actual_type = 0;
    let mut format = 0;
    let mut count = 0;
    let mut remaining = 0;
    let mut data = ptr::null_mut();
    let check = XInternAtom(display, c"_NET_SUPPORTING_WM_CHECK".as_ptr(), 0);
    let status = XGetWindowProperty(
        display,
        root,
        check,
        0,
        1,
        0,
        33,
        &mut actual_type,
        &mut format,
        &mut count,
        &mut remaining,
        &mut data,
    );
    let wm = if status == 0 && actual_type == 33 && format == 32 && count == 1 && !data.is_null() {
        *data.cast::<Window>()
    } else {
        0
    };
    if !data.is_null() {
        XFree(data.cast());
    }
    if wm == 0 {
        return false;
    }
    data = ptr::null_mut();
    let name = XInternAtom(display, c"_NET_WM_NAME".as_ptr(), 0);
    let utf8 = XInternAtom(display, c"UTF8_STRING".as_ptr(), 0);
    let status = XGetWindowProperty(
        display,
        wm,
        name,
        0,
        256,
        0,
        utf8,
        &mut actual_type,
        &mut format,
        &mut count,
        &mut remaining,
        &mut data,
    );
    let muffin = status == 0
        && actual_type == utf8
        && format == 8
        && !data.is_null()
        && is_muffin_name(&String::from_utf8_lossy(std::slice::from_raw_parts(
            data,
            count as usize,
        )));
    if !data.is_null() {
        XFree(data.cast());
    }
    muffin
}

// EWMH work areas exclude desktop panels and docks. Read only the active
// desktop's four CARDINALs, keeping the property allocation bounded.
unsafe fn desktop_work_area(display: *mut Display, root: Window) -> Option<(i32, i32, i32, i32)> {
    unsafe fn read(
        display: *mut Display,
        root: Window,
        name: &CStr,
        offset: i64,
        length: i64,
    ) -> Option<Vec<u64>> {
        let atom = XInternAtom(display, name.as_ptr(), 0);
        let (mut ty, mut format, mut count, mut after) = (0, 0, 0, 0);
        let mut data = ptr::null_mut();
        let status = XGetWindowProperty(
            display,
            root,
            atom,
            offset,
            length,
            0,
            6,
            &mut ty,
            &mut format,
            &mut count,
            &mut after,
            &mut data,
        );
        let result = if status == 0
            && ty == 6
            && format == 32
            && count <= length as u64
            && !data.is_null()
        {
            Some(std::slice::from_raw_parts(data.cast::<u64>(), count as usize).to_vec())
        } else {
            None
        };
        if !data.is_null() {
            XFree(data.cast());
        }
        result
    }
    let desktop = read(display, root, c"_NET_CURRENT_DESKTOP", 0, 1)
        .and_then(|v| v.first().copied())
        .unwrap_or(0)
        .min(4096);
    let values = read(display, root, c"_NET_WORKAREA", desktop as i64 * 4, 4)?;
    if values.len() != 4 {
        return None;
    }
    let (x, y, w, h) = (
        values[0] as u32 as i32,
        values[1] as u32 as i32,
        values[2] as i32,
        values[3] as i32,
    );
    (w > 0 && h > 0 && x.checked_add(w).is_some() && y.checked_add(h).is_some())
        .then_some((x, y, w, h))
}

extern "C" fn stop_signal(_: i32) {
    STOP.store(true, Ordering::SeqCst);
}
extern "C" fn x_error(_: *mut Display, error: *mut XErrorEvent) -> i32 {
    unsafe {
        LAST_X_ERROR.store((*error).error_code, Ordering::SeqCst);
        eprintln!(
            "tinyhand-rs: X11 error {} on request {}",
            (*error).error_code,
            (*error).request_code
        );
    }
    0
}
#[link(name = "c")]
extern "C" {
    fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
}

struct App {
    display: *mut Display,
    root: Window,
    window: Window,
    tray: Window,
    cmap: Colormap,
    invisible: Cursor,
    renderer: Renderer,
    config: Config,
    ink: Ink,
    sound: Sound,
    cursor: Point,
    pressed: Option<Instant>,
    active: bool,
    settings: SettingsUi,
    import_rx: Option<Receiver<Result<Option<String>, String>>>,
    page: usize,
    capture: Option<bool>,
    quit: bool,
    docked: bool,
    last_dock: Instant,
    monitors: Vec<(i32, i32, i32, i32)>,
    work_area: Option<(i32, i32, i32, i32)>,
    last_monitor_check: Instant,
    toggle_key: (i32, u32),
    clear_key: (i32, u32),
    redraw_pending: bool,
    last_render: Instant,
}

fn shortcut(display: *mut Display, value: &str) -> (i32, u32) {
    let bits: Vec<&str> = value.split('+').collect();
    let key = CString::new(bits.last().copied().unwrap_or("h")).expect("shortcut");
    let mut mods = 0;
    for bit in &bits[..bits.len() - 1] {
        match bit.to_ascii_lowercase().as_str() {
            "control" => mods |= 4,
            "alt" => mods |= 8,
            "shift" => mods |= 1,
            _ => {}
        }
    }
    unsafe {
        (
            XKeysymToKeycode(display, XStringToKeysym(key.as_ptr())) as i32,
            mods,
        )
    }
}

impl App {
    unsafe fn new() -> Result<Self, String> {
        XSetErrorHandler(Some(x_error));
        let display = XOpenDisplay(ptr::null());
        if display.is_null() {
            return Err("No X11 display. Log in to an X11 session and set DISPLAY. Wayland global input is unsupported.".into());
        }
        let screen = XDefaultScreen(display);
        let root = XRootWindow(display, screen);
        let disable_tray = cinnamon_session() || muffin_window_manager(display, root);
        if disable_tray {
            eprintln!("tinyhand-rs: legacy tray disabled on Cinnamon/Muffin; use Ctrl+Alt+H to toggle and Escape to release the pointer");
        }
        let w = XDisplayWidth(display, screen);
        let h = XDisplayHeight(display, screen);
        let mut vi = XVisualInfo::default();
        if XMatchVisualInfo(display, screen, 32, 4, &mut vi) == 0 {
            XCloseDisplay(display);
            return Err(
                "X11 server has no 32-bit ARGB visual. Enable the Cinnamon compositor.".into(),
            );
        }
        let format = XRenderFindVisualFormat(display, vi.visual);
        if format.is_null() || (*format).direct.alpha_mask == 0 {
            XCloseDisplay(display);
            return Err("X11 server has no ARGB visual with an alpha channel".into());
        }
        let cmap = XCreateColormap(display, root, vi.visual, 0);
        let mut attrs = XSetWindowAttributes {
            override_redirect: 1,
            colormap: cmap,
            border_pixel: 0,
            event_mask: (1 << 15) | (1 << 17),
            ..Default::default()
        };
        let window = XCreateWindow(
            display,
            root,
            0,
            0,
            w as u32,
            h as u32,
            0,
            32,
            1,
            vi.visual,
            (1 << 3) | (1 << 9) | (1 << 11) | (1 << 13),
            &mut attrs,
        );
        if window == 0 {
            XFreeColormap(display, cmap);
            XCloseDisplay(display);
            return Err("XCreateWindow failed".into());
        }
        XStoreName(display, window, c"TinyHand overlay".as_ptr());
        XShapeCombineRectangles(display, window, 2, 0, 0, ptr::null(), 0, 0, 0);
        let mut tray_attrs = XSetWindowAttributes {
            colormap: cmap,
            border_pixel: 0,
            event_mask: (1 << 15) | (1 << 2),
            ..Default::default()
        };
        let tray = if disable_tray {
            0
        } else {
            let tray = XCreateWindow(
                display,
                root,
                0,
                0,
                26,
                26,
                0,
                32,
                1,
                vi.visual,
                (1 << 3) | (1 << 11) | (1 << 13),
                &mut tray_attrs,
            );
            XStoreName(
                display,
                tray,
                c"TinyHand: left toggle, right settings, middle quit".as_ptr(),
            );
            let embed = XInternAtom(display, c"_XEMBED_INFO".as_ptr(), 0);
            let info: [usize; 2] = [0, 1];
            XChangeProperty(display, tray, embed, embed, 32, 0, info.as_ptr().cast(), 2);
            tray
        };
        XSelectInput(display, root, 1 << 17);
        let bitmap = XCreateBitmapFromData(display, root, c"".as_ptr(), 1, 1);
        let mut black = XColor::default();
        let invisible = XCreatePixmapCursor(display, bitmap, bitmap, &mut black, &mut black, 0, 0);
        XFreePixmap(display, bitmap);
        let config = Config::load(&Config::path());
        let renderer =
            match Renderer::new(display, window, vi.visual, w, h, asset_dir()?, config.style) {
                Ok(r) => r,
                Err(e) => {
                    XDestroyWindow(display, window);
                    if tray != 0 {
                        XDestroyWindow(display, tray);
                    }
                    XFreeColormap(display, cmap);
                    XCloseDisplay(display);
                    return Err(e);
                }
            };
        let toggle_key = shortcut(display, &config.toggle);
        let clear_key = shortcut(display, &config.clear);
        let mut app = Self {
            display,
            root,
            window,
            tray,
            cmap,
            invisible,
            renderer,
            config,
            ink: Ink::new(),
            sound: Sound::new(),
            cursor: Point {
                x: w as f64 / 2.0,
                y: h as f64 / 2.0,
            },
            pressed: None,
            active: false,
            settings: SettingsUi::default(),
            import_rx: None,
            page: 0,
            capture: None,
            quit: false,
            docked: false,
            last_dock: Instant::now() - Duration::from_secs(2),
            monitors: vec![(0, 0, w, h)],
            work_area: None,
            last_monitor_check: Instant::now(),
            toggle_key,
            clear_key,
            redraw_pending: false,
            last_render: Instant::now() - Duration::from_millis(34),
        };
        if let Err(error) = app.renderer.select_config(&app.config) {
            eprintln!("tinyhand-rs: {error}; using the built-in pointer");
            app.config.custom_enabled = false;
            app.renderer.select_config(&app.config)?;
        }
        app.register_keys();
        XSync(display, 0);
        app.query_pointer();
        app.refresh_monitors();
        app.dock_tray();
        Ok(app)
    }

    unsafe fn dock_tray(&mut self) {
        self.last_dock = Instant::now();
        if self.docked || self.tray == 0 {
            return;
        }
        let name = format!("_NET_SYSTEM_TRAY_S{}", XDefaultScreen(self.display));
        let name = CString::new(name).expect("atom");
        let selection = XInternAtom(self.display, name.as_ptr(), 0);
        let owner = XGetSelectionOwner(self.display, selection);
        if owner == 0 {
            return;
        }
        let opcode = XInternAtom(self.display, c"_NET_SYSTEM_TRAY_OPCODE".as_ptr(), 0);
        let mut message = XEvent {
            client: XClientMessageEvent {
                type_: 33,
                serial: 0,
                send_event: 1,
                display: self.display,
                window: owner,
                message_type: opcode,
                format: 32,
                data: [0, 0, self.tray as i64, 0, 0],
            },
        };
        XSendEvent(self.display, owner, 0, 0, &mut message);
        XMapRaised(self.display, self.tray);
        self.docked = true;
        self.paint_tray();
        XFlush(self.display);
    }

    unsafe fn paint_tray(&self) {
        let mut vi = XVisualInfo::default();
        if XMatchVisualInfo(self.display, XDefaultScreen(self.display), 32, 4, &mut vi) == 0 {
            return;
        }
        let surface = cairo_xlib_surface_create(self.display, self.tray, vi.visual, 26, 26);
        let c = cairo_create(surface);
        cairo_set_operator(c, 0);
        cairo_paint(c);
        cairo_set_operator(c, 2);
        cairo_set_source_rgba(c, if self.active { 0.77 } else { 0.62 }, 0.46, 0.91, 1.0);
        cairo_arc(c, 13.0, 13.0, 11.0, 0.0, std::f64::consts::TAU);
        cairo_fill(c);
        cairo_set_source_rgba(c, 1.0, 1.0, 1.0, 1.0);
        cairo_set_line_width(c, 2.0);
        cairo_move_to(c, 6.0, 17.0);
        cairo_line_to(c, 14.0, 7.0);
        cairo_stroke(c);
        cairo_arc(c, 15.0, 7.0, 3.5, 0.0, std::f64::consts::TAU);
        cairo_fill(c);
        cairo_destroy(c);
        cairo_surface_destroy(surface);
    }

    unsafe fn register_keys(&self) {
        for (name, (key, mods)) in [
            (&self.config.toggle, self.toggle_key),
            (&self.config.clear, self.clear_key),
        ] {
            for extra in [0, 2, 16, 18] {
                LAST_X_ERROR.store(0, Ordering::SeqCst);
                XGrabKey(self.display, key, mods | extra, self.root, 0, 1, 1);
                XSync(self.display, 0);
                if LAST_X_ERROR.load(Ordering::SeqCst) == 10 {
                    eprintln!(
                        "tinyhand-rs: shortcut {name} with lock mask {extra} is already in use"
                    );
                }
            }
        }
    }
    unsafe fn unregister_keys(&self) {
        for (key, mods) in [self.toggle_key, self.clear_key] {
            for extra in [0, 2, 16, 18] {
                XUngrabKey(self.display, key, mods | extra, self.root);
            }
        }
    }

    unsafe fn query_pointer(&mut self) {
        let (mut r, mut child, mut rx, mut ry, mut x, mut y, mut mask) = (0, 0, 0, 0, 0, 0, 0);
        if XQueryPointer(
            self.display,
            self.root,
            &mut r,
            &mut child,
            &mut rx,
            &mut ry,
            &mut x,
            &mut y,
            &mut mask,
        ) != 0
        {
            self.cursor = Point {
                x: rx as f64,
                y: ry as f64,
            };
        }
    }

    unsafe fn refresh_monitors(&mut self) {
        self.work_area = desktop_work_area(self.display, self.root);
        self.monitors.clear();
        if XineramaIsActive(self.display) != 0 {
            let mut count = 0;
            let screens = XineramaQueryScreens(self.display, &mut count);
            if !screens.is_null() {
                for s in std::slice::from_raw_parts(screens, count.max(0) as usize) {
                    self.monitors.push((
                        s.x_org as i32,
                        s.y_org as i32,
                        s.width as i32,
                        s.height as i32,
                    ));
                }
                XFree(screens.cast());
            }
        }
        if self.monitors.is_empty() {
            self.monitors
                .push((0, 0, self.renderer.width, self.renderer.height));
        }
        self.place_bar();
    }

    fn place_bar(&mut self) {
        let p = self.cursor;
        let m = self
            .monitors
            .iter()
            .find(|&&(x, y, w, h)| {
                p.x >= x as f64 && p.x < (x + w) as f64 && p.y >= y as f64 && p.y < (y + h) as f64
            })
            .copied()
            .unwrap_or(self.monitors[0]);
        self.renderer.place_bar_with_work_area(m, self.work_area);
    }

    unsafe fn activate(&mut self) -> Result<(), String> {
        if self.active {
            return Ok(());
        }
        // PointerMotionHintMask would suppress subsequent movement until an
        // XQueryPointer acknowledgement. Request ordinary motion events;
        // run() already bounds event batches and rendering frequency.
        let mask = (1 << 6) | (1 << 2) | (1 << 3);
        let status = XGrabPointer(self.display, self.root, 0, mask, 1, 1, 0, self.invisible, 0);
        if status != 0 {
            return Err(format!(
                "XGrabPointer failed ({status}). Another program may have captured the pointer."
            ));
        }
        self.active = true;
        self.query_pointer();
        self.place_bar();
        self.renderer.invalidate();
        let escape = XKeysymToKeycode(self.display, 0xff1b) as i32;
        for extra in [0, 2, 16, 18] {
            LAST_X_ERROR.store(0, Ordering::SeqCst);
            XGrabKey(self.display, escape, extra, self.root, 0, 1, 1);
            XSync(self.display, 0);
            if extra == 0 && LAST_X_ERROR.load(Ordering::SeqCst) != 0 {
                self.active = false;
                XUngrabPointer(self.display, 0);
                XFlush(self.display);
                return Err("Cannot register Escape to leave presentation mode".into());
            }
        }
        XMapRaised(self.display, self.window);
        if self.docked {
            self.paint_tray();
        }
        self.draw();
        XFlush(self.display);
        Ok(())
    }

    unsafe fn deactivate(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.renderer.reset_motion();
        self.redraw_pending = false;
        self.pressed = None;
        self.ink.end(Instant::now());
        self.ink.clear();
        self.settings.open = false;
        self.page = 0;
        self.capture = None;
        XUngrabKeyboard(self.display, 0);
        XUngrabPointer(self.display, 0);
        let escape = XKeysymToKeycode(self.display, 0xff1b) as i32;
        for extra in [0, 2, 16, 18] {
            XUngrabKey(self.display, escape, extra, self.root);
        }
        XUnmapWindow(self.display, self.window);
        if self.docked {
            self.paint_tray();
        }
        XFlush(self.display);
    }

    fn draw(&mut self) {
        if self.active {
            self.redraw_pending = true;
        }
    }

    unsafe fn render(&mut self, now: Instant) {
        if self.active && self.redraw_pending {
            self.renderer.paint(
                &self.config,
                &self.ink,
                self.cursor,
                (
                    SettingsUi {
                        capture: self.capture,
                        ..self.settings
                    },
                    self.page,
                ),
                self.pressed,
                now,
            );
            XFlush(self.display);
            self.last_render = now;
            self.redraw_pending = false;
        }
    }

    fn bar_hit(&self, p: Point) -> Option<usize> {
        let x = (p.x - self.renderer.bar_x) / self.renderer.ui_scale;
        let y = (p.y - self.renderer.bar_y) / self.renderer.ui_scale;
        if !(0.0..=54.0).contains(&y) || !(0.0..488.0).contains(&x) {
            return None;
        }
        let mut edge = 0.0;
        for (i, width) in [42.0, 42.0, 86.0, 95.0, 66.0, 96.0, 61.0]
            .iter()
            .enumerate()
        {
            edge += width;
            if x < edge {
                return Some(i);
            }
        }
        None
    }
    fn grid_hit(&self, p: Point) -> Option<usize> {
        if self.page == 0 || self.settings.open {
            return None;
        }
        let x = (p.x - self.renderer.bar_x) / self.renderer.ui_scale - 17.0;
        let y = (p.y - self.renderer.bar_y) / self.renderer.ui_scale + 336.0 - 42.0;
        if x < 0.0 || y < 0.0 || x >= 468.0 || y >= 288.0 {
            return None;
        }
        let idx = (self.page - 1) * 36 + (y / 48.0) as usize * 6 + (x / 78.0) as usize;
        (idx < 37).then_some(idx)
    }
    unsafe fn settings_hit(&mut self, p: Point) -> bool {
        if !self.settings.open {
            return false;
        }
        let x = (p.x - self.renderer.settings_x) / self.renderer.settings_scale;
        let y = (p.y - self.renderer.settings_y) / self.renderer.settings_scale;
        if !settings::contains(x, y, (0.0, 0.0, settings::WIDTH, settings::HEIGHT)) {
            return false;
        }
        if let Some(action) = settings::hit(x, y, self.settings) {
            self.settings_action(action);
        }
        true
    }

    unsafe fn settings_action(&mut self, action: Action) {
        match action {
            Action::Close => self.settings.open = false,
            Action::Tab(tab) => {
                self.settings.tab = tab;
                self.settings.custom = false;
            }
            Action::Custom => self.settings.custom = true,
            Action::Back => self.settings.custom = false,
            Action::UseCustom => {
                if !self.config.custom_path.is_empty() {
                    self.config.custom_enabled = true;
                }
            }
            Action::Style(style) => {
                self.config.style = style;
                self.config.custom_enabled = false;
            }
            Action::Reset => {
                self.config.style = 0;
                self.config.custom_enabled = false;
            }
            Action::Startup => {
                self.config.startup = !self.config.startup;
                if let Ok(exe) = std::env::current_exe() {
                    if let Err(e) = config::update_autostart(self.config.startup, &exe) {
                        eprintln!("Autostart: {e}");
                    }
                }
            }
            Action::Capture(toggle) => {
                self.capture = Some(toggle);
                if XGrabKeyboard(self.display, self.root, 0, 1, 1, 0) != 0 {
                    self.capture = None;
                }
            }
            Action::Size(step) => {
                self.config.size = (self.config.size + step * 0.01).clamp(0.05, 0.35)
            }
            Action::Angle(step) => {
                self.config.angle = (self.config.angle + step * 0.08).clamp(-1.2, 1.2)
            }
            Action::Opacity(step) => {
                self.config.opacity = (self.config.opacity + step * 0.1).clamp(0.2, 1.0)
            }
            Action::Shadow => self.config.shadow = !self.config.shadow,
            Action::Thickness(step) => {
                self.config.thickness = (self.config.thickness + step).clamp(1.0, 30.0)
            }
            Action::Fade(step) => {
                self.config.fade = (self.config.fade + step * 0.5).clamp(0.3, 20.0)
            }
            Action::Color(color) => {
                self.config.color = color;
                self.config.match_color = false;
            }
            Action::MatchColor => self.config.match_color = true,
            Action::Sound(kind) => {
                self.config.sound = kind > 0;
                if kind > 0 {
                    self.config.sound_kind = kind;
                    self.sound.tap(self.config.volume, kind);
                }
            }
            Action::Volume(step) => {
                self.config.volume = (self.config.volume + step * 0.1).clamp(0.0, 1.0)
            }
            Action::LaserSound => self.config.laser_sound = !self.config.laser_sound,
            Action::Clear => self.ink.clear(),
            Action::Import => {
                self.choose_custom_hand();
                return;
            }
        }
        if let Err(e) = self.renderer.select_config(&self.config) {
            eprintln!("{e}");
            self.config.custom_enabled = false;
            self.settings.notice = Some("The imported PNG is unavailable. Choose it again.");
            let _ = self.renderer.select_config(&self.config);
        }
        self.save();
        self.draw();
    }

    unsafe fn choose_custom_hand(&mut self) {
        // Release capture while the native chooser is open. Its subprocess
        // runs off the event thread; the desktop remains usable throughout.
        self.deactivate();
        let (tx, rx) = mpsc::channel();
        self.import_rx = Some(rx);
        std::thread::spawn(move || {
            let result = std::process::Command::new("zenity")
                .args([
                    "--file-selection",
                    "--title=Choose your TinyHand PNG",
                    "--file-filter=PNG images | *.png",
                ])
                .output()
                .map_err(|e| format!("Cannot open the file chooser: {e}"))
                .and_then(|out| {
                    if !out.status.success() {
                        return Ok(None);
                    }
                    let path = String::from_utf8(out.stdout).map_err(|e| e.to_string())?;
                    Ok(Some(path.trim_end_matches(['\n', '\r']).to_owned()))
                });
            let _ = tx.send(result);
        });
    }

    unsafe fn poll_import(&mut self) {
        let Some(result) = self.import_rx.as_ref().and_then(|rx| rx.try_recv().ok()) else {
            return;
        };
        self.import_rx = None;
        let mut notice = None;
        if let Ok(Some(path)) = &result {
            let previous = self.config.clone();
            self.config.custom_path = path.clone();
            self.config.custom_enabled = true;
            if let Err(e) = self.renderer.select_config(&self.config) {
                eprintln!("Import: {e}");
                notice = Some("Cannot read that PNG. Try another image.");
                self.config = previous;
            } else {
                self.save();
            }
        } else if let Err(e) = result {
            eprintln!("Import: {e}");
            notice = Some("Install zenity to choose a PNG file.");
        }
        if let Err(e) = self.activate() {
            eprintln!("{e}");
        }
        self.settings = SettingsUi {
            open: true,
            tab: 1,
            custom: true,
            capture: None,
            notice,
        };
        self.draw();
    }

    fn save(&self) {
        if let Err(e) = self.config.save(&Config::path()) {
            eprintln!("Could not save settings: {e}");
        }
    }

    unsafe fn press(&mut self, button: u32, p: Point) {
        self.cursor = p;
        self.place_bar();
        if self.capture.take().is_some() {
            XUngrabKeyboard(self.display, 0);
        }
        if button == 4 || button == 5 {
            self.config.custom_enabled = false;
            self.config.style = (self.config.style + if button == 4 { 36 } else { 1 }) % 37;
            if let Err(e) = self.renderer.select_config(&self.config) {
                eprintln!("{e}");
            }
            self.save();
            self.draw();
            return;
        }
        if button != 1 && button != 3 {
            return;
        }
        self.ink.end(Instant::now());
        self.renderer.pointer_down();
        if self.settings_hit(p) {
            self.draw();
            return;
        }
        if let Some(i) = self.bar_hit(p) {
            match i {
                0 | 1 => {
                    self.config.custom_enabled = false;
                    self.config.style = (self.config.style + if i == 0 { 36 } else { 1 }) % 37;
                    if let Err(e) = self.renderer.select_config(&self.config) {
                        eprintln!("{e}");
                    }
                    self.save();
                }
                2 => {
                    self.page = if self.page == 0 {
                        1
                    } else if self.page == 1 {
                        2
                    } else {
                        0
                    };
                    self.settings.open = false;
                }
                3 => {
                    self.config.sound = !self.config.sound;
                    self.save();
                }
                4 => self.ink.clear(),
                5 => {
                    self.settings.open = !self.settings.open;
                    self.page = 0;
                }
                6 => self.deactivate(),
                _ => {}
            }
        } else if let Some(style) = self.grid_hit(p) {
            self.config.style = style;
            self.config.custom_enabled = false;
            if let Err(e) = self.renderer.select(style) {
                eprintln!("{e}");
            }
            self.page = 0;
            self.save();
        } else {
            self.page = 0;
            if self.settings.open {
                self.settings.open = false;
            } else {
                self.ink.down_with_brush(
                    p,
                    Brush {
                        color: self.renderer.ink_color(&self.config),
                        width: self.config.thickness,
                    },
                );
                self.pressed = Some(Instant::now());
                if self.config.sound
                    && (self.config.style != 36
                        || self.config.custom_enabled
                        || self.config.laser_sound)
                {
                    self.sound.tap(self.config.volume, self.config.sound_kind);
                }
            }
        }
        self.draw();
    }

    unsafe fn handle(&mut self, e: XEvent) {
        match e.type_ {
            2 => {
                let k = e.key;
                if let Some(toggle) = self.capture {
                    let keysym = XKeycodeToKeysym(self.display, k.keycode as u8, 0);
                    let text = XKeysymToString(keysym);
                    if !text.is_null() {
                        let s = CStr::from_ptr(text).to_string_lossy();
                        if s.len() == 1
                            && s.as_bytes()[0].is_ascii_alphabetic()
                            && k.state & (4 | 8 | 1) != 0
                        {
                            let mut name = String::new();
                            if k.state & 4 != 0 {
                                name.push_str("Control+");
                            }
                            if k.state & 8 != 0 {
                                name.push_str("Alt+");
                            }
                            if k.state & 1 != 0 {
                                name.push_str("Shift+");
                            }
                            name.push_str(&s.to_ascii_lowercase());
                            self.unregister_keys();
                            if toggle {
                                self.config.toggle = name;
                            } else {
                                self.config.clear = name;
                            }
                            self.toggle_key = shortcut(self.display, &self.config.toggle);
                            self.clear_key = shortcut(self.display, &self.config.clear);
                            self.register_keys();
                            self.save();
                        }
                    }
                    XUngrabKeyboard(self.display, 0);
                    self.capture = None;
                    self.draw();
                    return;
                }
                if k.keycode == XKeysymToKeycode(self.display, 0xff1b) as u32 && self.active {
                    self.deactivate();
                    return;
                }
                let mods = k.state & !(2 | 16);
                if (k.keycode as i32, mods) == self.toggle_key {
                    if self.active {
                        self.deactivate();
                    } else if let Err(err) = self.activate() {
                        eprintln!("{err}");
                    }
                } else if (k.keycode as i32, mods) == self.clear_key {
                    self.ink.clear();
                    self.draw();
                }
            }
            4 if e.button.window == self.tray => match e.button.button {
                1 => {
                    if self.active {
                        self.deactivate();
                    } else if let Err(err) = self.activate() {
                        eprintln!("{err}");
                    }
                }
                2 => self.quit = true,
                3 => {
                    if !self.active {
                        if let Err(err) = self.activate() {
                            eprintln!("{err}");
                        }
                    }
                    self.settings.open = true;
                    self.page = 0;
                    self.draw();
                }
                _ => {}
            },
            4 if self.active => {
                let b = e.button;
                self.press(
                    b.button,
                    Point {
                        x: b.x_root as f64,
                        y: b.y_root as f64,
                    },
                );
            }
            5 if self.active => {
                let b = e.button;
                self.ink.end(Instant::now());
                if e.button.button == 1 || e.button.button == 3 {
                    self.renderer.pointer_up();
                }
                self.cursor = Point {
                    x: b.x_root as f64,
                    y: b.y_root as f64,
                };
                self.place_bar();
                self.draw();
            }
            6 if self.active => {
                let m = e.motion;
                let p = Point {
                    x: m.x_root as f64,
                    y: m.y_root as f64,
                };
                self.cursor = p;
                self.place_bar();
                if m.state & ((1 << 8) | (1 << 10)) != 0 {
                    self.ink.move_to(p);
                } else {
                    self.ink.end(Instant::now());
                }
                self.draw();
            }
            12 if e.button.window == self.tray => self.paint_tray(),
            12 if self.active => {
                self.renderer.invalidate();
                self.draw();
            }
            22 => {
                let c = e.configure;
                if c.window == self.root
                    && (self.renderer.width != c.width || self.renderer.height != c.height)
                {
                    XMoveResizeWindow(
                        self.display,
                        self.window,
                        0,
                        0,
                        c.width as u32,
                        c.height as u32,
                    );
                    self.renderer.resize(c.width, c.height);
                    self.refresh_monitors();
                    self.draw();
                }
            }
            _ => {}
        }
    }

    unsafe fn run(&mut self) -> Result<(), String> {
        self.activate()?;
        let fd = XConnectionNumber(self.display);
        while !self.quit && !STOP.load(Ordering::SeqCst) {
            self.poll_import();
            let batch_start = Instant::now();
            let mut events = 0;
            while XPending(self.display) > 0
                && events < 256
                && batch_start.elapsed() < Duration::from_millis(5)
            {
                let mut event = XEvent::default();
                XNextEvent(self.display, &mut event);
                self.handle(event);
                events += 1;
            }
            let now = Instant::now();
            if now.duration_since(self.last_monitor_check) > Duration::from_secs(2) {
                let before = (
                    self.renderer.bar_x,
                    self.renderer.bar_y,
                    self.renderer.ui_scale,
                    self.renderer.monitor_h,
                );
                self.refresh_monitors();
                self.last_monitor_check = now;
                if before
                    != (
                        self.renderer.bar_x,
                        self.renderer.bar_y,
                        self.renderer.ui_scale,
                        self.renderer.monitor_h,
                    )
                {
                    self.draw();
                }
            }
            let old = self.ink.strokes.len();
            self.ink
                .prune(now, Duration::from_secs_f64(self.config.fade));
            if self
                .pressed
                .is_some_and(|t| now.duration_since(t) >= Duration::from_millis(300))
            {
                self.pressed = None;
                self.draw();
            }
            if !self.docked && now.duration_since(self.last_dock) > Duration::from_secs(2) {
                self.dock_tray();
            }
            let animating = self.active
                && (self.renderer.motion_animating()
                    || self.ink.strokes.iter().any(|s| s.released.is_some())
                    || self
                        .pressed
                        .is_some_and(|t| now.duration_since(t) < Duration::from_millis(300)));
            let animation_interval = if self.renderer.motion_animating() {
                16
            } else {
                33
            };
            if self.active
                && (old != self.ink.strokes.len()
                    || (animating
                        && now.duration_since(self.last_render)
                            >= Duration::from_millis(animation_interval)))
            {
                self.draw();
            }
            let elapsed = now.duration_since(self.last_render);
            if self.redraw_pending && (events == 0 || elapsed >= Duration::from_millis(16)) {
                self.render(now);
            }
            if XPending(self.display) > 0 {
                continue;
            }
            let wait = if self.redraw_pending {
                16_u64.saturating_sub(self.last_render.elapsed().as_millis() as u64)
            } else if animating {
                animation_interval.saturating_sub(self.last_render.elapsed().as_millis() as u64)
            } else {
                500
            };
            let mut pfd = PollFd {
                fd,
                events: 1,
                revents: 0,
            };
            poll(&mut pfd, 1, wait.min(500) as i32);
        }
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            self.deactivate();
            self.unregister_keys();
            XSync(self.display, 0);
            std::thread::sleep(Duration::from_millis(100));
            self.renderer.release_x11();
            XDestroyWindow(self.display, self.window);
            if self.tray != 0 {
                XDestroyWindow(self.display, self.tray);
            }
            XFreeCursor(self.display, self.invisible);
            XFreeColormap(self.display, self.cmap);
            XCloseDisplay(self.display);
        }
    }
}

fn main() {
    unsafe {
        signal(2, stop_signal);
        signal(15, stop_signal);
    }
    let result = unsafe { App::new().and_then(|mut app| app.run()) };
    if let Err(e) = result {
        eprintln!("tinyhand-rs: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod safety_tests {
    use super::{is_cinnamon_desktop, is_muffin_name};

    #[test]
    fn cinnamon_and_muffin_names_disable_the_legacy_tray() {
        assert!(is_cinnamon_desktop("X-Cinnamon", ""));
        assert!(is_cinnamon_desktop("GNOME:Cinnamon", ""));
        assert!(is_cinnamon_desktop("", "cinnamon"));
        assert!(!is_cinnamon_desktop("XFCE", "xfce"));
        assert!(is_muffin_name("Mutter (Muffin)"));
        assert!(is_muffin_name("Cinnamon"));
        assert!(!is_muffin_name("Mutter"));
        assert!(!is_muffin_name("Xfwm4"));
    }
}
