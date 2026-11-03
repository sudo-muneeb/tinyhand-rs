use crate::{config::Config, hands::PreparedHand};
use std::path::PathBuf;

pub enum Event {
    Configure(Config),
    Pick(String),
    Pin(String),
    Unpin(String),
    Reorder(String, usize),
    Swap(String, String),
    Prepare(PathBuf, String),
    Prepared(Result<PreparedHand, String>),
    AddHand(PreparedHand),
    RemoveHand(String),
    Toggle,
    Start,
    Stop,
    Settings,
    Quit,
    Reset,
    Clear,
    Mute,
    Message(String),
    SuspendHotkeys(bool),
}

/// Integrate GLib's actual descriptors with the X11 event wait. Hidden GTK
/// windows and D-Bus do not require a busy polling timer.
/// # Safety
/// File descriptors must remain open throughout this main-thread GLib dispatch.
pub unsafe fn wait(xfd: i32, extra: &[i32], milliseconds: i32) {
    use gtk::glib::{self, translate::ToGlibPtr};
    let context = glib::MainContext::default();
    let Ok(_owner) = context.acquire() else {
        return;
    };
    let raw = context.to_glib_none().0;
    let mut priority = 0;
    let ready = glib::ffi::g_main_context_prepare(raw, &mut priority);
    let mut timeout = -1;
    let n = glib::ffi::g_main_context_query(raw, priority, &mut timeout, std::ptr::null_mut(), 0);
    let mut descriptors = (0..n)
        .map(|_| std::mem::zeroed::<glib::ffi::GPollFD>())
        .collect::<Vec<_>>();
    let count =
        glib::ffi::g_main_context_query(raw, priority, &mut timeout, descriptors.as_mut_ptr(), n)
            .min(n);
    let mut fds = descriptors
        .iter()
        .take(count as usize)
        .map(|p| crate::ffi::PollFd {
            fd: p.fd,
            events: p.events as i16,
            revents: 0,
        })
        .collect::<Vec<_>>();
    fds.push(crate::ffi::PollFd {
        fd: xfd,
        events: 1,
        revents: 0,
    });
    for fd in extra {
        fds.push(crate::ffi::PollFd {
            fd: *fd,
            events: 1,
            revents: 0,
        });
    }
    let timeout = if ready != 0 {
        0
    } else if timeout < 0 {
        milliseconds
    } else {
        timeout.min(milliseconds)
    };
    crate::ffi::poll(fds.as_mut_ptr(), fds.len() as u64, timeout);
    for (i, p) in descriptors.iter_mut().take(count as usize).enumerate() {
        p.revents = fds[i].revents as u16;
    }
    if glib::ffi::g_main_context_check(raw, priority, descriptors.as_mut_ptr(), count) != 0 {
        glib::ffi::g_main_context_dispatch(raw);
    }
}

pub fn launch(uri: &str) -> Result<(), String> {
    gtk::gio::AppInfo::launch_default_for_uri(uri, None::<&gtk::gio::AppLaunchContext>)
        .map_err(|e| e.to_string())
}
pub fn encoded(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
