//! M3 builtins and M4 redirection-only exec. Remaining builtin behavior is TODO(M7).
use crate::env::{ShellEnv, valid_name};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum Builtin {
    Exec,
    Colon,
    Exit,
    Export,
    True,
    False,
    Cd,
    Pwd,
}

pub fn special(name: &[u8]) -> Option<Builtin> {
    match name {
        b"exec" => Some(Builtin::Exec),
        b":" => Some(Builtin::Colon),
        b"exit" => Some(Builtin::Exit),
        b"export" => Some(Builtin::Export),
        _ => None,
    }
}
pub fn regular(name: &[u8]) -> Option<Builtin> {
    match name {
        b"true" => Some(Builtin::True),
        b"false" => Some(Builtin::False),
        b"cd" => Some(Builtin::Cd),
        b"pwd" => Some(Builtin::Pwd),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub status: u8,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
impl Outcome {
    pub fn error(context: &[u8], message: &[u8], status: u8) -> Self {
        let mut stderr = b"rsh: ".to_vec();
        stderr.extend_from_slice(context);
        stderr.extend_from_slice(b": ");
        stderr.extend_from_slice(message);
        stderr.push(b'\n');
        Self {
            status,
            stderr,
            stdout: Vec::new(),
        }
    }
}

pub fn run(builtin: Builtin, args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    match builtin {
        Builtin::Exec => {
            if args.is_empty() {
                Outcome::default()
            } else {
                Outcome::error(b"exec", b"command replacement is TODO(M7)", 2)
            }
        }
        Builtin::Colon | Builtin::True => Outcome::default(),
        Builtin::False => Outcome {
            status: 1,
            ..Outcome::default()
        },
        Builtin::Exit => exit(args, env),
        Builtin::Export => export(args, env),
        Builtin::Cd => cd(args, env),
        Builtin::Pwd => pwd(args, env),
    }
}

fn exit(args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    let args = if args.first().is_some_and(|arg| arg == b"--") {
        args.get(1..).unwrap_or_default()
    } else {
        args
    };
    let code = if let Some(arg) = args.first() {
        match exit_code(arg) {
            Some(code) => code,
            None => {
                env.exit_requested = Some(2);
                return Outcome::error(b"exit", b"numeric argument required", 2);
            }
        }
    } else {
        env.last_status
    };
    if args.len() > 1 {
        env.exit_requested = Some(1);
        return Outcome::error(b"exit", b"too many arguments", 1);
    }
    env.exit_requested = Some(code);
    Outcome {
        status: code,
        ..Outcome::default()
    }
}
fn exit_code(arg: &[u8]) -> Option<u8> {
    let negative = arg.first() == Some(&b'-');
    let digits = if matches!(arg.first(), Some(b'-' | b'+')) {
        arg.get(1..)?
    } else {
        arg
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut code = 0u8;
    for digit in digits {
        code = code.wrapping_mul(10).wrapping_add(digit - b'0');
    }
    Some(if negative { code.wrapping_neg() } else { code })
}
fn export(args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    if args.is_empty() || args == [b"-p".to_vec()] {
        let mut stdout = Vec::new();
        for (name, var) in &env.variables {
            if !var.exported || !valid_name(name) {
                continue;
            }
            stdout.extend_from_slice(b"export ");
            stdout.extend_from_slice(name);
            if let Some(value) = &var.value {
                stdout.extend_from_slice(b"='");
                for byte in value {
                    if *byte == b'\'' {
                        stdout.extend_from_slice(b"'\\''");
                    } else {
                        stdout.push(*byte);
                    }
                }
                stdout.push(b'\'');
            }
            stdout.push(b'\n');
        }
        return Outcome {
            stdout,
            ..Outcome::default()
        };
    }
    for arg in args {
        let equal = arg.iter().position(|c| *c == b'=');
        let name = equal.and_then(|i| arg.get(..i)).unwrap_or(arg);
        if !valid_name(name) {
            env.exit_requested = Some(2);
            return Outcome::error(b"export", b"invalid variable name", 2);
        }
        if let Some(value) = equal.and_then(|i| arg.get(i + 1..)) {
            env.set(name.to_vec(), value.to_vec());
        }
        env.mark_exported(name.to_vec());
    }
    Outcome::default()
}
fn cd(args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    let mut physical = false;
    let mut operands = args;
    while let Some(arg) = operands.first() {
        if arg == b"--" {
            operands = operands.get(1..).unwrap_or_default();
            break;
        }
        if arg == b"-P" || arg == b"-L" {
            physical = arg == b"-P";
            operands = operands.get(1..).unwrap_or_default();
        } else {
            break;
        }
    }
    if operands.len() > 1 {
        return Outcome::error(b"cd", b"too many arguments", 1);
    }
    let print = operands.first().is_some_and(|a| a == b"-");
    let target = if print {
        env.get(b"OLDPWD")
    } else if let Some(arg) = operands.first() {
        Some(arg.as_slice())
    } else {
        env.get(b"HOME")
    };
    let Some(target) = target else {
        return Outcome::error(b"cd", b"HOME or OLDPWD is not set", 1);
    };
    if target.is_empty() {
        return Outcome::default();
    }
    let path = Path::new(std::ffi::OsStr::from_bytes(target));
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env.cwd.join(path)
    };
    let logical = if physical {
        joined.clone()
    } else {
        normalize(&joined)
    };
    let real = match logical.canonicalize() {
        Ok(real) if real.is_dir() => real,
        _ => return Outcome::error(b"cd", b"cannot change directory", 1),
    };
    if crate::sys::directory_searchable(&real).is_err() {
        return Outcome::error(b"cd", b"permission denied", 1);
    }
    let old = env.cwd.as_os_str().as_bytes().to_vec();
    env.cwd = if physical { real } else { logical };
    env.set(b"OLDPWD".to_vec(), old);
    env.mark_exported(b"OLDPWD".to_vec());
    let pwd = env.cwd.as_os_str().as_bytes().to_vec();
    env.set(b"PWD".to_vec(), pwd.clone());
    env.mark_exported(b"PWD".to_vec());
    let stdout = if print {
        let mut text = pwd;
        text.push(b'\n');
        text
    } else {
        Vec::new()
    };
    Outcome {
        stdout,
        ..Outcome::default()
    }
}
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
fn pwd(args: &[Vec<u8>], env: &ShellEnv) -> Outcome {
    let mut physical = false;
    for arg in args {
        match arg.as_slice() {
            b"-P" => physical = true,
            b"-L" => physical = false,
            b"--" => {}
            _ => return Outcome::error(b"pwd", b"unsupported argument", 2),
        }
    }
    let path = if physical {
        match env.cwd.canonicalize() {
            Ok(p) => p,
            Err(_) => return Outcome::error(b"pwd", b"cannot determine directory", 1),
        }
    } else {
        env.cwd.clone()
    };
    let mut stdout = path.as_os_str().as_bytes().to_vec();
    stdout.push(b'\n');
    Outcome {
        stdout,
        ..Outcome::default()
    }
}
