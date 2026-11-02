#![allow(non_snake_case, non_camel_case_types, dead_code)]
use std::ffi::{c_char, c_double, c_int, c_long, c_uint, c_ulong, c_void};

pub type Display = c_void;
pub type Visual = c_void;
pub type Window = c_ulong;
pub type Pixmap = c_ulong;
pub type Cursor = c_ulong;
pub type Colormap = c_ulong;
pub type Drawable = c_ulong;
pub type Cairo = c_void;
pub type Surface = c_void;

#[repr(C)]
#[derive(Default)]
pub struct XSetWindowAttributes {
    pub background_pixmap: c_ulong,
    pub background_pixel: c_ulong,
    pub border_pixmap: c_ulong,
    pub border_pixel: c_ulong,
    pub bit_gravity: c_int,
    pub win_gravity: c_int,
    pub backing_store: c_int,
    pub backing_planes: c_ulong,
    pub backing_pixel: c_ulong,
    pub save_under: c_int,
    pub event_mask: c_long,
    pub do_not_propagate_mask: c_long,
    pub override_redirect: c_int,
    pub colormap: Colormap,
    pub cursor: Cursor,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct XColor {
    pub pixel: c_ulong,
    pub red: u16,
    pub green: u16,
    pub blue: u16,
    pub flags: u8,
    pub pad: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XButtonEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: c_ulong,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub button: c_uint,
    pub same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XMotionEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: c_ulong,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub is_hint: c_char,
    pub same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XKeyEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: c_ulong,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub keycode: c_uint,
    pub same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XConfigureEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub event: Window,
    pub window: Window,
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    pub border_width: c_int,
    pub above: Window,
    pub override_redirect: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XClientMessageEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub window: Window,
    pub message_type: c_ulong,
    pub format: c_int,
    pub data: [c_long; 5],
}

#[repr(C)]
pub struct XErrorEvent {
    pub type_: c_int,
    pub display: *mut Display,
    pub resourceid: c_ulong,
    pub serial: c_ulong,
    pub error_code: u8,
    pub request_code: u8,
    pub minor_code: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union XEvent {
    pub type_: c_int,
    pub button: XButtonEvent,
    pub motion: XMotionEvent,
    pub key: XKeyEvent,
    pub configure: XConfigureEvent,
    pub client: XClientMessageEvent,
    pub pad: [c_long; 24],
}

impl Default for XEvent {
    fn default() -> Self {
        Self { pad: [0; 24] }
    }
}

#[repr(C)]
pub struct PollFd {
    pub fd: c_int,
    pub events: i16,
    pub revents: i16,
}

#[link(name = "X11")]
extern "C" {
    pub fn XOpenDisplay(name: *const c_char) -> *mut Display;
    pub fn XSetErrorHandler(
        handler: Option<extern "C" fn(*mut Display, *mut XErrorEvent) -> c_int>,
    ) -> Option<extern "C" fn(*mut Display, *mut XErrorEvent) -> c_int>;
    pub fn XSync(d: *mut Display, discard: c_int) -> c_int;
    pub fn XCloseDisplay(d: *mut Display) -> c_int;
    pub fn XDefaultScreen(d: *mut Display) -> c_int;
    pub fn XRootWindow(d: *mut Display, s: c_int) -> Window;
    pub fn XDisplayWidth(d: *mut Display, s: c_int) -> c_int;
    pub fn XDisplayHeight(d: *mut Display, s: c_int) -> c_int;
    pub fn XMatchVisualInfo(
        d: *mut Display,
        s: c_int,
        depth: c_int,
        class: c_int,
        info: *mut XVisualInfo,
    ) -> c_int;
    pub fn XCreateColormap(d: *mut Display, w: Window, v: *mut Visual, alloc: c_int) -> Colormap;
    pub fn XFreeColormap(d: *mut Display, c: Colormap) -> c_int;
    pub fn XCreateWindow(
        d: *mut Display,
        parent: Window,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
        border: c_uint,
        depth: c_int,
        class: c_uint,
        v: *mut Visual,
        mask: c_ulong,
        attrs: *mut XSetWindowAttributes,
    ) -> Window;
    pub fn XDestroyWindow(d: *mut Display, w: Window) -> c_int;
    pub fn XMapRaised(d: *mut Display, w: Window) -> c_int;
    pub fn XUnmapWindow(d: *mut Display, w: Window) -> c_int;
    pub fn XRaiseWindow(d: *mut Display, w: Window) -> c_int;
    pub fn XMoveResizeWindow(
        d: *mut Display,
        w: Window,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    pub fn XSelectInput(d: *mut Display, w: Window, mask: c_long) -> c_int;
    pub fn XPending(d: *mut Display) -> c_int;
    pub fn XNextEvent(d: *mut Display, e: *mut XEvent) -> c_int;
    pub fn XFlush(d: *mut Display) -> c_int;
    pub fn XConnectionNumber(d: *mut Display) -> c_int;
    pub fn XGrabPointer(
        d: *mut Display,
        w: Window,
        owner_events: c_int,
        mask: c_uint,
        pointer_mode: c_int,
        keyboard_mode: c_int,
        confine: Window,
        cursor: Cursor,
        time: c_ulong,
    ) -> c_int;
    pub fn XUngrabPointer(d: *mut Display, time: c_ulong) -> c_int;
    pub fn XGrabKey(
        d: *mut Display,
        keycode: c_int,
        modifiers: c_uint,
        w: Window,
        owner: c_int,
        pointer_mode: c_int,
        keyboard_mode: c_int,
    ) -> c_int;
    pub fn XUngrabKey(d: *mut Display, keycode: c_int, modifiers: c_uint, w: Window) -> c_int;
    pub fn XKeysymToKeycode(d: *mut Display, key: c_ulong) -> u8;
    pub fn XKeycodeToKeysym(d: *mut Display, key: u8, index: c_int) -> c_ulong;
    pub fn XKeysymToString(key: c_ulong) -> *const c_char;
    pub fn XStringToKeysym(name: *const c_char) -> c_ulong;
    pub fn XGrabKeyboard(
        d: *mut Display,
        w: Window,
        owner: c_int,
        pointer_mode: c_int,
        keyboard_mode: c_int,
        time: c_ulong,
    ) -> c_int;
    pub fn XUngrabKeyboard(d: *mut Display, time: c_ulong) -> c_int;
    pub fn XQueryPointer(
        d: *mut Display,
        w: Window,
        root: *mut Window,
        child: *mut Window,
        rx: *mut c_int,
        ry: *mut c_int,
        wx: *mut c_int,
        wy: *mut c_int,
        mask: *mut c_uint,
    ) -> c_int;
    pub fn XCreateBitmapFromData(
        d: *mut Display,
        w: Window,
        bits: *const c_char,
        width: c_uint,
        height: c_uint,
    ) -> Pixmap;
    pub fn XCreatePixmapCursor(
        d: *mut Display,
        source: Pixmap,
        mask: Pixmap,
        fg: *mut XColor,
        bg: *mut XColor,
        x: c_uint,
        y: c_uint,
    ) -> Cursor;
    pub fn XFreePixmap(d: *mut Display, p: Pixmap) -> c_int;
    pub fn XFree(data: *mut c_void) -> c_int;
    pub fn XFreeCursor(d: *mut Display, c: Cursor) -> c_int;
    pub fn XStoreName(d: *mut Display, w: Window, name: *const c_char) -> c_int;
    pub fn XInternAtom(d: *mut Display, name: *const c_char, only: c_int) -> c_ulong;
    pub fn XGetSelectionOwner(d: *mut Display, selection: c_ulong) -> Window;
    pub fn XSendEvent(
        d: *mut Display,
        w: Window,
        propagate: c_int,
        mask: c_long,
        event: *mut XEvent,
    ) -> c_int;
    pub fn XChangeProperty(
        d: *mut Display,
        w: Window,
        property: c_ulong,
        typ: c_ulong,
        format: c_int,
        mode: c_int,
        data: *const u8,
        n: c_int,
    ) -> c_int;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XVisualInfo {
    pub visual: *mut Visual,
    pub visualid: c_ulong,
    pub screen: c_int,
    pub depth: c_int,
    pub class: c_int,
    pub red_mask: c_ulong,
    pub green_mask: c_ulong,
    pub blue_mask: c_ulong,
    pub colormap_size: c_int,
    pub bits_per_rgb: c_int,
}
impl Default for XVisualInfo {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
pub struct XRenderDirectFormat {
    pub red: u16,
    pub red_mask: u16,
    pub green: u16,
    pub green_mask: u16,
    pub blue: u16,
    pub blue_mask: u16,
    pub alpha: u16,
    pub alpha_mask: u16,
}
#[repr(C)]
pub struct XRenderPictFormat {
    pub id: c_ulong,
    pub kind: c_int,
    pub depth: c_int,
    pub direct: XRenderDirectFormat,
    pub colormap: Colormap,
}
#[link(name = "Xrender")]
extern "C" {
    pub fn XRenderFindVisualFormat(d: *mut Display, v: *mut Visual) -> *mut XRenderPictFormat;
}

#[repr(C)]
pub struct XineramaScreenInfo {
    pub screen_number: c_int,
    pub x_org: i16,
    pub y_org: i16,
    pub width: i16,
    pub height: i16,
}
#[link(name = "Xinerama")]
extern "C" {
    pub fn XineramaIsActive(d: *mut Display) -> c_int;
    pub fn XineramaQueryScreens(d: *mut Display, count: *mut c_int) -> *mut XineramaScreenInfo;
}

#[link(name = "Xext")]
extern "C" {
    pub fn XShapeCombineRectangles(
        d: *mut Display,
        w: Window,
        dest: c_int,
        x: c_int,
        y: c_int,
        rects: *const c_void,
        n: c_int,
        op: c_int,
        ordering: c_int,
    );
}

#[link(name = "cairo")]
extern "C" {
    pub fn cairo_xlib_surface_create(
        d: *mut Display,
        w: Drawable,
        v: *mut Visual,
        width: c_int,
        height: c_int,
    ) -> *mut Surface;
    pub fn cairo_xlib_surface_set_size(s: *mut Surface, width: c_int, height: c_int);
    pub fn cairo_image_surface_create_from_png(path: *const c_char) -> *mut Surface;
    pub fn cairo_image_surface_create(format: c_int, width: c_int, height: c_int) -> *mut Surface;
    pub fn cairo_image_surface_get_data(s: *mut Surface) -> *mut u8;
    pub fn cairo_image_surface_get_stride(s: *mut Surface) -> c_int;
    pub fn cairo_surface_write_to_png(s: *mut Surface, path: *const c_char) -> c_int;
    pub fn cairo_surface_status(s: *mut Surface) -> c_int;
    pub fn cairo_image_surface_get_width(s: *mut Surface) -> c_int;
    pub fn cairo_image_surface_get_height(s: *mut Surface) -> c_int;
    pub fn cairo_surface_flush(s: *mut Surface);
    pub fn cairo_surface_destroy(s: *mut Surface);
    pub fn cairo_create(s: *mut Surface) -> *mut Cairo;
    pub fn cairo_destroy(c: *mut Cairo);
    pub fn cairo_set_operator(c: *mut Cairo, op: c_int);
    pub fn cairo_set_source_rgba(c: *mut Cairo, r: c_double, g: c_double, b: c_double, a: c_double);
    pub fn cairo_set_source_surface(c: *mut Cairo, s: *mut Surface, x: c_double, y: c_double);
    pub fn cairo_paint(c: *mut Cairo);
    pub fn cairo_paint_with_alpha(c: *mut Cairo, a: c_double);
    pub fn cairo_move_to(c: *mut Cairo, x: c_double, y: c_double);
    pub fn cairo_new_path(c: *mut Cairo);
    pub fn cairo_close_path(c: *mut Cairo);
    pub fn cairo_line_to(c: *mut Cairo, x: c_double, y: c_double);
    pub fn cairo_stroke(c: *mut Cairo);
    pub fn cairo_fill(c: *mut Cairo);
    pub fn cairo_arc(
        c: *mut Cairo,
        x: c_double,
        y: c_double,
        radius: c_double,
        a1: c_double,
        a2: c_double,
    );
    pub fn cairo_rectangle(
        c: *mut Cairo,
        x: c_double,
        y: c_double,
        width: c_double,
        height: c_double,
    );
    pub fn cairo_set_line_width(c: *mut Cairo, width: c_double);
    pub fn cairo_set_line_cap(c: *mut Cairo, cap: c_int);
    pub fn cairo_set_line_join(c: *mut Cairo, join: c_int);
    pub fn cairo_save(c: *mut Cairo);
    pub fn cairo_restore(c: *mut Cairo);
    pub fn cairo_translate(c: *mut Cairo, x: c_double, y: c_double);
    pub fn cairo_scale(c: *mut Cairo, x: c_double, y: c_double);
    pub fn cairo_rotate(c: *mut Cairo, angle: c_double);
    pub fn cairo_select_font_face(
        c: *mut Cairo,
        family: *const c_char,
        slant: c_int,
        weight: c_int,
    );
    pub fn cairo_set_font_size(c: *mut Cairo, size: c_double);
    pub fn cairo_show_text(c: *mut Cairo, text: *const c_char);
}

#[link(name = "c")]
extern "C" {
    pub fn poll(fds: *mut PollFd, count: c_ulong, timeout: c_int) -> c_int;
}
