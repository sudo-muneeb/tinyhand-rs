//! XInput2 observes releases even while another application owns the pointer.
use crate::ffi::*;
use std::{
    collections::VecDeque,
    ffi::{c_char, c_int, c_ulong, c_void, CStr},
    ptr,
};
#[repr(C)]
pub struct Cookie {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub extension: c_int,
    pub evtype: c_int,
    pub cookie: u32,
    pub data: *mut c_void,
}
#[repr(C)]
struct Raw {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    extension: c_int,
    evtype: c_int,
    time: c_ulong,
    device: c_int,
    source: c_int,
    detail: c_int,
}
#[repr(C)]
struct EventMask {
    device: c_int,
    len: c_int,
    mask: *mut u8,
}
#[repr(C)]
struct DeviceInfo {
    id: i32,
    name: *mut c_char,
    use_: i32,
    attachment: i32,
    enabled: i32,
    num_classes: i32,
    classes: *mut *mut c_void,
}
#[link(name = "Xi")]
extern "C" {
    fn XIQueryDevice(d: *mut Display, id: i32, n: *mut i32) -> *mut DeviceInfo;
    fn XIFreeDeviceInfo(info: *mut DeviceInfo);
    fn XIQueryVersion(d: *mut Display, major: *mut c_int, minor: *mut c_int) -> c_int;
    fn XISelectEvents(d: *mut Display, w: Window, m: *mut EventMask, n: c_int) -> c_int;
}
#[link(name = "X11")]
extern "C" {
    fn XQueryExtension(
        d: *mut Display,
        n: *const c_char,
        opcode: *mut c_int,
        event: *mut c_int,
        error: *mut c_int,
    ) -> c_int;
    fn XNextRequest(d: *mut Display) -> c_ulong;
    fn XGetEventData(d: *mut Display, c: *mut Cookie) -> c_int;
    fn XFreeEventData(d: *mut Display, c: *mut Cookie);
    pub fn XQueryKeymap(d: *mut Display, keys: *mut c_char) -> c_int;
}
#[link(name = "Xtst")]
extern "C" {
    fn XTestQueryExtension(
        d: *mut Display,
        event: *mut i32,
        error: *mut i32,
        major: *mut i32,
        minor: *mut i32,
    ) -> i32;
    fn XTestFakeButtonEvent(d: *mut Display, b: u32, press: c_int, delay: c_ulong) -> c_int;
    fn XTestFakeKeyEvent(d: *mut Display, k: u32, press: c_int, delay: c_ulong) -> c_int;
}
#[link(name = "Xfixes")]
extern "C" {
    pub fn XFixesHideCursor(d: *mut Display, w: Window);
    pub fn XFixesShowCursor(d: *mut Display, w: Window);
}
pub struct Input {
    opcode: i32,
    keys: [bool; 256],
    virtual_keys: [bool; 256],
    restored: [bool; 256],
    virtual_devices: Vec<i32>,
    own: VecDeque<(i32, i32, c_ulong)>,
    pub buttons: u32,
    suppressed: Vec<u8>,
}
impl Input {
    /// # Safety
    /// A live X11 display on the calling thread is required.
    pub unsafe fn new(d: *mut Display, root: Window) -> Result<Self, String> {
        let (mut opcode, mut event, mut error) = (0, 0, 0);
        if XQueryExtension(
            d,
            c"XInputExtension".as_ptr(),
            &mut opcode,
            &mut event,
            &mut error,
        ) == 0
        {
            return Err("XInput2 is required for safe interaction".into());
        }
        let (mut major, mut minor) = (2, 0);
        if XIQueryVersion(d, &mut major, &mut minor) != 0 {
            return Err("XInput2 version 2 is required".into());
        }
        if XTestQueryExtension(d, &mut event, &mut error, &mut major, &mut minor) == 0 {
            return Err("XTEST is required to forward interaction clicks".into());
        }
        let mut mask = [0_u8; 3];
        for bit in [13, 14, 15, 16, 17] {
            mask[bit / 8] |= 1 << (bit % 8);
        }
        let mut selection = EventMask {
            device: 1,
            len: mask.len() as i32,
            mask: mask.as_mut_ptr(),
        };
        XISelectEvents(d, root, &mut selection, 1);
        let mut map = [0_i8; 32];
        XQueryKeymap(d, map.as_mut_ptr());
        let keys = std::array::from_fn(|i| map[i / 8] as u8 & (1 << (i % 8)) != 0);
        let mut n = 0;
        let devices = XIQueryDevice(d, 0, &mut n);
        let mut virtual_devices = Vec::new();
        if !devices.is_null() {
            for device in std::slice::from_raw_parts(devices, n.max(0) as usize) {
                if !device.name.is_null()
                    && CStr::from_ptr(device.name)
                        .to_string_lossy()
                        .contains("XTEST")
                {
                    virtual_devices.push(device.id);
                }
            }
            XIFreeDeviceInfo(devices);
        }
        Ok(Self {
            opcode,
            virtual_keys: [false; 256],
            restored: [false; 256],
            virtual_devices,
            keys,
            own: VecDeque::new(),
            buttons: 0,
            suppressed: Vec::new(),
        })
    }
    /// # Safety
    /// Event and display must belong to this X connection.
    pub unsafe fn raw(&mut self, d: *mut Display, e: &mut XEvent) -> bool {
        if e.type_ != 35 {
            return false;
        }
        let cookie = (e as *mut XEvent).cast::<Cookie>();
        if (*cookie).extension != self.opcode || XGetEventData(d, cookie) == 0 {
            return false;
        }
        let raw = &*(*cookie).data.cast::<Raw>();
        let (kind, detail) = (raw.evtype, raw.detail);
        if let Some(index) = self.own.iter().position(|p| {
            self.virtual_devices.contains(&raw.source) && *p == (kind, detail, raw.serial)
        }) {
            self.own.remove(index);
        } else if (0..256).contains(&detail) {
            match kind {
                13 => {
                    self.keys[detail as usize] = true;
                    self.virtual_keys[detail as usize] = self.virtual_devices.contains(&raw.source);
                }
                14 => {
                    self.keys[detail as usize] = false;
                    // A physical release can already be queued when restore
                    // injects its matching virtual press. Balance that press
                    // on the XTEST device too, even if the master is now up.
                    if self.restored[detail as usize] {
                        self.restored[detail as usize] = false;
                        self.own.push_back((14, detail, XNextRequest(d)));
                        XTestFakeKeyEvent(d, detail as u32, 0, 0);
                        XSync(d, 0);
                    }
                }
                15 if (1..=5).contains(&detail) => self.buttons |= 1 << (detail - 1),
                16 if (1..=5).contains(&detail) => self.buttons &= !(1 << (detail - 1)),
                _ => {}
            }
        }
        self.own.retain(|p| p.2 >= raw.serial);
        XFreeEventData(d, cookie);
        true
    }
    /// # Safety
    /// A live X connection is required. Called after releasing the pointer grab.
    pub unsafe fn replay(&mut self, d: *mut Display, button: u32, modifier: u32) {
        self.suppressed.clear();
        let names: &[u64] = match modifier {
            8 => &[0xffe9, 0xffea],
            4 => &[0xffe3, 0xffe4],
            64 => &[0xffeb, 0xffec],
            _ => &[],
        };
        for name in names {
            let key = XKeysymToKeycode(d, *name);
            if self.keys[key as usize] {
                self.own.push_back((14, key as i32, XNextRequest(d)));
                XTestFakeKeyEvent(d, key as u32, 0, 0);
                self.suppressed.push(key);
            }
        }
        // The grabbed press already changed the server's button state. Reset
        // it before injecting the press into the target application.
        self.own.push_back((16, button as i32, XNextRequest(d)));
        XTestFakeButtonEvent(d, button, 0, 0);
        self.own.push_back((15, button as i32, XNextRequest(d)));
        XTestFakeButtonEvent(d, button, 1, 0);
        if button >= 4 {
            self.own.push_back((16, button as i32, XNextRequest(d)));
            XTestFakeButtonEvent(d, button, 0, 0);
        }
        XSync(d, 0);
    }
    /// # Safety
    /// Restore only modifiers still physically held, never a released key.
    pub unsafe fn restore(&mut self, d: *mut Display) {
        for key in self.suppressed.drain(..) {
            // XTEST has no independent physical key state. Releasing its
            // modifier for replay also releases the originating virtual key;
            // a later duplicate virtual key-up may generate no raw event.
            // Never manufacture a fresh key-down for that device. Physical
            // keyboards retain their independent raw press/release state.
            if self.virtual_keys[key as usize] {
                self.keys[key as usize] = false;
            }
            if self.keys[key as usize] {
                self.restored[key as usize] = true;
                self.own.push_back((13, key as i32, XNextRequest(d)));
                XTestFakeKeyEvent(d, key as u32, 1, 0);
            }
        }
        XSync(d, 0);
    }
    /// # Safety
    /// Release injected device state before closing its display connection.
    pub unsafe fn finish(&mut self, d: *mut Display) {
        for (key, restored) in self.restored.iter_mut().enumerate() {
            if *restored {
                XTestFakeKeyEvent(d, key as u32, 0, 0);
                *restored = false;
            }
        }
        XSync(d, 0);
    }
    /// Call only after the main thread has drained its X event queue. All
    /// replay writes use XSync, so remaining tokens refer to no-op requests.
    pub fn clear_replayed(&mut self) {
        self.own.clear();
    }
    /// # Safety
    /// Display must be live on this thread.
    pub unsafe fn held(&self, d: *mut Display, mask: u32) -> bool {
        let names: &[u64] = match mask {
            8 => &[0xffe9, 0xffea],
            4 => &[0xffe3, 0xffe4],
            64 => &[0xffeb, 0xffec],
            _ => &[],
        };
        names.iter().any(|name| {
            let key = XKeysymToKeycode(d, *name) as usize;
            if !self.keys[key] {
                return false;
            }
            if !self.virtual_keys[key] {
                return true;
            }
            // A virtual device can omit duplicate releases; the master map
            // is authoritative for virtual modifiers, never a stale raw bit.
            let mut map = [0_i8; 32];
            XQueryKeymap(d, map.as_mut_ptr());
            map[key / 8] as u8 & (1 << (key % 8)) != 0
        })
    }
}
/// Docks and popup menus should keep normal desktop behaviour without a modifier.
/// # Safety
/// Requires a live display/root. The caller's overlay is excluded.
pub unsafe fn desktop_target(
    d: *mut Display,
    root: Window,
    overlay: Window,
    x: i32,
    y: i32,
) -> bool {
    extern "C" {
        fn XQueryTree(
            d: *mut Display,
            w: Window,
            root: *mut Window,
            parent: *mut Window,
            children: *mut *mut Window,
            n: *mut u32,
        ) -> i32;
        fn XGetGeometry(
            d: *mut Display,
            w: Window,
            r: *mut Window,
            x: *mut i32,
            y: *mut i32,
            width: *mut u32,
            height: *mut u32,
            border: *mut u32,
            depth: *mut u32,
        ) -> i32;
    }
    let (mut r, mut parent, mut children, mut n) = (0, 0, ptr::null_mut(), 0);
    if XQueryTree(d, root, &mut r, &mut parent, &mut children, &mut n) == 0 {
        return false;
    }
    let names = [
        c"_NET_WM_WINDOW_TYPE_DOCK",
        c"_NET_WM_WINDOW_TYPE_MENU",
        c"_NET_WM_WINDOW_TYPE_POPUP_MENU",
        c"_NET_WM_WINDOW_TYPE_DROPDOWN_MENU",
    ];
    let types = names.map(|name| XInternAtom(d, name.as_ptr(), 0));
    let property = XInternAtom(d, c"_NET_WM_WINDOW_TYPE".as_ptr(), 0);
    let mut result = false;
    for w in std::slice::from_raw_parts(children, n as usize)
        .iter()
        .rev()
    {
        if *w == overlay {
            continue;
        }
        let mut attributes = XWindowAttributes::default();
        if XGetWindowAttributes(d, *w, &mut attributes) == 0 || attributes.map_state != 2 {
            continue;
        }
        let (mut wx, mut wy, mut width, mut height, mut border, mut depth) = (0, 0, 0, 0, 0, 0);
        if XGetGeometry(
            d,
            *w,
            &mut r,
            &mut wx,
            &mut wy,
            &mut width,
            &mut height,
            &mut border,
            &mut depth,
        ) == 0
            || x < wx
            || y < wy
            || x >= wx + width as i32
            || y >= wy + height as i32
        {
            continue;
        }
        let (mut ty, mut format, mut count, mut after, mut data) = (0, 0, 0, 0, ptr::null_mut());
        XGetWindowProperty(
            d,
            *w,
            property,
            0,
            8,
            0,
            4,
            &mut ty,
            &mut format,
            &mut count,
            &mut after,
            &mut data,
        );
        if !data.is_null() {
            if format == 32 && count <= 8 {
                result = std::slice::from_raw_parts(data.cast::<u64>(), count as usize)
                    .iter()
                    .any(|t| types.contains(t));
            }
            XFree(data.cast());
        }
        if result {
            break;
        }
    }
    if !children.is_null() {
        XFree(children.cast());
    }
    result
}
