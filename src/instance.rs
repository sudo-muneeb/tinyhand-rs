//! One desktop instance; launcher actions reach it through a private local socket.
use std::{
    io,
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        io::AsRawFd,
        net::UnixDatagram,
    },
    path::PathBuf,
};
pub struct Instance {
    socket: UnixDatagram,
    path: PathBuf,
}
impl Instance {
    pub fn open(command: &str) -> io::Result<Option<Self>> {
        extern "C" {
            fn getuid() -> u32;
        }
        let uid = unsafe { getuid() };
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join(format!("tinyhand-{uid}")));
        if !dir.exists() {
            std::fs::create_dir(&dir)?;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let meta = std::fs::symlink_metadata(&dir)?;
        if meta.uid() != uid || !meta.is_dir() || meta.permissions().mode() & 0o022 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "runtime directory must be owned by you and not writable by others",
            ));
        }
        let display = std::env::var("DISPLAY")
            .unwrap_or_default()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "_");
        let path = dir.join(format!("tinyhand-{display}.socket"));
        if path.exists() {
            let client = UnixDatagram::unbound()?;
            if client.send_to(command.as_bytes(), &path).is_ok() {
                return Ok(None);
            }
            let meta = std::fs::symlink_metadata(&path)?;
            if meta.uid() != uid {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "socket belongs to another user",
                ));
            }
            std::fs::remove_file(&path)?;
        }
        let socket = UnixDatagram::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        socket.set_nonblocking(true)?;
        Ok(Some(Self { socket, path }))
    }
    pub fn fd(&self) -> i32 {
        self.socket.as_raw_fd()
    }
    pub fn receive(&self) -> Option<String> {
        let mut data = [0; 1024];
        self.socket
            .recv(&mut data)
            .ok()
            .and_then(|n| String::from_utf8(data[..n].to_vec()).ok())
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
