//! Safe operating-system wrappers; future unsafe/raw syscalls belong here.

use std::ffi::{OsStr, OsString};
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

pub fn run_external(program: &OsStr, args: &[OsString]) -> std::io::Result<u8> {
    let status = Command::new(program).args(args).status()?;
    let code = status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0));
    Ok(code as u8)
}
