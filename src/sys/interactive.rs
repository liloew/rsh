//! Terminal recovery and history locking for the interactive layer.
use super::{TerminalModes, libc};
use std::{fs::File, io, os::fd::AsRawFd};
pub fn effective_uid() -> u32 {
    // SAFETY: geteuid has no arguments or memory effects.
    unsafe { libc::geteuid() }
}
pub fn terminal_size() -> Option<(u16, u16)> {
    let mut size = std::mem::MaybeUninit::<libc::winsize>::uninit();
    // SAFETY: ioctl initializes the winsize storage on success.
    if unsafe { libc::ioctl(0, libc::TIOCGWINSZ, size.as_mut_ptr()) } < 0 {
        return None;
    }
    // SAFETY: successful ioctl initialized size.
    let size = unsafe { size.assume_init() };
    (size.ws_col > 0 && size.ws_row > 0).then_some((size.ws_col, size.ws_row))
}
pub struct HistoryLock(File);
impl HistoryLock {
    pub fn acquire(file: File) -> io::Result<Self> {
        file.lock()?;
        Ok(Self(file))
    }
}
impl Drop for HistoryLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
pub struct Recovery {
    ansi: bool,
    modes: TerminalModes,
}
impl Recovery {
    fn restore(&self) {
        // Reopen on recovery instead of retaining a user-addressable descriptor:
        // `exec 3>file` must never replace our recovery terminal.
        if let Ok(file) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
        {
            let _ = super::restore_modes(file.as_raw_fd(), &self.modes);
            if self.ansi {
                let _ = super::write_fd(file.as_raw_fd(), b"\x1b[?2004l\x1b[?25h");
            }
        }
    }
}
impl Drop for Recovery {
    fn drop(&mut self) {
        self.restore();
    }
}
pub fn terminal_recovery(ansi: bool) -> io::Result<Recovery> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")?;
    let modes = super::terminal_modes(file.as_raw_fd())?;
    let recovery = Recovery {
        ansi,
        modes: modes.clone(),
    };
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let recovery = Recovery {
            ansi,
            modes: modes.clone(),
        };
        recovery.restore();
        previous(info);
    }));
    Ok(recovery)
}

pub fn resize_terminal(fd: i32, columns: u16, rows: u16) -> io::Result<()> {
    let size = libc::winsize {
        ws_col: columns,
        ws_row: rows,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: size is initialized and lives throughout ioctl; the fd identifies
    // a terminal owned by the caller. The kernel generates WINCH for its group.
    if unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &size) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

thread_local! { static EDITOR_FDS: std::cell::RefCell<std::collections::BTreeSet<i32>> = const { std::cell::RefCell::new(std::collections::BTreeSet::new()) }; }
pub(crate) fn editor_descriptor(fd: i32) -> bool {
    EDITOR_FDS.with(|fds| fds.borrow().contains(&fd))
}
/// Crossterm retains opaque descriptors which cannot be relocated through our
/// HiddenFd registry. Keep its initialization away from ordinary shell fds.
pub struct EditorDescriptors {
    held: Vec<File>,
    before: std::collections::BTreeSet<i32>,
}
fn open_descriptors() -> std::collections::BTreeSet<i32> {
    (3..1024)
        .filter(|fd| {
            // SAFETY: fcntl observes descriptor validity without changing ownership.
            unsafe { libc::fcntl(*fd, libc::F_GETFD) >= 0 }
        })
        .collect()
}
pub fn reserve_editor_descriptors() -> io::Result<EditorDescriptors> {
    let mut held = Vec::new();
    loop {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/null")?;
        let fd = file.as_raw_fd();
        held.push(file);
        if fd >= 64 {
            break;
        }
    }
    Ok(EditorDescriptors {
        before: open_descriptors(),
        held,
    })
}
impl EditorDescriptors {
    /// Call after the first read, with filesystem workers quiescent. Internal
    /// descriptors are CLOEXEC and cannot be referenced by user redirections.
    pub fn finish(self) {
        let private: std::collections::BTreeSet<_> = open_descriptors()
            .difference(&self.before)
            .copied()
            .collect();
        for fd in &private {
            // SAFETY: descriptor is live; this preserves flags and adds CLOEXEC.
            unsafe {
                let flags = libc::fcntl(*fd, libc::F_GETFD);
                if flags >= 0 {
                    libc::fcntl(*fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
                }
            }
        }
        EDITOR_FDS.with(|fds| *fds.borrow_mut() = private);
        drop(self.held);
    }
}
