//! Bounded lifecycle diagnostics; pointer motion never writes to disk.
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
static FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();
pub fn path() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state")
        })
        .join("tinyhand-rs/tinyhand.log")
}
pub fn write(event: &str, message: &str) {
    let file = FILE.get_or_init(|| {
        Mutex::new((|| {
            let path = path();
            std::fs::create_dir_all(path.parent()?).ok()?;
            if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1024 * 1024) {
                let _ = std::fs::rename(&path, path.with_extension("log.1"));
            }
            OpenOptions::new().create(true).append(true).open(path).ok()
        })())
    });
    if let Ok(mut file) = file.lock() {
        if let Some(file) = file.as_mut() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let message = message.replace(['\n', '\r'], " ");
            let _ = writeln!(
                file,
                "{}.{:03} [{}] {event} {message}",
                now.as_secs(),
                now.subsec_millis(),
                std::process::id()
            );
        }
    }
}
