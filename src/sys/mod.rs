//! Safe process wrappers. Every child path after fork is allocation/lock free.
use nix::{errno::Errno, libc};
use std::ffi::{CString, c_char};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    #[error("rsh: execution: NUL byte in argument, path or environment")]
    Nul(#[from] std::ffi::NulError),
}

mod fd;
pub use fd::{
    FdGuard, FdOp, FdPlan, OpenMode, capture_file, close_stdio, closed_stdio, here_input,
    read_worker_source, user_home, write_fd,
};

pub struct PreparedProcess {
    cwd: CString,
    action: Action,
    redirects: FdPlan,
    redirection_status: u8,
}
enum Action {
    Exec {
        paths: Vec<CString>,
        _argv: Vec<CString>,
        _env: Vec<CString>,
        argv: Vec<*const c_char>,
        env: Vec<*const c_char>,
        missing: Vec<u8>,
        denied: Vec<u8>,
        failed: Vec<u8>,
    },
    Finish {
        status: u8,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
}
#[derive(Clone, Copy, Default)]
pub struct SpawnOptions {
    pub detached: bool,
    pub null_stdin: bool,
    pub ignore_int_quit: bool,
}

impl PreparedProcess {
    pub fn external(
        words: &[Vec<u8>],
        environment: &[(Vec<u8>, Vec<u8>)],
        cwd: &Path,
        path: Option<&[u8]>,
    ) -> Result<Self, PrepareError> {
        let name = words.first().map(Vec::as_slice).unwrap_or_default();
        let candidates = if name.is_empty() {
            Vec::new()
        } else if name.contains(&b'/') {
            vec![name.to_vec()]
        } else {
            path.unwrap_or(b"/usr/bin:/bin")
                .split(|c| *c == b':')
                .map(|dir| {
                    let mut candidate = dir.to_vec();
                    if !dir.is_empty() {
                        candidate.push(b'/');
                    }
                    candidate.extend_from_slice(name);
                    candidate
                })
                .collect()
        };
        let paths = candidates
            .into_iter()
            .map(CString::new)
            .collect::<Result<Vec<_>, _>>()?;
        let strings = words
            .iter()
            .map(|s| CString::new(s.as_slice()))
            .collect::<Result<Vec<_>, _>>()?;
        let variables = environment
            .iter()
            .map(|(key, value)| {
                let mut bytes = key.clone();
                bytes.push(b'=');
                bytes.extend_from_slice(value);
                CString::new(bytes)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut argv: Vec<_> = strings.iter().map(|s| s.as_ptr()).collect();
        argv.push(std::ptr::null());
        let mut env: Vec<_> = variables.iter().map(|s| s.as_ptr()).collect();
        env.push(std::ptr::null());
        let diagnostic = |message: &[u8]| {
            let mut bytes = b"rsh: ".to_vec();
            bytes.extend_from_slice(name);
            bytes.extend_from_slice(b": ");
            bytes.extend_from_slice(message);
            bytes.push(b'\n');
            bytes
        };
        Ok(Self {
            cwd: CString::new(cwd.as_os_str().as_bytes())?,
            redirects: FdPlan::default(),
            redirection_status: 1,
            action: Action::Exec {
                paths,
                _argv: strings,
                _env: variables,
                argv,
                env,
                missing: diagnostic(b"not found"),
                denied: diagnostic(b"permission denied"),
                failed: diagnostic(b"cannot execute"),
            },
        })
    }
    pub fn with_redirects(mut self, plan: FdPlan, status: u8) -> Self {
        self.redirects.ops.extend(plan.ops);
        self.redirection_status = status;
        self
    }
    pub fn finished(
        status: u8,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        cwd: &Path,
    ) -> Result<Self, PrepareError> {
        Ok(Self {
            cwd: CString::new(cwd.as_os_str().as_bytes())?,
            redirects: FdPlan::default(),
            redirection_status: 1,
            action: Action::Finish {
                status,
                stdout,
                stderr,
            },
        })
    }
}

pub struct Child {
    pid: libc::pid_t,
}
impl Child {
    pub fn pid(&self) -> i32 {
        self.pid
    }
    pub fn wait(self) -> io::Result<u8> {
        wait(self.pid, false)?.ok_or_else(|| io::Error::other("waitpid returned no status"))
    }
    pub fn try_wait(&self) -> io::Result<Option<u8>> {
        wait(self.pid, true)
    }
}
fn wait(pid: libc::pid_t, nonblocking: bool) -> io::Result<Option<u8>> {
    loop {
        let mut status = 0;
        // SAFETY: status is writable; pid is a child handle; waitpid is retried on EINTR.
        let result = unsafe {
            libc::waitpid(
                pid,
                &mut status,
                if nonblocking { libc::WNOHANG } else { 0 },
            )
        };
        if result == 0 {
            return Ok(None);
        }
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if libc::WIFEXITED(status) {
            return Ok(Some(libc::WEXITSTATUS(status) as u8));
        }
        if libc::WIFSIGNALED(status) {
            return Ok(Some((128 + libc::WTERMSIG(status)) as u8));
        }
    }
}
fn promote(fd: OwnedFd) -> io::Result<OwnedFd> {
    // SAFETY: fd is owned and valid; fcntl duplicates it above the stdio descriptors.
    let new = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if new < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fcntl returned a new descriptor, transferred to this sole owner.
    Ok(unsafe { OwnedFd::from_raw_fd(new) })
}

pub fn spawn_pipeline(
    processes: &[PreparedProcess],
    options: SpawnOptions,
) -> io::Result<Vec<Child>> {
    for process in processes {
        process.redirects.protect_backups()?;
    }
    let mut pipes = Vec::new();
    for _ in 1..processes.len() {
        let (read, write) = nix::unistd::pipe()?;
        pipes.push((promote(read)?, promote(write)?));
    }
    let null = if options.null_stdin {
        Some(std::fs::File::open("/dev/null")?)
    } else {
        None
    };
    let fds: Vec<RawFd> = pipes
        .iter()
        .flat_map(|(r, w)| [r.as_raw_fd(), w.as_raw_fd()])
        .chain(null.iter().map(AsRawFd::as_raw_fd))
        .chain(
            processes
                .iter()
                .flat_map(|process| process.redirects.resource_fds()),
        )
        .collect();
    let mut children: Vec<Child> = Vec::new();
    for (i, process) in processes.iter().enumerate() {
        let input = i
            .checked_sub(1)
            .and_then(|index| pipes.get(index))
            .map(|(r, _)| r.as_raw_fd())
            .or_else(|| null.as_ref().map(AsRawFd::as_raw_fd));
        let output = pipes.get(i).map(|(_, w)| w.as_raw_fd());
        // SAFETY: the child immediately enters child_run, which never returns, allocates,
        // locks or unwinds; it uses only async-signal-safe calls and prebuilt buffers.
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            child_run(process, input, output, &fds, options);
        }
        if pid < 0 {
            let error = io::Error::last_os_error();
            drop(pipes);
            drop(null);
            for child in &children {
                // SAFETY: target only children created by this failed launch, never process groups.
                unsafe {
                    libc::kill(child.pid, libc::SIGKILL);
                }
            }
            for child in children {
                let _ = child.wait();
            }
            return Err(error);
        }
        // SAFETY: keep foreground children in the inherited group; detached workers get
        // their own group. Both parent and child call setpgid to close the race.
        unsafe {
            libc::setpgid(
                pid,
                if options.detached {
                    pid
                } else {
                    libc::getpgrp()
                },
            );
        }
        children.push(Child { pid });
    }
    // All parent pipe ends close before waiting, including failure paths through RAII.
    Ok(children)
}

fn child_run(
    process: &PreparedProcess,
    input: Option<RawFd>,
    output: Option<RawFd>,
    fds: &[RawFd],
    options: SpawnOptions,
) -> ! {
    // SAFETY: this block is the complete post-fork path. Pointers refer to immutable,
    // pre-fork-owned, NUL-terminated data; fd numbers are live until explicitly closed.
    // All libc calls below are async-signal-safe; no Rust allocation, formatting,
    // destructor, stdio lock, or panic-capable indexing occurs on this path.
    unsafe {
        libc::setpgid(0, if options.detached { 0 } else { libc::getpgrp() });
        let mut action: libc::sigaction = std::mem::zeroed();
        libc::sigemptyset(&mut action.sa_mask);
        for signal in [
            libc::SIGPIPE,
            libc::SIGINT,
            libc::SIGQUIT,
            libc::SIGTERM,
            libc::SIGHUP,
            libc::SIGTSTP,
            libc::SIGTTIN,
            libc::SIGTTOU,
            libc::SIGCHLD,
        ] {
            action.sa_sigaction =
                if options.ignore_int_quit && (signal == libc::SIGINT || signal == libc::SIGQUIT) {
                    libc::SIG_IGN
                } else {
                    libc::SIG_DFL
                };
            libc::sigaction(signal, &action, std::ptr::null_mut());
        }
        let mut mask = std::mem::zeroed();
        libc::sigemptyset(&mut mask);
        libc::sigprocmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut());
        if let Some(fd) = input
            && libc::dup2(fd, 0) < 0
        {
            libc::_exit(1);
        }
        if let Some(fd) = output
            && libc::dup2(fd, 1) < 0
        {
            libc::_exit(1);
        }
        for fd in fds {
            if *fd > 2 && !process.redirects.owns(*fd) {
                libc::close(*fd);
            }
        }
        if libc::chdir(process.cwd.as_ptr()) < 0 {
            write_child(2, b"rsh: execution: cannot change directory\n");
            libc::_exit(1);
        }
        if let Err(failure) = process.redirects.apply() {
            process.redirects.report(failure);
            libc::_exit(i32::from(process.redirection_status));
        }
        for fd in process.redirects.resource_fds() {
            if !process.redirects.inherited(fd) {
                libc::close(fd);
            }
        }
        match &process.action {
            Action::Finish {
                status,
                stdout,
                stderr,
            } => {
                write_child(1, stdout);
                write_child(2, stderr);
                libc::_exit(i32::from(*status));
            }
            Action::Exec {
                paths,
                argv,
                env,
                missing,
                denied,
                failed,
                ..
            } => {
                let mut permission = false;
                for path in paths {
                    libc::execve(path.as_ptr(), argv.as_ptr(), env.as_ptr());
                    match Errno::last_raw() {
                        libc::ENOENT | libc::ENOTDIR => {}
                        libc::EACCES => permission = true,
                        _ => {
                            write_child(2, failed);
                            libc::_exit(126);
                        }
                    }
                }
                if permission {
                    write_child(2, denied);
                    libc::_exit(126);
                }
                write_child(2, missing);
                libc::_exit(127);
            }
        }
    }
}
fn write_child(fd: RawFd, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        // SAFETY: bytes is readable for its length; write neither retains nor mutates it.
        let written = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
        if written > 0 {
            bytes = bytes.get(written as usize..).unwrap_or_default();
        } else if written < 0 && Errno::last_raw() == libc::EINTR {
            continue;
        } else {
            break;
        }
    }
}

pub fn directory_searchable(path: &Path) -> io::Result<()> {
    nix::unistd::access(path, nix::unistd::AccessFlags::X_OK).map_err(io::Error::from)
}

/// Non-destructive process-lifecycle probe. Unlike waitpid, WNOWAIT preserves
/// any child's pending status, so callers cannot steal another Child's result.
pub fn has_children() -> io::Result<bool> {
    loop {
        // SAFETY: siginfo_t is a plain C output record, initialized before waitid.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: info is writable; WNOWAIT observes without reaping; WNOHANG never blocks.
        let result = unsafe {
            libc::waitid(
                libc::P_ALL,
                0,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ECHILD) {
            return Ok(false);
        }
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// Peak resident memory, normalized to bytes on Linux and macOS (for stress checks).
pub fn peak_resident_bytes() -> io::Result<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: usage points to writable storage; getrusage initializes it on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful call initialized the complete structure.
    let usage = unsafe { usage.assume_init() };
    let bytes = u64::try_from(usage.ru_maxrss)
        .map_err(|_| io::Error::other("negative resident memory measurement"))?;
    #[cfg(target_os = "linux")]
    let bytes = bytes.saturating_mul(1024);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waits_reap_every_pipeline_child_and_close_unused_ends() {
        let cwd = std::env::current_dir().unwrap();
        let one = PreparedProcess::finished(0, b"one\n".to_vec(), vec![], &cwd).unwrap();
        let two = PreparedProcess::external(&[b"/usr/bin/true".to_vec()], &[], &cwd, None).unwrap();
        let children = spawn_pipeline(&[one, two], SpawnOptions::default()).unwrap();
        let pids: Vec<_> = children.iter().map(Child::pid).collect();
        for child in children {
            child.wait().unwrap();
        }
        for pid in pids {
            assert_eq!(
                wait(pid, true).unwrap_err().raw_os_error(),
                Some(libc::ECHILD)
            );
        }
    }
}
