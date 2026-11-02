use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tinyhand_rs::config::{self, Config};
use tinyhand_rs::ffi::*;
use tinyhand_rs::ink::{Ink, Point};
use tinyhand_rs::render::{asset_dir, Renderer};
use tinyhand_rs::sound::Sound;

static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop_signal(_: i32) {
    STOP.store(true, Ordering::SeqCst);
}
extern "C" fn x_error(_: *mut Display, error: *mut XErrorEvent) -> i32 {
    unsafe {
        eprintln!(
            "tinyhand-rs: X11 error {} on request {} (shortcut conflict or unsupported visual)",
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
    settings: bool,
    page: usize,
    capture: Option<bool>,
    quit: bool,
    docked: bool,
    last_dock: Instant,
    monitors: Vec<(i32, i32, i32, i32)>,
    last_monitor_check: Instant,
    toggle_key: (i32, u32),
    clear_key: (i32, u32),
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
        XStoreName(display, window, b"TinyHand overlay\0".as_ptr().cast());
        XShapeCombineRectangles(display, window, 2, 0, 0, ptr::null(), 0, 0, 0);
        let mut tray_attrs = XSetWindowAttributes {
            colormap: cmap,
            border_pixel: 0,
            event_mask: (1 << 15) | (1 << 2),
            ..Default::default()
        };
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
            b"TinyHand: left toggle, right settings, middle quit\0"
                .as_ptr()
                .cast(),
        );
        let embed = XInternAtom(display, b"_XEMBED_INFO\0".as_ptr().cast(), 0);
        let info: [usize; 2] = [0, 1];
        XChangeProperty(display, tray, embed, embed, 32, 0, info.as_ptr().cast(), 2);
        XSelectInput(display, root, 1 << 17);
        let bitmap = XCreateBitmapFromData(display, root, b"\0".as_ptr().cast(), 1, 1);
        let mut black = XColor::default();
        let invisible = XCreatePixmapCursor(display, bitmap, bitmap, &mut black, &mut black, 0, 0);
        XFreePixmap(display, bitmap);
        let config = Config::load(&Config::path());
        let renderer =
            match Renderer::new(display, window, vi.visual, w, h, asset_dir()?, config.style) {
                Ok(r) => r,
                Err(e) => {
                    XDestroyWindow(display, window);
                    XDestroyWindow(display, tray);
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
            settings: false,
            page: 0,
            capture: None,
            quit: false,
            docked: false,
            last_dock: Instant::now() - Duration::from_secs(2),
            monitors: vec![(0, 0, w, h)],
            last_monitor_check: Instant::now(),
            toggle_key,
            clear_key,
        };
        app.register_keys();
        XSync(display, 0);
        app.query_pointer();
        app.refresh_monitors();
        app.dock_tray();
        Ok(app)
    }

    unsafe fn dock_tray(&mut self) {
        self.last_dock = Instant::now();
        if self.docked {
            return;
        }
        let name = format!("_NET_SYSTEM_TRAY_S{}", XDefaultScreen(self.display));
        let name = CString::new(name).expect("atom");
        let selection = XInternAtom(self.display, name.as_ptr(), 0);
        let owner = XGetSelectionOwner(self.display, selection);
        if owner == 0 {
            return;
        }
        let opcode = XInternAtom(
            self.display,
            b"_NET_SYSTEM_TRAY_OPCODE\0".as_ptr().cast(),
            0,
        );
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
        for (key, mods) in [self.toggle_key, self.clear_key] {
            for extra in [0, 2, 16, 18] {
                XGrabKey(self.display, key, mods | extra, self.root, 0, 1, 1);
            }
        }
        XFlush(self.display);
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
        self.renderer.place_bar(m);
    }

    unsafe fn activate(&mut self) -> Result<(), String> {
        if self.active {
            return Ok(());
        }
        let mask = (1 << 6) | (1 << 7) | (1 << 8) | (1 << 2) | (1 << 3);
        let status = XGrabPointer(self.display, self.root, 0, mask, 1, 1, 0, self.invisible, 0);
        if status != 0 {
            return Err(format!(
                "XGrabPointer failed ({status}). Another program may have captured the pointer."
            ));
        }
        self.active = true;
        XGrabKey(
            self.display,
            XKeysymToKeycode(self.display, 0xff1b) as i32,
            32768,
            self.root,
            0,
            1,
            1,
        );
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
        self.ink.end(Instant::now());
        self.ink.clear();
        self.settings = false;
        self.page = 0;
        self.capture = None;
        XUngrabKeyboard(self.display, 0);
        XUngrabPointer(self.display, 0);
        XUngrabKey(
            self.display,
            XKeysymToKeycode(self.display, 0xff1b) as i32,
            32768,
            self.root,
        );
        XUnmapWindow(self.display, self.window);
        if self.docked {
            self.paint_tray();
        }
        XFlush(self.display);
    }

    unsafe fn draw(&self) {
        if self.active {
            self.renderer.paint(
                &self.config,
                &self.ink,
                self.cursor,
                (self.settings, self.page),
                self.pressed,
                Instant::now(),
            );
            XFlush(self.display);
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
        if self.page == 0 || self.settings {
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
        if !self.settings {
            return false;
        }
        let x = (p.x - self.renderer.bar_x) / self.renderer.ui_scale;
        let y = (p.y - self.renderer.bar_y) / self.renderer.ui_scale + 374.0;
        if !(0.0..488.0).contains(&x) || !(0.0..362.0).contains(&y) {
            return false;
        }
        if (46.0..232.0).contains(&y) {
            let row = ((y - 46.0) / 31.0) as usize;
            let direction = if x > 250.0 { 1.0 } else { -1.0 };
            match row {
                0 => self.config.size = (self.config.size + direction * 0.01).clamp(0.05, 0.35),
                1 => self.config.thickness = (self.config.thickness + direction).clamp(1.0, 30.0),
                2 => {
                    self.config.color = ((self.config.color as i32
                        + if direction > 0.0 { 1 } else { 7 })
                        % 8) as usize
                }
                3 => self.config.fade = (self.config.fade + direction * 0.5).clamp(0.3, 20.0),
                4 => self.config.volume = (self.config.volume + direction * 0.1).clamp(0.0, 1.0),
                5 => {
                    self.config.startup = !self.config.startup;
                    if let Ok(exe) = std::env::current_exe() {
                        if let Err(e) = config::update_autostart(self.config.startup, &exe) {
                            eprintln!("Autostart: {e}");
                        }
                    }
                }
                _ => {}
            }
            self.save();
        } else if (232.0..302.0).contains(&y) {
            self.capture = Some(y < 263.0);
            if XGrabKeyboard(self.display, self.root, 0, 1, 1, 0) != 0 {
                self.capture = None;
                eprintln!("Cannot capture shortcut: keyboard is busy");
            }
        }
        true
    }
    fn save(&self) {
        if let Err(e) = self.config.save(&Config::path()) {
            eprintln!("Could not save settings: {e}");
        }
    }

    unsafe fn press(&mut self, button: u32, p: Point) {
        self.cursor = p;
        self.place_bar();
        if button == 4 || button == 5 {
            self.config.style = (self.config.style + if button == 4 { 36 } else { 1 }) % 37;
            if let Err(e) = self.renderer.select(self.config.style) {
                eprintln!("{e}");
            }
            self.save();
            self.draw();
            return;
        }
        if button != 1 && button != 3 {
            return;
        }
        if let Some(i) = self.bar_hit(p) {
            match i {
                0 | 1 => {
                    self.config.style = (self.config.style + if i == 0 { 36 } else { 1 }) % 37;
                    if let Err(e) = self.renderer.select(self.config.style) {
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
                    self.settings = false;
                }
                3 => {
                    self.config.sound = !self.config.sound;
                    self.save();
                }
                4 => self.ink.clear(),
                5 => {
                    self.settings = !self.settings;
                    self.page = 0;
                }
                6 => self.deactivate(),
                _ => {}
            }
        } else if self.settings_hit(p) {
        } else if let Some(style) = self.grid_hit(p) {
            self.config.style = style;
            if let Err(e) = self.renderer.select(style) {
                eprintln!("{e}");
            }
            self.page = 0;
            self.save();
        } else {
            self.page = 0;
            if self.settings {
                self.settings = false;
            } else {
                self.ink.down(p);
                self.pressed = Some(Instant::now());
                if self.config.sound {
                    self.sound.tap(self.config.volume);
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
                    self.settings = true;
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
                self.ink.move_to(p);
                self.draw();
            }
            12 if e.button.window == self.tray => self.paint_tray(),
            12 if self.active => self.draw(),
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
            while XPending(self.display) > 0 {
                let mut event = XEvent::default();
                XNextEvent(self.display, &mut event);
                self.handle(event);
            }
            let now = Instant::now();
            if now.duration_since(self.last_monitor_check) > Duration::from_secs(2) {
                self.refresh_monitors();
                self.last_monitor_check = now;
                self.draw();
            }
            let old = self.ink.strokes.len();
            self.ink
                .prune(now, Duration::from_secs_f64(self.config.fade));
            if !self.docked && now.duration_since(self.last_dock) > Duration::from_secs(2) {
                self.dock_tray();
            }
            let animating = self.active
                && (self.ink.is_drawing()
                    || !self.ink.strokes.is_empty()
                    || self
                        .pressed
                        .is_some_and(|t| now.duration_since(t) < Duration::from_millis(300)));
            if self.active && (animating || old != self.ink.strokes.len()) {
                self.draw();
            }
            let mut pfd = PollFd {
                fd,
                events: 1,
                revents: 0,
            };
            poll(&mut pfd, 1, if animating { 33 } else { 500 });
        }
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            self.deactivate();
            self.unregister_keys();
            // The renderer must release its Xlib surface before the connection closes.
            // It is held until struct teardown, so explicitly flush before destroying the drawable.
            cairo_surface_flush(self.renderer.surface);
            cairo_surface_destroy(self.renderer.surface);
            self.renderer.surface = ptr::null_mut();
            XDestroyWindow(self.display, self.window);
            XDestroyWindow(self.display, self.tray);
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
