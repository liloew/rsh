//! Terminal and wait primitives; callers never execute Rust between fork and exec.
use super::libc;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
#[derive(Clone)]
pub struct TerminalModes(libc::termios);
pub struct Terminal {
    file: super::HiddenFd,
    pgid: i32,
    modes: TerminalModes,
}
pub fn process_group() -> i32 {
    // SAFETY: getpgrp takes no arguments.
    unsafe { libc::getpgrp() }
}
pub fn terminal_modes(fd: RawFd) -> io::Result<TerminalModes> {
    let mut modes = std::mem::MaybeUninit::uninit();
    // SAFETY: tcgetattr initializes the writable storage on success.
    if unsafe { libc::tcgetattr(fd, modes.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful tcgetattr initialized modes.
    Ok(TerminalModes(unsafe { modes.assume_init() }))
}
pub fn restore_modes(fd: RawFd, modes: &TerminalModes) -> io::Result<()> {
    // SAFETY: modes is initialized and remains live throughout tcsetattr.
    if unsafe { libc::tcsetattr(fd, libc::TCSADRAIN, &modes.0) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub fn foreground_group(fd: RawFd, group: i32) -> io::Result<()> {
    // SAFETY: tcsetpgrp takes integer identifiers and retains no memory.
    if unsafe { libc::tcsetpgrp(fd, group) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
impl Terminal {
    pub fn initialize() -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")?;
        let file = super::HiddenFd::new(file.into())?;
        let fd = file.as_raw_fd();
        loop {
            // SAFETY: tcgetpgrp inspects the controlling terminal.
            let foreground = unsafe { libc::tcgetpgrp(fd) };
            if foreground < 0 {
                return Err(io::Error::last_os_error());
            }
            let group = process_group();
            if foreground == group {
                break;
            }
            // SAFETY: reset TTIN so an inherited ignore cannot cause a busy loop;
            // kill addresses only this shell's current process group.
            unsafe {
                libc::signal(libc::SIGTTIN, libc::SIG_DFL);
            }
            super::send_signal(-group, libc::SIGTTIN)?;
        }
        // SAFETY: these signal dispositions belong to the single-threaded shell;
        // children reset them before exec, independently of explicit trap ignores.
        unsafe {
            for n in [
                libc::SIGINT,
                libc::SIGQUIT,
                libc::SIGTSTP,
                libc::SIGTTIN,
                libc::SIGTTOU,
            ] {
                libc::signal(n, libc::SIG_IGN);
            }
            let pid = libc::getpid();
            if libc::setpgid(pid, pid) < 0 && libc::getpgrp() != pid {
                return Err(io::Error::last_os_error());
            }
        }
        super::allow_interactive_traps();
        let pgid = process_group();
        foreground_group(fd, pgid)?;
        let modes = terminal_modes(fd)?;
        Ok(Self { file, pgid, modes })
    }
    pub fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
    pub fn reclaim(&self) -> io::Result<()> {
        let result = foreground_group(self.fd(), self.pgid);
        let modes = restore_modes(self.fd(), &self.modes);
        result.and(modes)
    }
    pub fn give(&self, pgid: i32) -> io::Result<()> {
        foreground_group(self.fd(), pgid)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessEvent {
    Exited(u8),
    Signaled(i32),
    Stopped(i32),
    Continued,
}
impl ProcessEvent {
    pub fn status(self) -> u8 {
        match self {
            Self::Exited(s) => s,
            Self::Signaled(n) | Self::Stopped(n) => (128 + n) as u8,
            Self::Continued => 0,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct WaitEvent {
    pub pid: i32,
    pub event: ProcessEvent,
}
/// The caller names a specific owned PID or process group; never steals other waits.
pub fn wait_event(target: i32, nonblocking: bool) -> io::Result<Option<WaitEvent>> {
    let mut status = 0;
    // SAFETY: status is writable; waitpid retains no pointers. EINTR is exposed so
    // shell-context traps can run without waiting for another child transition.
    let pid = unsafe {
        libc::waitpid(
            target,
            &mut status,
            libc::WUNTRACED | libc::WCONTINUED | if nonblocking { libc::WNOHANG } else { 0 },
        )
    };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }
    if pid == 0 {
        return Ok(None);
    }
    let event = if libc::WIFEXITED(status) {
        ProcessEvent::Exited(libc::WEXITSTATUS(status) as u8)
    } else if libc::WIFSIGNALED(status) {
        ProcessEvent::Signaled(libc::WTERMSIG(status))
    } else if libc::WIFSTOPPED(status) {
        ProcessEvent::Stopped(libc::WSTOPSIG(status))
    } else {
        ProcessEvent::Continued
    };
    Ok(Some(WaitEvent { pid, event }))
}
pub fn watch_children() -> io::Result<()> {
    super::enable_child_watch()?;
    super::trap_signal(libc::SIGCHLD, Some(b"pending"))
}
pub fn watch_interrupt() -> io::Result<()> {
    // SAFETY: querying sigaction initializes writable local storage. A command
    // substitution collector in the interactive parent must preserve its ignore.
    let ignored = unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        libc::sigaction(libc::SIGINT, std::ptr::null(), &mut action) == 0
            && action.sa_sigaction == libc::SIG_IGN
    };
    if !ignored && !super::ignored_signal(libc::SIGINT) {
        super::trap_signal(libc::SIGINT, Some(b"pending"))?;
    }
    Ok(())
}
pub fn signal_message(n: i32) -> &'static str {
    match n {
        libc::SIGINT => "Interrupt",
        libc::SIGQUIT => "Quit",
        libc::SIGTERM => "Terminated",
        libc::SIGSEGV => "Segmentation fault",
        libc::SIGABRT => "Aborted",
        libc::SIGPIPE => "Broken pipe",
        libc::SIGHUP => "Hangup",
        libc::SIGKILL => "Killed",
        libc::SIGTSTP | libc::SIGSTOP | libc::SIGTTIN | libc::SIGTTOU => "Stopped",
        _ => "Signal",
    }
}
pub const INT: i32 = libc::SIGINT;
pub const QUIT: i32 = libc::SIGQUIT;
pub const PIPE: i32 = libc::SIGPIPE;
pub const CONT: i32 = libc::SIGCONT;
pub const HUP: i32 = libc::SIGHUP;
pub const CHLD: i32 = libc::SIGCHLD;
/// Undo shell-only interactive ignores for an exec builtin. Explicit trap ignores
/// remain inherited; caught traps become default, as required across exec.
pub fn reset_shell_signals() {
    // SAFETY: signal dispositions are changed only in the single-threaded shell.
    unsafe {
        for n in [
            libc::SIGINT,
            libc::SIGQUIT,
            libc::SIGTSTP,
            libc::SIGTTIN,
            libc::SIGTTOU,
            libc::SIGCHLD,
        ] {
            if !super::ignored_signal(n) {
                libc::signal(n, libc::SIG_DFL);
            }
        }
    }
}
pub fn current_foreground(fd: RawFd) -> io::Result<i32> {
    // SAFETY: tcgetpgrp only inspects this terminal descriptor.
    let group = unsafe { libc::tcgetpgrp(fd) };
    if group <= 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(group)
    }
}

impl TerminalModes {
    /// Observable terminal flags, useful when checking editor recovery.
    pub fn echo_and_canonical(&self) -> (bool, bool) {
        (
            self.0.c_lflag & libc::ECHO != 0,
            self.0.c_lflag & libc::ICANON != 0,
        )
    }
}
