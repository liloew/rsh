//! Nonblocking, bounded reads of untrusted message files.
use super::libc;
use std::{
    fs::OpenOptions,
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
pub const MOTD_LIMIT: usize = 64 * 1024;
pub fn motd_file(path: &Path) -> io::Result<Option<(Vec<u8>, bool)>> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOCTTY)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    // File::metadata uses fstat on this very descriptor, never a racy path check.
    // Follow final symlinks, but never read before validating the opened target.
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    let mut bytes = vec![0; MOTD_LIMIT];
    let mut length = 0;
    while let Some(buffer) = bytes.get_mut(length..).filter(|b| !b.is_empty()) {
        match file.read(buffer) {
            Ok(0) => break,
            Ok(count) => length += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    let mut extra = [0];
    let truncated = length == MOTD_LIMIT
        && loop {
            match file.read(&mut extra) {
                Ok(count) => break count != 0,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        };
    bytes.truncate(length);
    Ok(Some((bytes, truncated)))
}
pub fn motd_hushlogin(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

static INTERRUPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
extern "C" fn interrupt(_: i32) {
    INTERRUPTED.store(true, std::sync::atomic::Ordering::Relaxed);
}
pub struct MotdInterrupt(libc::sigaction);
impl MotdInterrupt {
    pub fn install() -> io::Result<Self> {
        INTERRUPTED.store(false, std::sync::atomic::Ordering::Relaxed);
        // SAFETY: sigaction receives initialized mask/handler storage; the handler
        // only updates a lock-free flag. The previous disposition is saved intact.
        unsafe {
            let mut previous = std::mem::zeroed();
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = interrupt as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            if libc::sigaction(libc::SIGINT, &action, &mut previous) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(previous))
        }
    }
}
impl Drop for MotdInterrupt {
    fn drop(&mut self) {
        // SAFETY: restore the disposition captured by install; no retained pointer.
        unsafe {
            libc::sigaction(libc::SIGINT, &self.0, std::ptr::null_mut());
        }
        INTERRUPTED.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}
pub fn motd_interrupted() -> bool {
    INTERRUPTED.load(std::sync::atomic::Ordering::Relaxed)
}
