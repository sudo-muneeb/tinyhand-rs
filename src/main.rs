use gtk::prelude::*;
use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tinyhand_rs::config::{self, Config};
use tinyhand_rs::ffi::*;
use tinyhand_rs::ink::{Brush, Ink, Point};
use tinyhand_rs::render::{asset_dir, Renderer};
use tinyhand_rs::sound::Sound;
use tinyhand_rs::{
    desktop::{self, Event},
    hands::Library,
    native_settings::SettingsWindow,
};

static STOP: AtomicBool = AtomicBool::new(false);
static LAST_X_ERROR: AtomicU8 = AtomicU8::new(0);

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
        tinyhand_rs::logging::write(
            "x11_error",
            &format!(
                "code={} request={}",
                (*error).error_code,
                (*error).request_code
            ),
        );
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
    cmap: Colormap,
    invisible: Cursor,
    arrow: Cursor,
    interaction_cursor: bool,
    renderer: Renderer,
    config: Config,
    ink: Ink,
    sound: Sound,
    cursor: Point,
    pressed: Option<Instant>,
    active: bool,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    preferences: SettingsWindow,
    library: Library,
    hotkeys_suspended: bool,
    preparing: bool,
    trace: bool,
    status_item: Option<tinyhand_rs::tray::Tray>,
    input: tinyhand_rs::input::Input,
    passing: bool,
    restore_at: Option<Instant>,
    cursor_hidden: bool,
    revealed: Instant,
    hint_until: Instant,
    toolbar_drag: Option<(String, Point)>,
    instance: tinyhand_rs::instance::Instance,
    page: usize,
    quit: bool,
    monitors: Vec<(i32, i32, i32, i32)>,
    active_monitor: Option<(i32, i32, i32, i32)>,
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
            "control" | "ctrl" => mods |= 4,
            "alt" | "option" => mods |= 8,
            "super" | "command" | "meta" => mods |= 64,
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
    unsafe fn new(instance: tinyhand_rs::instance::Instance) -> Result<Self, String> {
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
        XStoreName(display, window, c"TinyHand overlay".as_ptr());
        XShapeCombineRectangles(display, window, 2, 0, 0, ptr::null(), 0, 0, 0);
        XSelectInput(display, root, 1 << 17);
        let bitmap = XCreateBitmapFromData(display, root, c"".as_ptr(), 1, 1);
        let mut black = XColor::default();
        let invisible = XCreatePixmapCursor(display, bitmap, bitmap, &mut black, &mut black, 0, 0);
        XFreePixmap(display, bitmap);
        let mut config = Config::load(&Config::path());
        let library = Library::load();
        config
            .pinned
            .retain(|id| !id.starts_with("custom-") || library.hand(id).is_some());
        if config.pinned.is_empty() {
            config.pinned = config::default_pins();
        }
        if let Some(hand) = library.hand(&config.custom_id) {
            config.custom_path = Library::path(&hand.id).to_string_lossy().into_owned();
        }
        let (tx, rx) = mpsc::channel();
        let preferences = SettingsWindow::new(tx.clone());
        let status_item = tinyhand_rs::tray::Tray::new(tx.clone())
            .map_err(|e| eprintln!("Status notifier: {e}"))
            .ok();
        let renderer = match Renderer::new(display, window, vi.visual, w, h, asset_dir()?, &config)
        {
            Ok(r) => r,
            Err(e) => {
                XDestroyWindow(display, window);
                XFreeColormap(display, cmap);
                XCloseDisplay(display);
                return Err(e);
            }
        };
        let toggle_key = shortcut(display, &config.toggle);
        let clear_key = shortcut(display, &config.clear);
        let input = tinyhand_rs::input::Input::new(display, root)?;
        let mut app = Self {
            display,
            root,
            window,
            cmap,
            invisible,
            arrow: XCreateFontCursor(display, 68),
            interaction_cursor: false,
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
            tx,
            rx,
            preferences,
            library,
            hotkeys_suspended: false,
            preparing: false,
            trace: std::env::var_os("TINYHAND_TRACE_INPUT").is_some(),
            status_item,
            input,
            passing: false,
            restore_at: None,
            cursor_hidden: false,
            revealed: Instant::now(),
            hint_until: Instant::now(),
            toolbar_drag: None,
            instance,
            page: 0,
            quit: false,
            monitors: vec![(0, 0, w, h)],
            active_monitor: None,
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
        if app.config.custom_enabled && app.config.custom_id.is_empty() {
            if let Ok(png) = std::fs::read(&app.config.custom_path) {
                let (r, g, b) = app.renderer.ink_color(&Config {
                    match_color: true,
                    ..app.config.clone()
                });
                let ink =
                    ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32;
                match app.library.add(tinyhand_rs::hands::PreparedHand {
                    png,
                    ink,
                    name: "Imported hand".into(),
                }) {
                    Ok(id) => {
                        app.config.custom_path = Library::path(&id).to_string_lossy().into_owned();
                        app.config.custom_id = id.clone();
                        if app.config.pinned.len() < 8 {
                            app.config.pinned.push(id);
                        }
                        app.save();
                        let _ = app.renderer.select_config(&app.config);
                    }
                    Err(e) => tinyhand_rs::logging::write("legacy_import", &e),
                }
            }
        }
        if app.config.custom_enabled {
            if let Some(hand) = app.library.hand(&app.config.custom_id) {
                app.renderer.set_custom_ink(hand.ink);
            }
        }
        app.renderer.warm_toolbar(&app.config);
        app.register_keys();
        XSync(display, 0);
        app.query_pointer();
        app.refresh_monitors();
        Ok(app)
    }

    unsafe fn register_keys(&self) -> bool {
        let mut success = true;
        for (name, (key, mods)) in [
            (&self.config.toggle, self.toggle_key),
            (&self.config.clear, self.clear_key),
        ] {
            for extra in [0, 2, 16, 18] {
                LAST_X_ERROR.store(0, Ordering::SeqCst);
                XGrabKey(self.display, key, mods | extra, self.root, 0, 1, 1);
                XSync(self.display, 0);
                if LAST_X_ERROR.load(Ordering::SeqCst) == 10 {
                    eprintln!("tinyhand-rs: shortcut {name} is already in use");
                    success = false;
                }
            }
        }
        success
    }
    unsafe fn unregister_keys(&self) {
        for (key, mods) in [self.toggle_key, self.clear_key] {
            for extra in [0, 2, 16, 18] {
                XUngrabKey(self.display, key, mods | extra, self.root);
            }
        }
    }

    unsafe fn query_pointer(&mut self) -> u32 {
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
        mask
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
        if self.active_monitor.is_some_and(|old| old != m) {
            self.ink.clear();
            self.renderer.pointer_up();
            self.toolbar_drag = None;
        }
        self.active_monitor = Some(m);
        self.renderer.toolbar_width = tinyhand_rs::toolbar::width(&self.config);
        self.renderer.place_bar_with_work_area(m, self.work_area);
        if let Err(e) = unsafe { self.renderer.select_config(&self.config) } {
            tinyhand_rs::logging::write("asset", &e);
        }
    }

    unsafe fn activate(&mut self) -> Result<(), String> {
        if self.active {
            return Ok(());
        }
        if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
            return Err("Presentation needs an X11 login session. XWayland cannot capture input across Wayland applications.".into());
        }
        self.renderer.warm_toolbar(&self.config);
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
        self.input.buttons = 0;
        self.interaction_cursor = false;
        self.active = true;
        tinyhand_rs::logging::write("presentation", "started");
        if self.config.tap_sound_enabled() {
            self.sound.warm();
        }
        self.revealed = Instant::now() + Duration::from_secs(3);
        self.hint_until = Instant::now() + Duration::from_secs(9);
        self.renderer.toolbar_alpha = 1.0;
        self.renderer.hint = true;
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
        if let Some(tray) = &self.status_item {
            tray.set_active(self.active);
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
        tinyhand_rs::logging::write("presentation", "stopped");
        self.input.restore(self.display);
        self.passing = false;
        self.restore_at = None;
        if self.cursor_hidden {
            tinyhand_rs::input::XFixesShowCursor(self.display, self.root);
            self.cursor_hidden = false;
        }
        self.renderer.pointer_visible = true;
        self.renderer.reset_motion();
        self.redraw_pending = false;
        self.pressed = None;
        self.ink.end(Instant::now());
        self.ink.clear();
        self.page = 0;
        XUngrabKeyboard(self.display, 0);
        XUngrabPointer(self.display, 0);
        let escape = XKeysymToKeycode(self.display, 0xff1b) as i32;
        for extra in [0, 2, 16, 18] {
            XUngrabKey(self.display, escape, extra, self.root);
        }
        XUnmapWindow(self.display, self.window);
        if let Some(tray) = &self.status_item {
            tray.set_active(self.active);
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
            let begin = Instant::now();
            self.renderer.paint(
                &self.config,
                &self.ink,
                self.cursor,
                self.page,
                self.pressed,
                now,
            );
            XFlush(self.display);
            if self.trace {
                eprintln!(
                    "frame at {},{} took {}ms, strokes={}",
                    self.cursor.x,
                    self.cursor.y,
                    begin.elapsed().as_millis(),
                    self.ink.strokes.len()
                );
            }
            self.last_render = now;
            self.redraw_pending = false;
        }
    }

    fn bar_hit(&self, p: Point) -> Option<tinyhand_rs::toolbar::Item> {
        if self.renderer.toolbar_alpha < 0.2 {
            return None;
        }
        tinyhand_rs::toolbar::hit(
            &self.config,
            (p.x - self.renderer.bar_x) / self.renderer.ui_scale,
            (p.y - self.renderer.bar_y) / self.renderer.ui_scale,
        )
    }
    unsafe fn open_settings(&mut self) {
        self.deactivate();
        self.preferences.show(&self.config, &self.library);
    }

    unsafe fn configure(&mut self, next: Config) {
        let old = self.config.clone();
        self.unregister_keys();
        self.config = next;
        self.toggle_key = shortcut(self.display, &self.config.toggle);
        self.clear_key = shortcut(self.display, &self.config.clear);
        if self.toggle_key == self.clear_key || !self.register_keys() {
            self.unregister_keys();
            self.config.toggle = old.toggle.clone();
            self.config.clear = old.clear.clone();
            self.toggle_key = shortcut(self.display, &self.config.toggle);
            self.clear_key = shortcut(self.display, &self.config.clear);
            self.register_keys();
            self.preferences.show(&self.config, &self.library);
            self.preferences
                .message("That shortcut is already in use. The previous shortcuts were restored.");
        }
        if old.startup != self.config.startup {
            if let Err(e) = std::env::current_exe()
                .and_then(|exe| config::update_autostart(self.config.startup, &exe))
            {
                self.config.startup = old.startup;
                self.preferences
                    .message(&format!("Cannot change launch at login: {e}"));
            }
        }
        if let Err(e) = self.renderer.select_config(&self.config) {
            self.config = old.clone();
            let _ = self.renderer.select_config(&self.config);
            self.preferences.message(&e);
        }
        if self.config.custom_enabled {
            if let Some(hand) = self.library.hand(&self.config.custom_id) {
                self.renderer.set_custom_ink(hand.ink);
            }
        }
        self.preferences.sync(&self.config);
        if self.preferences.visible() && self.config.pinned != old.pinned {
            self.preferences.show(&self.config, &self.library);
        }
        self.save();
        self.draw();
    }

    unsafe fn desktop_event(&mut self, event: Event) {
        match event {
            Event::Configure(c) => self.configure(c),
            Event::Pick(id) => {
                let mut c = self.config.clone();
                if let Some(index) = config::STYLES.iter().position(|s| *s == id) {
                    c.style = index;
                    c.custom_enabled = false;
                } else if self.library.hand(&id).is_some() {
                    c.custom_enabled = true;
                    c.custom_id = id.clone();
                    c.custom_path = Library::path(&id).to_string_lossy().into_owned();
                } else {
                    return;
                }
                self.configure(c);
            }
            Event::Swap(old, new) => {
                if !self.config.pinned.contains(&new)
                    && (config::STYLES.contains(&new.as_str()) || self.library.hand(&new).is_some())
                {
                    if let Some(slot) = self.config.pinned.iter().position(|p| *p == old) {
                        self.config.pinned[slot] = new;
                        self.save();
                        if self.preferences.visible() {
                            self.preferences.show(&self.config, &self.library);
                        }
                        self.draw();
                    }
                }
            }
            Event::Pin(id) => {
                if self.config.pinned.len() >= 8 {
                    self.preferences
                        .message("The toolbar holds up to eight pointers. Remove one first.");
                    return;
                }
                if !self.config.pinned.contains(&id)
                    && (config::STYLES.contains(&id.as_str()) || self.library.hand(&id).is_some())
                {
                    self.config.pinned.push(id);
                    self.save();
                    if self.preferences.visible() {
                        self.preferences.show(&self.config, &self.library);
                    }
                    self.draw();
                }
            }
            Event::Unpin(id) => {
                if self.config.pinned.len() > 1 {
                    self.config.pinned.retain(|p| *p != id);
                    self.save();
                    if self.preferences.visible() {
                        self.preferences.show(&self.config, &self.library);
                    }
                    self.draw();
                } else {
                    self.preferences
                        .message("Keep at least one pointer in the toolbar.");
                }
            }
            Event::Reorder(id, slot) => {
                if let Some(index) = self.config.pinned.iter().position(|p| *p == id) {
                    self.config.pinned.remove(index);
                    let slot = slot.min(self.config.pinned.len());
                    self.config.pinned.insert(slot, id);
                    self.save();
                    if self.preferences.visible() {
                        self.preferences.show(&self.config, &self.library);
                    }
                    self.draw();
                }
            }
            Event::Prepare(path, name) => {
                if self.preparing {
                    self.preferences
                        .message("An image is still being prepared. Please wait.");
                    return;
                }
                self.preparing = true;
                self.preferences
                    .message("Checking and preparing your hand…");
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(Event::Prepared(tinyhand_rs::hands::prepare(&path, &name)));
                    gtk::glib::MainContext::default().wakeup();
                });
            }
            Event::Prepared(result) => {
                self.preparing = false;
                match result {
                    Ok(hand) => self.preferences.show_draft(hand),
                    Err(e) => self.preferences.message(&e),
                }
            }
            Event::AddHand(hand) => match self.library.add(hand) {
                Ok(id) => {
                    if self.config.pinned.len() < 8 {
                        self.config.pinned.push(id.clone());
                    }
                    self.desktop_event(Event::Pick(id));
                    self.preferences.show(&self.config, &self.library);
                    self.preferences
                        .message("Your hand is ready in the library.");
                }
                Err(e) => self.preferences.message(&e),
            },
            Event::RemoveHand(id) => match self.library.remove(&id) {
                Ok(()) => {
                    self.config.pinned.retain(|p| *p != id);
                    if self.config.pinned.is_empty() {
                        self.config.pinned.push("lavender".into());
                    }
                    if self.config.custom_id == id {
                        self.config.custom_enabled = false;
                        self.config.custom_id.clear();
                        self.config.custom_path.clear();
                        let _ = self.renderer.select_config(&self.config);
                    }
                    self.save();
                    self.preferences.show(&self.config, &self.library);
                }
                Err(e) => self.preferences.message(&e),
            },
            Event::Toggle => {
                if self.active {
                    self.deactivate();
                } else {
                    self.desktop_event(Event::Start);
                }
            }
            Event::Start => {
                self.preferences.window.hide();
                if let Err(e) = self.activate() {
                    self.open_settings();
                    self.preferences.message(&e);
                }
            }
            Event::Stop => self.deactivate(),
            Event::Settings => self.open_settings(),
            Event::Quit => self.quit = true,
            Event::Clear => {
                self.ink.clear();
                self.draw();
            }
            Event::Mute => {
                if self.config.style == 36 && !self.config.custom_enabled {
                    self.config.laser_sound = !self.config.laser_sound;
                } else {
                    self.config.sound = !self.config.sound;
                }
                self.save();
                self.draw();
            }
            Event::Reset => {
                let c = Config {
                    startup: self.config.startup,
                    welcomed: true,
                    ..Config::default()
                };
                self.configure(c);
                self.preferences.show(&self.config, &self.library);
            }
            Event::Message(s) => {
                if let Some(kind) = s
                    .strip_prefix("preview-sound:")
                    .and_then(|n| n.parse::<usize>().ok())
                {
                    self.sound.tap(self.config.volume, kind);
                } else {
                    self.preferences.message(&s);
                }
            }
            Event::SuspendHotkeys(yes) => {
                self.hotkeys_suspended = yes;
                if yes {
                    self.unregister_keys();
                } else {
                    self.register_keys();
                }
            }
        }
        if self.active {
            self.place_bar();
        }
        if let Some(tray) = &self.status_item {
            tray.set_active(self.active);
        }
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
            self.config.custom_enabled = false;
            self.config.style = (self.config.style + if button == 4 { 36 } else { 1 }) % 37;
            if let Err(e) = self.renderer.select_config(&self.config) {
                eprintln!("{e}");
            }
            self.save();
            self.draw();
            return;
        }
        if !(1..=3).contains(&button) {
            return;
        }
        self.hint_until = Instant::now();
        self.ink.end(Instant::now());
        self.renderer.pointer_down();

        let px = (p.x - self.renderer.bar_x) / self.renderer.ui_scale;
        let py = (p.y - self.renderer.bar_y) / self.renderer.ui_scale;
        if self.page > 0 && (-62.0..-8.0).contains(&py) && (0.0..366.0).contains(&px) {
            let i = ((px - 3.0) / 40.0).floor().max(0.0) as usize;
            if i < 9 {
                self.config.match_color = i == 0;
                if i > 0 {
                    self.config.color = tinyhand_rs::colors::SWATCH_ORDER[i - 1];
                }
                self.save();
            }
            self.page = 0;
        } else if let Some(item) = self.bar_hit(p) {
            use tinyhand_rs::toolbar::Item;
            match item {
                Item::Pointer(id) => {
                    self.toolbar_drag = Some((id.clone(), p));
                    self.desktop_event(Event::Pick(id));
                }
                Item::Ink => self.page = usize::from(self.page == 0),
                Item::Sound => self.desktop_event(Event::Mute),
                Item::Clear => self.desktop_event(Event::Clear),
                Item::Settings => self.open_settings(),
                Item::Exit => self.deactivate(),
            }
        } else {
            self.page = 0;
            if button == 1 {
                self.ink.down_with_brush(
                    p,
                    Brush {
                        color: self.renderer.ink_color(&self.config),
                        width: self.config.thickness,
                        fade_after: self.config.fade,
                    },
                );
            }
            self.pressed = Some(Instant::now());
            if self.config.tap_sound_enabled() {
                self.sound.tap(self.config.volume, self.config.sound_kind);
            }
        }
        self.draw();
    }

    unsafe fn handle(&mut self, mut e: XEvent) {
        if self.input.raw(self.display, &mut e) {
            if self.active && self.passing {
                self.query_pointer();
                self.draw();
            }
            return;
        }
        match e.type_ {
            2 => {
                let k = e.key;
                if k.keycode == XKeysymToKeycode(self.display, 0xff1b) as u32 && self.active {
                    self.deactivate();
                    return;
                }
                let mods = k.state & (1 | 4 | 8 | 64);
                if (k.keycode as i32, mods) == self.toggle_key {
                    self.desktop_event(Event::Toggle);
                } else if (k.keycode as i32, mods) == self.clear_key {
                    self.ink.clear();
                    self.draw();
                }
            }
            4 if self.active => {
                let b = e.button;
                let interaction = b.state & self.config.interaction_mask() != 0
                    || self
                        .input
                        .held(self.display, self.config.interaction_mask());
                if self.trace {
                    eprintln!(
                        "press {} at {},{} state={} interaction={} passing={}",
                        b.button, b.x_root, b.y_root, b.state, interaction, self.passing
                    );
                }
                if !self.passing
                    && (interaction
                        || tinyhand_rs::input::desktop_target(
                            self.display,
                            self.root,
                            self.window,
                            b.x_root,
                            b.y_root,
                        ))
                {
                    if self.trace {
                        eprintln!("pass through target");
                    }
                    self.ink.end(Instant::now());
                    self.renderer.pointer_up();
                    self.passing = true;
                    self.restore_at = None;
                    self.renderer.pointer_visible = interaction && self.config.interact_keeps_hand;
                    XUngrabPointer(self.display, 0);
                    let escape = XKeysymToKeycode(self.display, 0xff1b) as i32;
                    for extra in [0, 2, 16, 18] {
                        XUngrabKey(self.display, escape, extra, self.root);
                    }
                    if self.renderer.pointer_visible {
                        tinyhand_rs::input::XFixesHideCursor(self.display, self.root);
                        self.cursor_hidden = true;
                    }
                    self.input.replay(
                        self.display,
                        b.button,
                        if interaction {
                            self.config.interaction_mask()
                        } else {
                            0
                        },
                    );
                    self.draw();
                    return;
                }
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
                if let Some((id, start)) = self.toolbar_drag.take() {
                    let end = Point {
                        x: b.x_root as f64,
                        y: b.y_root as f64,
                    };
                    if start.distance(end) > 12.0 {
                        if self.bar_hit(end).is_some() {
                            let slot =
                                ((end.x - self.renderer.bar_x) / self.renderer.ui_scale / 44.0)
                                    .max(0.0) as usize;
                            self.desktop_event(Event::Reorder(id, slot));
                        } else {
                            self.desktop_event(Event::Unpin(id));
                        }
                    }
                }
                self.ink.end(Instant::now());
                if (1..=3).contains(&e.button.button) {
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
                if self.trace {
                    eprintln!(
                        "motion {},{} state={} passing={}",
                        m.x_root, m.y_root, m.state, self.passing
                    );
                }
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
        let present = std::env::args().any(|s| s == "--present" || s == "--toggle");
        if present {
            self.activate()?;
        } else if !std::env::args().any(|s| s == "--background") || !self.config.welcomed {
            self.open_settings();
            if !self.config.welcomed {
                self.config.welcomed = true;
                self.save();
                self.preferences.welcome();
            }
        }
        let fd = XConnectionNumber(self.display);
        while !self.quit && !STOP.load(Ordering::SeqCst) {
            while let Some(command) = self.instance.receive() {
                let event = match command.as_str() {
                    "--background" => continue,
                    "--present" => Event::Start,
                    "--toggle" => Event::Toggle,
                    "--stop" => Event::Stop,
                    "--clear" => Event::Clear,
                    "--quit" => Event::Quit,
                    _ => Event::Settings,
                };
                self.desktop_event(event);
            }
            while let Ok(event) = self.rx.try_recv() {
                self.desktop_event(event);
            }
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
            if XPending(self.display) == 0 {
                self.input.clear_replayed();
            }
            let now = Instant::now();
            if self.active {
                if !self.passing {
                    let interaction = self
                        .input
                        .held(self.display, self.config.interaction_mask());
                    if interaction != self.interaction_cursor {
                        self.interaction_cursor = interaction;
                        self.renderer.pointer_visible =
                            !interaction || self.config.interact_keeps_hand;
                        XChangeActivePointerGrab(
                            self.display,
                            (1 << 6) | (1 << 2) | (1 << 3),
                            if interaction && !self.config.interact_keeps_hand {
                                self.arrow
                            } else {
                                self.invisible
                            },
                            0,
                        );
                        self.draw();
                    }
                }
                let x = (self.cursor.x - self.renderer.bar_x) / self.renderer.ui_scale;
                let y = (self.cursor.y - self.renderer.bar_y) / self.renderer.ui_scale;
                if (-70.0..self.renderer.toolbar_width + 70.0).contains(&x)
                    && (-85.0..70.0).contains(&y)
                    && self.input.buttons == 0
                {
                    self.revealed = now + Duration::from_millis(700);
                }
                let target = if !self.ink.is_drawing()
                    && (now < self.revealed || self.page > 0 || self.toolbar_drag.is_some())
                {
                    1.0
                } else {
                    0.0
                };
                let old = self.renderer.toolbar_alpha;
                let dt = now
                    .saturating_duration_since(self.last_render)
                    .as_secs_f64()
                    .min(0.05);
                let next = if target > old {
                    (old + dt / 0.15).min(1.0)
                } else {
                    (old - dt / 0.35).max(0.0)
                };
                let hint = now < self.hint_until;
                if next != old || self.renderer.hint != hint {
                    self.renderer.toolbar_alpha = next;
                    self.renderer.hint = hint;
                    self.draw();
                }
            }
            if self.restore_at.is_some_and(|deadline| now >= deadline)
                && self.input.buttons == 0
                && !self.passing
            {
                self.input.restore(self.display);
                self.restore_at = None;
            }
            if self.active
                && self.passing
                && self.input.buttons == 0
                && self.query_pointer() & (31 << 8) == 0
            {
                let mask = (1 << 6) | (1 << 2) | (1 << 3);
                if XGrabPointer(self.display, self.root, 0, mask, 1, 1, 0, self.invisible, 0) == 0 {
                    self.passing = false;
                    // Raw releases precede delivery of the cooked mouse-up.
                    // Restore modifiers after that event has reached its app.
                    self.restore_at = Some(now + Duration::from_millis(25));
                    self.interaction_cursor = false;
                    self.renderer.pointer_visible = true;
                    let escape = XKeysymToKeycode(self.display, 0xff1b) as i32;
                    for extra in [0, 2, 16, 18] {
                        XGrabKey(self.display, escape, extra, self.root, 0, 1, 1);
                    }
                    if self.cursor_hidden {
                        tinyhand_rs::input::XFixesShowCursor(self.display, self.root);
                        self.cursor_hidden = false;
                    }
                    self.query_pointer();
                    self.draw();
                }
            }
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
            let animating = self.active
                && ((self.renderer.toolbar_alpha > 0.0 && self.renderer.toolbar_alpha < 1.0)
                    || self.renderer.motion_animating()
                    || self.ink.needs_animation(now)
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
            let mut wait = self
                .ink
                .next_change(now)
                .map_or(wait, |d| wait.min(d.as_millis().max(1) as u64));
            if let Some(deadline) = self.restore_at {
                if deadline > now {
                    wait = wait.min((deadline - now).as_millis().max(1) as u64);
                }
            }
            if self.active {
                for deadline in [self.revealed, self.hint_until] {
                    if deadline > now {
                        wait = wait.min((deadline - now).as_millis().max(1) as u64);
                    }
                }
            }
            desktop::wait(fd, &[self.instance.fd()], wait.min(500) as i32);
        }
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            self.deactivate();
            self.input.finish(self.display);
            self.unregister_keys();
            XSync(self.display, 0);
            std::thread::sleep(Duration::from_millis(100));
            self.renderer.release_x11();
            XDestroyWindow(self.display, self.window);
            XFreeCursor(self.display, self.invisible);
            XFreeCursor(self.display, self.arrow);
            XFreeColormap(self.display, self.cmap);
            XCloseDisplay(self.display);
        }
    }
}

fn main() {
    unsafe {
        XInitThreads();
        signal(2, stop_signal);
        signal(15, stop_signal);
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|s| s == "--make-hand") && args.len() == 3 {
        match tinyhand_rs::hands::prepare(std::path::Path::new(&args[1]), "").and_then(|hand| {
            std::fs::write(&args[2], &hand.png)
                .map(|_| hand.ink)
                .map_err(|e| e.to_string())
        }) {
            Ok(ink) => println!("ok {ink:06X}"),
            Err(e) => {
                eprintln!("rejected: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|a| a == "--help") {
        println!("TinyHand for Linux\nUsage: tinyhand-rs [--present|--settings|--toggle|--stop|--clear|--quit]\nNo option opens settings. Escape exits presentation mode.");
        return;
    }
    if args.iter().any(|a| a == "--version") {
        println!("tinyhand-rs {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| {
        ![
            "--present",
            "--background",
            "--settings",
            "--toggle",
            "--stop",
            "--clear",
            "--quit",
        ]
        .contains(&a.as_str())
    }) || args.len() > 1
    {
        eprintln!("Use --help for supported options.");
        std::process::exit(2);
    }
    let command = args.first().map(String::as_str).unwrap_or("--settings");
    let instance = match tinyhand_rs::instance::Instance::open(command) {
        Ok(Some(instance)) => instance,
        Ok(None) => return,
        Err(e) => {
            eprintln!("Cannot start TinyHand: {e}");
            std::process::exit(1);
        }
    };
    if command == "--quit" || command == "--stop" || command == "--clear" {
        return;
    }
    if let Err(e) = gtk::init() {
        eprintln!("Cannot initialize GTK: {e}");
        std::process::exit(1);
    }
    tinyhand_rs::logging::write("startup", concat!("version=", env!("CARGO_PKG_VERSION")));
    let result = unsafe { App::new(instance).and_then(|mut app| app.run()) };
    tinyhand_rs::logging::write("shutdown", if result.is_ok() { "clean" } else { "error" });
    if let Err(e) = result {
        tinyhand_rs::logging::write("fatal", &e);
        eprintln!("tinyhand-rs: {e}");
        std::process::exit(1);
    }
}
