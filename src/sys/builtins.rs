//! Raw builtin/process/signal primitives. Signal handlers only set lock-free flags.
use super::{Action, Errno, PreparedProcess, libc};
use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::{AtomicU64, Ordering};
static PENDING: AtomicU64 = AtomicU64::new(0);
static IGNORED: AtomicU64 = AtomicU64::new(0);
static WATCH_CHILDREN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
extern "C" fn caught(signal: i32) {
    if (1..64).contains(&signal) {
        PENDING.fetch_or(1u64 << (signal as u32), Ordering::Relaxed);
    }
}
pub fn first_pending_signal() -> Option<i32> {
    let mask = PENDING.load(Ordering::Relaxed);
    (mask != 0).then(|| mask.trailing_zeros() as i32)
}
pub fn pending_signals() -> u64 {
    PENDING.swap(0, Ordering::Relaxed)
}
pub fn ignored_signal(signal: i32) -> bool {
    (1..64).contains(&signal) && IGNORED.load(Ordering::Relaxed) & (1u64 << (signal as u32)) != 0
}
pub const SIGNALS: &[(i32, &str)] = &[
    (libc::SIGHUP, "HUP"),
    (libc::SIGINT, "INT"),
    (libc::SIGQUIT, "QUIT"),
    (libc::SIGILL, "ILL"),
    (libc::SIGTRAP, "TRAP"),
    (libc::SIGABRT, "ABRT"),
    (libc::SIGFPE, "FPE"),
    (libc::SIGKILL, "KILL"),
    (libc::SIGBUS, "BUS"),
    (libc::SIGSEGV, "SEGV"),
    (libc::SIGPIPE, "PIPE"),
    (libc::SIGALRM, "ALRM"),
    (libc::SIGTERM, "TERM"),
    (libc::SIGUSR1, "USR1"),
    (libc::SIGUSR2, "USR2"),
    (libc::SIGCHLD, "CHLD"),
    (libc::SIGCONT, "CONT"),
    (libc::SIGSTOP, "STOP"),
    (libc::SIGTSTP, "TSTP"),
    (libc::SIGTTIN, "TTIN"),
    (libc::SIGTTOU, "TTOU"),
    (libc::SIGURG, "URG"),
    (libc::SIGXCPU, "XCPU"),
    (libc::SIGXFSZ, "XFSZ"),
    (libc::SIGVTALRM, "VTALRM"),
    (libc::SIGPROF, "PROF"),
    (libc::SIGWINCH, "WINCH"),
    (libc::SIGSYS, "SYS"),
    (libc::SIGIO, "IO"),
];
pub fn signal_number(name: &[u8]) -> Option<i32> {
    let text = std::str::from_utf8(name).ok()?;
    if text == "EXIT" {
        return Some(0);
    }
    if let Ok(n) = text.parse::<i32>() {
        return (n == 0 || SIGNALS.iter().any(|(s, _)| *s == n)).then_some(n);
    }
    let text = text.strip_prefix("SIG").unwrap_or(text);
    SIGNALS.iter().find(|(_, n)| *n == text).map(|(n, _)| *n)
}
pub fn signal_name(n: i32) -> Option<&'static str> {
    if n == 0 {
        Some("EXIT")
    } else {
        SIGNALS.iter().find(|(s, _)| *s == n).map(|(_, n)| *n)
    }
}
pub fn trap_signal(n: i32, action: Option<&[u8]>) -> io::Result<()> {
    if n == 0 {
        return Ok(());
    }
    if (1..64).contains(&n) && INHERITED.load(Ordering::Relaxed) & (1u64 << (n as u32)) != 0 {
        return Ok(());
    }

    if n == libc::SIGKILL || n == libc::SIGSTOP {
        return Err(io::Error::other("signal cannot be trapped"));
    }
    if n != libc::SIGCHLD || !WATCH_CHILDREN.load(Ordering::Relaxed) {
        // SAFETY: sigaction is initialized and no pointers escape; caught performs only a
        // lock-free atomic operation on the supported 64-bit Linux/macOS targets.
        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            libc::sigemptyset(&mut sa.sa_mask);
            sa.sa_sigaction = match action {
                None if n == libc::SIGCHLD && WATCH_CHILDREN.load(Ordering::Relaxed) => {
                    caught as *const () as usize
                }
                None => libc::SIG_DFL,
                Some(b"") if n != libc::SIGCHLD => libc::SIG_IGN,
                Some(b"") => caught as *const () as usize,
                Some(_) => caught as *const () as usize,
            };
            if libc::sigaction(n, &sa, std::ptr::null_mut()) < 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }
    if (1..64).contains(&n) {
        let bit = 1u64 << (n as u32);
        if action == Some(b"") {
            IGNORED.fetch_or(bit, Ordering::Relaxed);
        } else {
            IGNORED.fetch_and(!bit, Ordering::Relaxed);
        }
    }
    Ok(())
}
pub fn send_signal(pid: i32, signal: i32) -> io::Result<()> {
    // SAFETY: kill takes integer identifiers and retains no memory.
    if unsafe { libc::kill(pid, signal) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub fn parent_id() -> u32 {
    // SAFETY: getppid has no arguments or mutable memory.
    unsafe { libc::getppid() as u32 }
}
pub fn read_byte(fd: i32) -> io::Result<Option<u8>> {
    let mut b = 0;
    // SAFETY: b is writable for one byte; no read-ahead is performed.
    let n = unsafe { libc::read(fd, (&mut b as *mut u8).cast(), 1) };
    if n == 1 {
        Ok(Some(b))
    } else if n == 0 {
        Ok(None)
    } else {
        Err(io::Error::last_os_error())
    }
}
pub fn terminal(fd: i32) -> bool {
    // SAFETY: isatty only inspects the supplied descriptor.
    unsafe { libc::isatty(fd) == 1 }
}
pub fn accessible(path: &std::path::Path, mode: u8) -> bool {
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: path is NUL-terminated and remains alive for access.
    unsafe { libc::access(path.as_ptr(), i32::from(mode)) == 0 }
}
// mode_t is u16 on macOS and u32 on Linux.
#[allow(clippy::useless_conversion)]
pub fn mask(value: Option<u32>) -> u32 {
    // SAFETY: umask takes a mode value only; query restores the mask immediately.
    // Shell builtin execution is single-threaded; this is not used on the fork child path.
    unsafe {
        let old = libc::umask(value.unwrap_or(0) as libc::mode_t);
        if value.is_none() {
            libc::umask(old);
        }
        u32::from(old)
    }
}
pub fn times_text() -> io::Result<Vec<u8>> {
    let mut out = String::new();
    for who in [libc::RUSAGE_SELF, libc::RUSAGE_CHILDREN] {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        // SAFETY: storage is writable and initialized by successful getrusage.
        if unsafe { libc::getrusage(who, usage.as_mut_ptr()) } < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the successful call initialized usage.
        let u = unsafe { usage.assume_init() };
        for (i, t) in [u.ru_utime, u.ru_stime].iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            let seconds = t.tv_sec;
            out.push_str(&format!(
                "{}m{}.{:03}s",
                seconds / 60,
                seconds % 60,
                t.tv_usec / 1000
            ));
        }
        out.push('\n');
    }
    Ok(out.into_bytes())
}
/// Replace the current process, never fork. All argv/env storage is already prepared.
pub fn replace_process(process: &PreparedProcess) -> u8 {
    let Action::Exec {
        paths,
        argv,
        env,
        missing,
        denied,
        failed,
        ..
    } = &process.action
    else {
        return 126;
    };
    // SAFETY: all pointers are prebuilt NUL-terminated storage valid across execve.
    unsafe {
        if !ignored_signal(libc::SIGPIPE) {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }
        if libc::chdir(process.cwd.as_ptr()) < 0 {
            return 126;
        }
        if let Err(failure) = process.redirects.apply() {
            process.redirects.report(failure);
            return process.redirection_status;
        }
        for fd in process.redirects.resource_fds() {
            if !process.redirects.inherited(fd) {
                libc::close(fd);
            }
        }
        let mut permission = false;
        for path in paths {
            libc::execve(path.as_ptr(), argv.as_ptr(), env.as_ptr());
            match Errno::last_raw() {
                libc::ENOENT | libc::ENOTDIR => {}
                libc::EACCES => permission = true,
                _ => {
                    super::write_child(2, failed);
                    return 126;
                }
            }
        }
        super::write_child(2, if permission { denied } else { missing });
        if permission { 126 } else { 127 }
    }
}

static INHERITED: AtomicU64 = AtomicU64::new(0);
pub fn capture_inherited_ignored() {
    let mut mask = 0;
    for &(n, _) in SIGNALS {
        if n == libc::SIGPIPE {
            continue;
        }
        // SAFETY: querying sigaction writes initialized storage; no disposition is changed.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(n, std::ptr::null(), &mut action) == 0
                && action.sa_sigaction == libc::SIG_IGN
            {
                mask |= 1u64 << (n as u32);
                if n == libc::SIGCHLD {
                    // A caught no-op preserves waitable statuses without permitting
                    // a user action for an inherited ignored CHLD disposition.
                    action.sa_sigaction = caught as *const () as usize;
                    action.sa_flags = 0;
                    libc::sigaction(n, &action, std::ptr::null_mut());
                }
            }
        }
    }
    INHERITED.store(mask, Ordering::Relaxed);
    IGNORED.fetch_or(mask, Ordering::Relaxed);
}
/// Consume the shell's cooperative SIGINT flag without discarding other traps.
pub fn take_interrupt() -> bool {
    PENDING.fetch_and(!(1u64 << (libc::SIGINT as u32)), Ordering::Relaxed)
        & (1u64 << (libc::SIGINT as u32))
        != 0
}

pub fn inherited_ignored(signal: i32) -> bool {
    (1..64).contains(&signal) && INHERITED.load(Ordering::Relaxed) & (1u64 << (signal as u32)) != 0
}
pub fn pending_signal_mask() -> u64 {
    PENDING.load(Ordering::Relaxed)
}

pub(super) fn enable_child_watch() -> io::Result<()> {
    if !WATCH_CHILDREN.load(Ordering::Relaxed) {
        // SAFETY: the handler only updates a lock-free atomic flag. It captures no
        // heap-owned state, runs no interpreter, allocates nothing and never locks.
        // Registration persists for this process; exec resets it in every worker.
        unsafe {
            signal_hook::low_level::register(libc::SIGCHLD, || {
                PENDING.fetch_or(1u64 << (libc::SIGCHLD as u32), Ordering::Relaxed);
            })?;
            // signal-hook installs SA_RESTART. Idle terminal reads must wake so
            // notifications are emitted in shell context, not delayed until input.
            let mut action: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut action) < 0 {
                return Err(io::Error::last_os_error());
            }
            action.sa_flags &= !libc::SA_RESTART;
            if libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()) < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        WATCH_CHILDREN.store(true, Ordering::Relaxed);
    }
    Ok(())
}
pub(super) fn allow_interactive_traps() {
    // Interactive shells may change the startup dispositions of job-control signals.
    let mut bits = 0u64;
    for n in [
        libc::SIGINT,
        libc::SIGQUIT,
        libc::SIGTSTP,
        libc::SIGTTIN,
        libc::SIGTTOU,
        libc::SIGCHLD,
    ] {
        bits |= 1u64 << (n as u32);
    }
    INHERITED.fetch_and(!bits, Ordering::Relaxed);
    IGNORED.fetch_and(!bits, Ordering::Relaxed);
}
