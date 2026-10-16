//! POSIX extended regular expressions, isolated from shell parsing/evaluation.
use nix::libc;
use std::{ffi::CString, mem::MaybeUninit};

pub fn regex_matches(pattern: &[u8], value: &[u8], ignore_case: bool) -> Result<bool, String> {
    let pattern = CString::new(pattern).map_err(|_| "NUL in regular expression".to_owned())?;
    let value = CString::new(value).map_err(|_| "NUL in regular expression operand".to_owned())?;
    let mut storage = MaybeUninit::<libc::regex_t>::uninit();
    let flags =
        libc::REG_EXTENDED | libc::REG_NOSUB | if ignore_case { libc::REG_ICASE } else { 0 };
    // SAFETY: storage is suitably aligned writable regex_t memory, and pattern
    // is NUL terminated. Only successful regcomp initializes the regex object.
    let status = unsafe { libc::regcomp(storage.as_mut_ptr(), pattern.as_ptr(), flags) };
    if status != 0 {
        return Err("invalid extended regular expression".to_owned());
    }
    // SAFETY: successful regcomp initialized storage. regexec receives a valid
    // C string; REG_NOSUB permits a null match array with zero entries.
    let status =
        unsafe { libc::regexec(storage.as_ptr(), value.as_ptr(), 0, std::ptr::null_mut(), 0) };
    // SAFETY: the successfully compiled object is freed exactly once, after use.
    unsafe {
        libc::regfree(storage.as_mut_ptr());
    }
    match status {
        0 => Ok(true),
        libc::REG_NOMATCH => Ok(false),
        _ => Err("regular expression evaluation failed".to_owned()),
    }
}
