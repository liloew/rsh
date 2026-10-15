use super::{Outcome, variables::quote};
use crate::env::{ShellEnv, valid_name};
use std::os::unix::ffi::OsStrExt;
pub fn find(
    name: &[u8],
    env: &ShellEnv,
    default: bool,
    executable: bool,
) -> Option<std::path::PathBuf> {
    let name = std::ffi::OsStr::from_bytes(name);
    let bytes = name.as_bytes();
    let candidates = if bytes.contains(&b'/') {
        vec![env.cwd.join(name)]
    } else {
        let path = if default {
            b"/usr/bin:/bin".as_slice()
        } else {
            env.get(b"PATH").unwrap_or(b"/usr/bin:/bin")
        };
        path.split(|b| *b == b':')
            .map(|p| env.cwd.join(std::ffi::OsStr::from_bytes(p)).join(name))
            .collect()
    };
    candidates
        .into_iter()
        .find(|p| p.is_file() && (!executable || crate::sys::accessible(p, 1)))
}
pub fn describe(name: &[u8], env: &ShellEnv, verbose: bool, default: bool) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    if let Some(alias) = env.aliases.get(name) {
        if verbose {
            out.extend(name);
            out.extend(b" is an alias for ");
        }
        out.extend(quote(alias));
    } else if [
        b"if".as_slice(),
        b"then",
        b"elif",
        b"else",
        b"fi",
        b"while",
        b"until",
        b"do",
        b"done",
        b"for",
        b"in",
        b"case",
        b"esac",
        b"{",
        b"}",
        b"!",
    ]
    .contains(&name)
    {
        out.extend(name);
        if verbose {
            out.extend(b" is a shell keyword");
        }
    } else if super::special(name).is_some() {
        out.extend(name);
        if verbose {
            out.extend(b" is a special shell builtin");
        }
    } else if env.functions.contains_key(name) {
        out.extend(name);
        if verbose {
            out.extend(b" is a shell function");
        }
    } else if super::regular(name).is_some() {
        out.extend(name);
        if verbose {
            out.extend(b" is a shell builtin");
        }
    } else {
        let path = find(name, env, default, true)?;
        if verbose {
            out.extend(name);
            out.extend(b" is ");
        }
        out.extend(path.as_os_str().as_bytes());
    }
    out.push(b'\n');
    Some(out)
}
pub fn alias(args: &[Vec<u8>], env: &mut ShellEnv, remove: bool) -> Outcome {
    if remove {
        if args.is_empty() {
            return Outcome::error(b"unalias", b"alias name or -a required", 2);
        }
        if args.iter().any(|a| a.starts_with(b"-") && a != b"-a") {
            return Outcome::error(b"unalias", b"invalid option", 2);
        }
        if args == [b"-a".to_vec()] {
            env.aliases.clear();
            return Outcome::default();
        }
        let mut o = Outcome::default();
        for n in args {
            if env.aliases.remove(n).is_none() {
                o.status = 1;
                o.stderr.extend(b"rsh: unalias: unknown alias\n");
            }
        }
        return o;
    }
    let mut o = Outcome::default();
    if args.is_empty() {
        for (n, v) in &env.aliases {
            o.stdout.extend(n);
            o.stdout.push(b'=');
            o.stdout.extend(quote(v));
            o.stdout.push(b'\n');
        }
        return o;
    }
    for a in args {
        if let Some(i) = a.iter().position(|b| *b == b'=') {
            let n = a.get(..i).unwrap_or_default();
            if n.is_empty() || n.iter().any(|b| b"/=$`\"' \t\n".contains(b)) {
                return Outcome::error(b"alias", b"invalid alias name", 1);
            }
            env.aliases
                .insert(n.to_vec(), a.get(i + 1..).unwrap_or_default().to_vec());
        } else if let Some(v) = env.aliases.get(a) {
            o.stdout.extend(a);
            o.stdout.push(b'=');
            o.stdout.extend(quote(v));
            o.stdout.push(b'\n');
        } else {
            o.status = 1;
            o.stderr.extend(b"rsh: alias: unknown alias\n");
        }
    }
    o
}
pub fn getopts(args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    let (Some(spec), Some(name)) = (args.first(), args.get(1)) else {
        return Outcome::error(b"getopts", b"optstring and name required", 2);
    };
    if !valid_name(name) {
        return Outcome::error(b"getopts", b"invalid variable name", 2);
    }
    let values = if args.len() > 2 {
        args.get(2..).unwrap_or_default().to_vec()
    } else {
        env.positional.clone()
    };
    let mut index = env
        .get(b"OPTIND")
        .and_then(|v| std::str::from_utf8(v).ok())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let mut offset = env.getopts_offset.max(1);
    let mut status = 0;
    let mut option = b'?';
    let mut optarg = None;
    let mut diagnostic = false;
    let silent = spec.first() == Some(&b':');
    let spec = if silent {
        spec.get(1..).unwrap_or_default()
    } else {
        spec.as_slice()
    };
    if let Some(value) = values.get(index - 1) {
        if value == b"--" {
            index += 1;
            status = 1;
        } else if !value.starts_with(b"-") || value == b"-" {
            status = 1;
        } else if let Some(&flag) = value.get(offset) {
            offset += 1;
            let found = spec.iter().position(|b| *b == flag && flag != b':');
            if let Some(p) = found {
                option = flag;
                if spec.get(p + 1) == Some(&b':') {
                    if offset < value.len() {
                        optarg = Some(value.get(offset..).unwrap_or_default().to_vec());
                        index += 1;
                    } else if let Some(v) = values.get(index) {
                        optarg = Some(v.clone());
                        index += 2;
                    } else {
                        index += 1;
                        option = if silent { b':' } else { b'?' };
                        if silent {
                            optarg = Some(vec![flag]);
                        } else {
                            diagnostic = true;
                        }
                    }
                    offset = 1;
                } else if offset >= value.len() {
                    index += 1;
                    offset = 1;
                }
            } else {
                if silent {
                    optarg = Some(vec![flag]);
                } else {
                    diagnostic = true;
                }
                if offset >= value.len() {
                    index += 1;
                    offset = 1;
                }
            }
        } else {
            status = 1;
        }
    } else {
        status = 1;
    }
    if env.assign(name.clone(), vec![option]).is_err()
        || env
            .assign(b"OPTIND".to_vec(), index.to_string().into_bytes())
            .is_err()
    {
        return Outcome::error(b"getopts", b"readonly variable", 1);
    }
    env.getopts_offset = offset;
    if let Some(v) = optarg {
        if let Err(e) = env.assign(b"OPTARG".to_vec(), v) {
            return Outcome::error(b"getopts", e.as_bytes(), 1);
        }
    } else {
        env.variables.remove(b"OPTARG".as_slice());
    }
    Outcome {
        status,
        stderr: if diagnostic && env.get(b"OPTERR") != Some(b"0") {
            b"rsh: getopts: invalid option or missing argument\n".to_vec()
        } else {
            Vec::new()
        },
        ..Outcome::default()
    }
}
pub fn umask(args: &[Vec<u8>]) -> Outcome {
    if args.is_empty() || args == [b"-S".to_vec()] {
        let mask = crate::sys::mask(None);
        let text = if args.is_empty() {
            format!("{mask:04o}\n")
        } else {
            let mut v = String::new();
            for (i, (name, shift)) in [("u", 6), ("g", 3), ("o", 0)].iter().enumerate() {
                if i > 0 {
                    v.push(',');
                }
                v.push_str(name);
                v.push('=');
                for (c, bit) in [('r', 4), ('w', 2), ('x', 1)] {
                    if mask & (bit << shift) == 0 {
                        v.push(c);
                    }
                }
            }
            v.push('\n');
            v
        };
        return Outcome {
            stdout: text.into_bytes(),
            ..Outcome::default()
        };
    }
    if args.len() != 1 {
        return Outcome::error(b"umask", b"one mask required", 1);
    }
    let value = std::str::from_utf8(args.first().map(Vec::as_slice).unwrap_or_default())
        .ok()
        .and_then(|s| u32::from_str_radix(s, 8).ok())
        .filter(|n| *n <= 0o777);
    if let Some(v) = value {
        crate::sys::mask(Some(v));
        Outcome::default()
    } else {
        Outcome::error(b"umask", b"invalid octal mask", 1)
    }
}
pub fn kill(args: &[Vec<u8>]) -> Outcome {
    if args.is_empty() {
        return Outcome::error(b"kill", b"pid required", 2);
    }
    let mut signal = crate::sys::signal_number(b"TERM").unwrap_or(15);
    let mut i = 0;
    if args.first().is_some_and(|a| a == b"-l") {
        let mut stdout = Vec::new();
        if let Some(a) = args.get(1) {
            if a.first().is_some_and(|b| !b.is_ascii_digit())
                && let Some(n) = crate::sys::signal_number(a)
            {
                return Outcome {
                    stdout: format!("{n}\n").into_bytes(),
                    ..Outcome::default()
                };
            }
            let n = std::str::from_utf8(a)
                .ok()
                .and_then(|s| s.parse::<i32>().ok())
                .map(|n| if n > 128 { n - 128 } else { n });
            if let Some(n) = n.and_then(crate::sys::signal_name) {
                stdout.extend(n.as_bytes());
                stdout.push(b'\n');
            } else {
                return Outcome::error(b"kill", b"unknown signal", 1);
            }
        } else {
            for (i, (_, n)) in crate::sys::SIGNALS.iter().enumerate() {
                if i > 0 {
                    stdout.push(b' ');
                }
                stdout.extend(n.as_bytes());
            }
            stdout.push(b'\n');
        }
        return Outcome {
            stdout,
            ..Outcome::default()
        };
    }
    if args.first().is_some_and(|a| a == b"-s") {
        let Some(n) = args.get(1).and_then(|s| crate::sys::signal_number(s)) else {
            return Outcome::error(b"kill", b"unknown signal", 2);
        };
        signal = n;
        i = 2;
    } else if let Some(a) = args.first()
        && a.starts_with(b"-")
        && a != b"--"
    {
        let Some(n) = crate::sys::signal_number(a.get(1..).unwrap_or_default()) else {
            return Outcome::error(b"kill", b"unknown signal", 2);
        };
        signal = n;
        i = 1;
    }
    if args.get(i).is_some_and(|a| a == b"--") {
        i += 1;
    }
    if i >= args.len() {
        return Outcome::error(b"kill", b"pid required", 2);
    }
    let mut outcome = Outcome::default();
    for a in args.get(i..).unwrap_or_default() {
        let pid = std::str::from_utf8(a)
            .ok()
            .and_then(|s| s.parse::<i32>().ok());
        if pid.is_none_or(|p| crate::sys::send_signal(p, signal).is_err()) {
            outcome.status = 1;
            outcome
                .stderr
                .extend(b"rsh: kill: invalid pid or signal failed\n");
        }
    }
    outcome
}
pub fn trap(args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    if args.is_empty() || args.first().is_some_and(|a| a == b"-p") {
        let mut stdout = Vec::new();
        let requested = args
            .get(1..)
            .unwrap_or_default()
            .iter()
            .map(|a| crate::sys::signal_number(a))
            .collect::<Option<Vec<_>>>();
        let Some(requested) = requested else {
            return Outcome::error(b"trap", b"unknown signal", 2);
        };
        for (n, action) in &env.traps {
            if !requested.is_empty() && !requested.contains(n) {
                continue;
            }
            stdout.extend(b"trap -- ");
            stdout.extend(quote(action));
            stdout.push(b' ');
            stdout.extend(crate::sys::signal_name(*n).unwrap_or("UNKNOWN").as_bytes());
            stdout.push(b'\n');
        }
        return Outcome {
            stdout,
            ..Outcome::default()
        };
    }
    let args = if args.first().is_some_and(|a| a == b"--") {
        args.get(1..).unwrap_or_default()
    } else {
        args
    };
    if args.len() < 2 {
        return Outcome::error(b"trap", b"action and signals required", 2);
    }
    let action = args.first().map(Vec::as_slice).unwrap_or_default();
    for sig in args.get(1..).unwrap_or_default() {
        let Some(n) = crate::sys::signal_number(sig) else {
            return Outcome::error(b"trap", b"unknown signal", 2);
        };
        let action = if action == b"-" { None } else { Some(action) };
        if let Err(e) = crate::sys::trap_signal(n, action) {
            return Outcome::error(b"trap", e.to_string().as_bytes(), 1);
        }
        if let Some(a) = action {
            env.traps.insert(n, a.to_vec());
        } else {
            env.traps.remove(&n);
        }
    }
    Outcome::default()
}
