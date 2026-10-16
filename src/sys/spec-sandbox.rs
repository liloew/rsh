//! Test-only resource-limit launcher. Limits are applied before exec, without a
//! post-fork Rust callback. The Python parent owns the process group/timeout.
use nix::libc;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};
fn run() -> std::io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let cpu = args
        .next()
        .and_then(|n| n.to_str()?.parse::<u64>().ok())
        .unwrap_or(5);
    let program = args
        .next()
        .ok_or_else(|| std::io::Error::other("missing program"))?;
    for (resource, limit) in [
        (libc::RLIMIT_CPU, cpu + 1),
        (libc::RLIMIT_FSIZE, 32 * 1024 * 1024),
        (libc::RLIMIT_NOFILE, 128),
        (libc::RLIMIT_NPROC, 1024),
    ] {
        let mut current = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        // SAFETY: current is writable storage initialized by successful getrlimit.
        if unsafe { libc::getrlimit(resource, current.as_mut_ptr()) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: getrlimit succeeded and initialized the structure.
        let current = unsafe { current.assume_init() };
        let value = libc::rlimit {
            rlim_cur: (limit as libc::rlim_t).min(current.rlim_max),
            rlim_max: current.rlim_max,
        };
        // SAFETY: initialized limits affect only this test launcher, before exec.
        if unsafe { libc::setrlimit(resource, &value) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    #[cfg(target_os = "linux")]
    {
        let value = libc::rlimit {
            rlim_cur: 512 * 1024 * 1024,
            rlim_max: 512 * 1024 * 1024,
        };
        // SAFETY: limits affect only the launcher; no pointer is retained.
        if unsafe { libc::setrlimit(libc::RLIMIT_AS, &value) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Err(Command::new(program).args(args).exec())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("spec-sandbox: {e}");
            ExitCode::from(126)
        }
    }
}
