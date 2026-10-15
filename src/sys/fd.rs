//! Descriptor primitives shared by parent guards and the allocation-free child path.
use super::{Errno, libc, write_child};
use std::cell::RefCell;
use std::ffi::CString;
use std::io::{self, Seek, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::rc::{Rc, Weak};

#[derive(Clone, Copy)]
pub enum OpenMode {
    Input,
    Output,
    Append,
    ReadWrite,
    NoClobber,
}
pub enum FdOp {
    Open {
        fd: RawFd,
        path: CString,
        mode: OpenMode,
        context: Vec<u8>,
    },
    Dup {
        fd: RawFd,
        source: RawFd,
        context: Vec<u8>,
    },
    Close {
        fd: RawFd,
    },
    Here {
        fd: RawFd,
        source: OwnedFd,
    },
    Inherit {
        source: OwnedFd,
    },
    Fail {
        context: Vec<u8>,
        reason: Vec<u8>,
    },
}
#[derive(Default)]
pub struct FdPlan {
    pub ops: Vec<FdOp>,
}
#[derive(Clone, Copy)]
pub struct FdFailure {
    index: usize,
    errno: i32,
}

thread_local! {
    static BACKUPS: RefCell<Vec<Weak<RefCell<OwnedFd>>>> = const {RefCell::new(Vec::new())};
}

impl FdPlan {
    /// Parent-only: move hidden backups away from descriptors explicitly accessed
    /// by this operation. This also protects dynamically expanded fd operands.
    pub fn protect_backups(&self) -> io::Result<()> {
        let references: Vec<_> = self
            .ops
            .iter()
            .flat_map(|op| match op {
                FdOp::Open { fd, .. } | FdOp::Close { fd } | FdOp::Here { fd, .. } => vec![*fd],
                FdOp::Dup { fd, source, .. } => vec![*fd, *source],
                _ => vec![],
            })
            .collect();
        BACKUPS.with(|registry| {
            let mut registry = registry.borrow_mut();
            registry.retain(|weak| weak.strong_count() > 0);
            for weak in registry.iter() {
                if let Some(slot) = weak.upgrade() {
                    let mut original = slot.borrow_mut();
                    if references.contains(&original.as_raw_fd()) {
                        let moved = duplicate_above(original.as_raw_fd(), self.floor())?;
                        *original = moved;
                    }
                }
            }
            Ok(())
        })
    }
    pub fn resource_fds(&self) -> impl Iterator<Item = RawFd> + '_ {
        self.ops.iter().filter_map(|op| {
            if let FdOp::Here { source, .. } | FdOp::Inherit { source } = op {
                Some(source.as_raw_fd())
            } else {
                None
            }
        })
    }
    pub fn inherited(&self, fd: RawFd) -> bool {
        self.ops
            .iter()
            .any(|op| matches!(op,FdOp::Inherit {source} if source.as_raw_fd()==fd))
    }
    pub fn closed_stdio(&self, mut mask: u8) -> u8 {
        for op in &self.ops {
            let (fd, closed) = match op {
                FdOp::Open { fd, .. } | FdOp::Dup { fd, .. } | FdOp::Here { fd, .. } => {
                    (*fd, false)
                }
                FdOp::Close { fd } => (*fd, true),
                _ => continue,
            };
            if (0..=2).contains(&fd) {
                let bit = 1u8 << fd;
                if closed {
                    mask |= bit;
                } else {
                    mask &= !bit;
                }
            }
        }
        mask
    }
    pub fn owns(&self, fd: RawFd) -> bool {
        self.resource_fds().any(|resource| resource == fd)
    }
    pub fn floor(&self) -> i32 {
        self.ops
            .iter()
            .flat_map(|op| match op {
                FdOp::Open { fd, .. } | FdOp::Close { fd } | FdOp::Here { fd, .. } => vec![*fd],
                FdOp::Dup { fd, source, .. } => vec![*fd, *source],
                FdOp::Fail { .. } | FdOp::Inherit { .. } => vec![],
            })
            .max()
            .unwrap_or(9)
            .saturating_add(1)
            .max(10)
    }
    /// Uses only async-signal-safe primitives; errors carry no allocated data.
    pub fn apply(&self) -> Result<(), FdFailure> {
        for (index, op) in self.ops.iter().enumerate() {
            // SAFETY: C strings and owned source descriptors remain live throughout
            // the call. No pointer is retained; all primitives are async-signal-safe.
            let result = unsafe {
                match op {
                    FdOp::Open { fd, path, mode, .. } => {
                        let flags = match mode {
                            OpenMode::Input => libc::O_RDONLY,
                            OpenMode::Output => libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
                            OpenMode::Append => libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND,
                            OpenMode::ReadWrite => libc::O_RDWR | libc::O_CREAT,
                            OpenMode::NoClobber => libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                        };
                        let opened = libc::open(
                            path.as_ptr(),
                            flags | libc::O_CLOEXEC,
                            0o666 as libc::c_uint,
                        );
                        if opened < 0 {
                            -1
                        } else {
                            let duplicated = libc::dup2(opened, *fd);
                            let saved_errno = Errno::last_raw();
                            if opened != *fd {
                                libc::close(opened);
                            }
                            if duplicated < 0 {
                                return Err(FdFailure {
                                    index,
                                    errno: saved_errno,
                                });
                            }
                            libc::fcntl(*fd, libc::F_SETFD, 0)
                        }
                    }
                    FdOp::Dup { fd, source, .. } => {
                        if libc::dup2(*source, *fd) < 0 {
                            -1
                        } else {
                            libc::fcntl(*fd, libc::F_SETFD, 0)
                        }
                    }
                    FdOp::Close { fd } => {
                        if libc::close(*fd) == 0 || Errno::last_raw() == libc::EBADF {
                            0
                        } else {
                            -1
                        }
                    }
                    FdOp::Here { fd, source } => libc::dup2(source.as_raw_fd(), *fd),
                    FdOp::Inherit { source } => libc::fcntl(source.as_raw_fd(), libc::F_SETFD, 0),
                    FdOp::Fail { .. } => return Err(FdFailure { index, errno: 0 }),
                }
            };
            if result < 0 {
                return Err(FdFailure {
                    index,
                    errno: Errno::last_raw(),
                });
            }
        }
        Ok(())
    }
    /// Diagnostic output must occur before restoring any redirected stderr.
    pub fn report(&self, failure: FdFailure) {
        let Some(op) = self.ops.get(failure.index) else {
            return;
        };
        let (context, reason) = match op {
            FdOp::Fail { context, reason } => (context.as_slice(), reason.as_slice()),
            FdOp::Open { context, .. } | FdOp::Dup { context, .. } => {
                (context.as_slice(), reason(failure.errno))
            }
            _ => (b"redirection".as_slice(), reason(failure.errno)),
        };
        write_child(2, b"rsh: ");
        write_child(2, context);
        write_child(2, b": ");
        write_child(2, reason);
        write_child(2, b"\n");
    }
}
fn reason(errno: i32) -> &'static [u8] {
    match errno {
        libc::ENOENT => b"No such file or directory",
        libc::EACCES | libc::EPERM => b"Permission denied",
        libc::EBADF => b"Bad file descriptor",
        libc::EISDIR => b"Is a directory",
        libc::ENOTDIR => b"Not a directory",
        libc::EEXIST => b"File exists",
        libc::EMFILE | libc::ENFILE => b"Too many open files",
        libc::ENOSPC => b"No space left on device",
        _ => b"Redirection failed",
    }
}

struct Saved {
    target: RawFd,
    original: Option<Rc<RefCell<OwnedFd>>>,
    flags: i32,
}
/// Restores all original descriptors even if application or command execution fails.
/// Descriptor state is process-global; callers must serialize parent redirections.
pub struct FdGuard {
    saved: Vec<Saved>,
}
impl FdGuard {
    pub fn save(plan: &FdPlan, reserved_floor: i32) -> io::Result<Self> {
        plan.protect_backups()?;
        let mut guard = Self { saved: Vec::new() };
        let floor = plan.floor().max(reserved_floor);
        for op in &plan.ops {
            let target = match op {
                FdOp::Open { fd, .. }
                | FdOp::Dup { fd, .. }
                | FdOp::Close { fd }
                | FdOp::Here { fd, .. } => *fd,
                FdOp::Fail { .. } | FdOp::Inherit { .. } => continue,
            };
            if guard.saved.iter().any(|saved| saved.target == target) {
                continue;
            }
            // SAFETY: fcntl observes or duplicates the descriptor; EBADF represents
            // an originally closed descriptor. The new duplicate is owned below.
            let flags = unsafe { libc::fcntl(target, libc::F_GETFD) };
            let original = if flags < 0 {
                if Errno::last_raw() != libc::EBADF {
                    return Err(io::Error::last_os_error());
                }
                None
            } else {
                let original = Rc::new(RefCell::new(duplicate_above(target, floor)?));
                BACKUPS.with(|registry| registry.borrow_mut().push(Rc::downgrade(&original)));
                Some(original)
            };
            guard.saved.push(Saved {
                target,
                original,
                flags,
            });
        }
        Ok(guard)
    }
    pub fn commit(mut self) {
        self.saved.clear();
    }
}
impl Drop for FdGuard {
    fn drop(&mut self) {
        for saved in self.saved.iter().rev() {
            // SAFETY: saved originals are owned, live high descriptors. dup2 restores
            // the target; flags restore its exact prior CLOEXEC state. No allocation.
            unsafe {
                if let Some(original) = &saved.original {
                    libc::dup2(original.borrow().as_raw_fd(), saved.target);
                    libc::fcntl(saved.target, libc::F_SETFD, saved.flags);
                } else {
                    libc::close(saved.target);
                }
            }
        }
    }
}
pub fn duplicate_above(fd: RawFd, floor: i32) -> io::Result<OwnedFd> {
    // SAFETY: fcntl duplicates fd; floor is a checked positive descriptor boundary.
    let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, floor) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: duplicate is freshly allocated and has a unique owner.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicate) })
}

/// 512 bytes fit in every POSIX pipe's guaranteed atomic-write capacity. Larger
/// documents use a seekable file unlinked BEFORE writing, never a blocking pipe fill.
pub fn here_input(bytes: &[u8], floor: i32) -> io::Result<OwnedFd> {
    if bytes.len() <= 512 {
        let (read, write) = nix::unistd::pipe()?;
        let read = duplicate_above(read.as_raw_fd(), floor)?;
        let mut writer = std::fs::File::from(write);
        writer.write_all(bytes)?;
        return Ok(read);
    }
    let template = std::env::temp_dir().join("rsh-heredoc-XXXXXX");
    let mut template = CString::new(template.as_os_str().as_bytes())?.into_bytes_with_nul();
    // SAFETY: template is mutable, NUL-terminated and ends in six X characters.
    let fd = unsafe { libc::mkstemp(template.as_mut_ptr().cast()) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: mkstemp returned a fresh descriptor, now uniquely owned.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: mkstemp left the newly created path NUL-terminated in template.
    if unsafe { libc::unlink(template.as_ptr().cast()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let owned = duplicate_above(owned.as_raw_fd(), floor)?;
    let mut file = std::fs::File::from(owned);
    file.write_all(bytes)?;
    file.rewind()?;
    Ok(file.into())
}
pub fn write_fd(fd: i32, bytes: &[u8]) -> io::Result<()> {
    let mut bytes = bytes;
    while !bytes.is_empty() {
        // SAFETY: bytes remains readable for its length; write retains no pointer.
        let written = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
        if written > 0 {
            bytes = bytes.get(written as usize..).unwrap_or_default();
        } else if written < 0 && Errno::last_raw() == libc::EINTR {
            continue;
        } else {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Only the internal re-exec worker consumes this dedicated inherited source fd.
pub fn read_worker_source(fd: i32) -> io::Result<Vec<u8>> {
    use std::io::Read;
    // SAFETY: caller transfers the dedicated worker-source descriptor, never stdio.
    // Validate the number; internal callers choose a high descriptor for this handoff.
    if fd < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid worker source descriptor",
        ));
    }
    // SAFETY: fcntl checks that the inherited descriptor is open before ownership transfer.
    if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the internal protocol transfers ownership of the validated fd to this worker.
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let mut source = Vec::new();
    file.read_to_end(&mut source)?;
    Ok(source)
}

/// Preserve explicitly closed stdio across Rust's process-startup sanitization.
pub fn closed_stdio() -> u8 {
    let mut mask = 0;
    for fd in 0..3 {
        // SAFETY: fcntl only observes this process's descriptor table.
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 && Errno::last_raw() == libc::EBADF {
            mask |= 1 << fd;
        }
    }
    mask
}
pub fn close_stdio(mask: u8) {
    for fd in 0..3 {
        if mask & (1 << fd) != 0 {
            // SAFETY: close only the stdio descriptors explicitly marked by the worker protocol.
            unsafe {
                libc::close(fd);
            }
        }
    }
}
