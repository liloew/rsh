//! POSIX builtin dispatch; execution-context operations live in exec.
pub mod misc;
mod motd;
mod printf;
mod read;
mod shopt;
pub(crate) mod test;
pub mod variables;
use crate::env::ShellEnv;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum Builtin {
    Dot,
    Source,
    Shopt,
    Eval,
    Readonly,
    Unset,
    Times,
    Trap,
    Motd,
    Echo,
    Printf,
    Test,
    Bracket,
    Read,
    Type,
    Command,
    Alias,
    Unalias,
    Getopts,
    Umask,
    History,
    Jobs,
    Fg,
    Bg,
    Disown,
    Wait,
    Kill,
    Hash,
    Break,
    Continue,
    Return,
    Shift,
    Set,
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
        b"." => Some(Builtin::Dot),
        b"eval" => Some(Builtin::Eval),
        b"readonly" => Some(Builtin::Readonly),
        b"unset" => Some(Builtin::Unset),
        b"times" => Some(Builtin::Times),
        b"trap" => Some(Builtin::Trap),
        b"break" => Some(Builtin::Break),
        b"continue" => Some(Builtin::Continue),
        b"return" => Some(Builtin::Return),
        b"shift" => Some(Builtin::Shift),
        b"set" => Some(Builtin::Set),
        b"exec" => Some(Builtin::Exec),
        b":" => Some(Builtin::Colon),
        b"exit" => Some(Builtin::Exit),
        b"export" => Some(Builtin::Export),
        _ => None,
    }
}
/// Every POSIX special name is protected, including M7 utilities not implemented yet.
pub fn is_special_name(name: &[u8]) -> bool {
    matches!(
        name,
        b":" | b"."
            | b"break"
            | b"continue"
            | b"eval"
            | b"exec"
            | b"exit"
            | b"export"
            | b"readonly"
            | b"return"
            | b"set"
            | b"shift"
            | b"times"
            | b"trap"
            | b"unset"
    )
}
pub fn regular(name: &[u8]) -> Option<Builtin> {
    match name {
        b"shopt" => Some(Builtin::Shopt),
        b"source" => Some(Builtin::Source),
        b"motd" => Some(Builtin::Motd),
        b"echo" => Some(Builtin::Echo),
        b"printf" => Some(Builtin::Printf),
        b"test" => Some(Builtin::Test),
        b"[" => Some(Builtin::Bracket),
        b"read" => Some(Builtin::Read),
        b"type" => Some(Builtin::Type),
        b"command" => Some(Builtin::Command),
        b"alias" => Some(Builtin::Alias),
        b"unalias" => Some(Builtin::Unalias),
        b"getopts" => Some(Builtin::Getopts),
        b"umask" => Some(Builtin::Umask),
        b"history" => Some(Builtin::History),
        b"jobs" => Some(Builtin::Jobs),
        b"fg" => Some(Builtin::Fg),
        b"bg" => Some(Builtin::Bg),
        b"disown" => Some(Builtin::Disown),
        b"wait" => Some(Builtin::Wait),
        b"kill" => Some(Builtin::Kill),
        b"hash" => Some(Builtin::Hash),
        b"true" => Some(Builtin::True),
        b"false" => Some(Builtin::False),
        b"cd" => Some(Builtin::Cd),
        b"pwd" => Some(Builtin::Pwd),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub flow: Option<crate::exec::Flow>,
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
            flow: None,
            status,
            stderr,
            stdout: Vec::new(),
        }
    }
}

pub fn run(builtin: Builtin, args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    match builtin {
        Builtin::Shopt => shopt::run(args, env),
        Builtin::Motd => motd::run(args, env),
        Builtin::History | Builtin::Jobs | Builtin::Fg | Builtin::Bg | Builtin::Disown => {
            Outcome::error(b"jobs", b"execution context required", 1)
        }
        Builtin::Break | Builtin::Continue | Builtin::Return => control(builtin, args, env, 0),
        Builtin::Shift => shift(args, env),
        Builtin::Set => variables::set(args, env),
        Builtin::Export => variables::attributes(args, env, false),
        Builtin::Readonly => variables::attributes(args, env, true),
        Builtin::Unset => variables::unset(args, env),
        Builtin::Echo => {
            let skip = usize::from(args.first().is_some_and(|a| a == b"-n"));
            let mut stdout = args.get(skip..).unwrap_or_default().join(b" ".as_slice());
            if skip == 0 {
                stdout.push(b'\n');
            }
            Outcome {
                stdout,
                ..Outcome::default()
            }
        }
        Builtin::Printf => printf::run(args),
        Builtin::Test | Builtin::Bracket => {
            test::run(args, matches!(builtin, Builtin::Bracket), env)
        }
        Builtin::Read => read::run(args, env),
        Builtin::Alias | Builtin::Unalias => {
            misc::alias(args, env, matches!(builtin, Builtin::Unalias))
        }
        Builtin::Getopts => misc::getopts(args, env),
        Builtin::Umask => misc::umask(args),
        Builtin::Kill => misc::kill(args),
        Builtin::Trap => misc::trap(args, env),
        Builtin::Times if !args.is_empty() => Outcome::error(b"times", b"unexpected operand", 2),
        Builtin::Times => match crate::sys::times_text() {
            Ok(stdout) => Outcome {
                stdout,
                ..Outcome::default()
            },
            Err(e) => Outcome::error(b"times", e.to_string().as_bytes(), 1),
        },
        Builtin::Hash => Outcome::default(),
        Builtin::Dot
        | Builtin::Source
        | Builtin::Eval
        | Builtin::Exec
        | Builtin::Type
        | Builtin::Command
        | Builtin::Wait => Outcome::error(b"builtin", b"execution context required", 2),
        Builtin::Colon | Builtin::True => Outcome::default(),
        Builtin::False => Outcome {
            status: 1,
            ..Outcome::default()
        },
        Builtin::Exit => exit(args, env),
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
    let mut selected = target.to_vec();
    let mut cdpath_print = false;
    if !print
        && !target.starts_with(b"/")
        && !matches!(target.split(|b| *b == b'/').next(), Some(b"." | b".."))
        && let Some(cdpath) = env.get(b"CDPATH")
    {
        for prefix in cdpath.split(|b| *b == b':') {
            let mut candidate = prefix.to_vec();
            if !candidate.is_empty() {
                candidate.push(b'/');
            }
            candidate.extend(target);
            let p = env.cwd.join(std::ffi::OsStr::from_bytes(&candidate));
            if p.is_dir() && crate::sys::directory_searchable(&p).is_ok() {
                selected = candidate;
                cdpath_print = !prefix.is_empty();
                break;
            }
        }
    }
    if env.is_readonly(b"PWD") || env.is_readonly(b"OLDPWD") {
        return Outcome::error(b"cd", b"readonly PWD or OLDPWD", 1);
    }
    let path = Path::new(std::ffi::OsStr::from_bytes(&selected));
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
    let stdout = if print || cdpath_print {
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

/// Context-sensitive special builtins return typed flow, never unwind the interpreter.
pub fn control(builtin: Builtin, args: &[Vec<u8>], env: &mut ShellEnv, loops: usize) -> Outcome {
    use crate::exec::Flow;
    let error = |env: &mut ShellEnv, name: &[u8]| {
        env.exit_requested = Some(2);
        Outcome::error(name, b"positive decimal count required", 2)
    };
    match builtin {
        Builtin::Break | Builtin::Continue => {
            let name = if matches!(builtin, Builtin::Break) {
                b"break".as_slice()
            } else {
                b"continue"
            };
            if args.len() > 1 {
                return error(env, name);
            }
            let n = match args.first() {
                Some(a) => count(a),
                None => Some(1),
            };
            let Some(n) = n.filter(|n| *n > 0) else {
                return error(env, name);
            };
            if loops == 0 {
                return Outcome::default();
            }
            Outcome {
                flow: Some(if matches!(builtin, Builtin::Break) {
                    Flow::Break(n.min(loops))
                } else {
                    Flow::Continue(n.min(loops))
                }),
                ..Outcome::default()
            }
        }
        Builtin::Return => {
            if env.function_depth == 0 && env.source_depth == 0 {
                return Outcome::error(b"return", b"not in a function or sourced script", 1);
            }
            if args.len() > 1 {
                env.exit_requested = Some(2);
                return Outcome::error(b"return", b"too many arguments", 2);
            }
            let status = if let Some(a) = args.first() {
                match exit_code(a) {
                    Some(n) => n,
                    None => {
                        env.exit_requested = Some(2);
                        return Outcome::error(b"return", b"numeric argument required", 2);
                    }
                }
            } else {
                env.last_status
            };
            Outcome {
                status,
                flow: Some(Flow::Return(status)),
                ..Outcome::default()
            }
        }
        _ => Outcome::default(),
    }
}
fn count(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(bytes.iter().fold(0usize, |n, b| {
        n.saturating_mul(10).saturating_add(usize::from(b - b'0'))
    }))
}
fn shift(args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    if args.len() > 1 {
        return Outcome::error(b"shift", b"too many arguments", 1);
    }
    let Some(n) = (match args.first() {
        Some(a) => count(a),
        None => Some(1),
    }) else {
        return Outcome::error(b"shift", b"nonnegative decimal count required", 1);
    };
    if n > env.positional.len() {
        return Outcome::error(b"shift", b"cannot shift that many", 1);
    }
    env.positional.drain(..n);
    Outcome::default()
}
