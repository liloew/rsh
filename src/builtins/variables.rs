use super::Outcome;
use crate::env::{ShellEnv, valid_name};
pub fn quote(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![b'\''];
    for b in bytes {
        if *b == b'\'' {
            out.extend(b"'\\''");
        } else {
            out.push(*b);
        }
    }
    out.push(b'\'');
    out
}
pub fn listing(env: &ShellEnv, kind: Option<bool>) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, v) in &env.variables {
        if !valid_name(name) || kind.is_some_and(|ro| if ro { !v.readonly } else { !v.exported }) {
            continue;
        }
        if let Some(ro) = kind {
            out.extend(if ro {
                b"readonly ".as_slice()
            } else {
                b"export "
            });
        }
        out.extend(name);
        if let Some(value) = &v.value {
            out.push(b'=');
            out.extend(quote(value));
        }
        out.push(b'\n');
    }
    out
}
pub fn attributes(args: &[Vec<u8>], env: &mut ShellEnv, readonly: bool) -> Outcome {
    let name = if readonly {
        b"readonly".as_slice()
    } else {
        b"export"
    };
    let mut args = args;
    if args.first().is_some_and(|a| a == b"-p") {
        args = args.get(1..).unwrap_or_default();
    }
    if args.first().is_some_and(|a| a == b"--") {
        args = args.get(1..).unwrap_or_default();
    }
    if args.is_empty() {
        return Outcome {
            stdout: listing(env, Some(readonly)),
            ..Outcome::default()
        };
    }
    for arg in args {
        let equal = arg.iter().position(|b| *b == b'=');
        let key = equal.and_then(|i| arg.get(..i)).unwrap_or(arg);
        if !valid_name(key) {
            return Outcome::error(name, b"invalid variable name", 2);
        }
        if let Some(i) = equal
            && let Err(e) = env.assign(key.to_vec(), arg.get(i + 1..).unwrap_or_default().to_vec())
        {
            return Outcome::error(name, e.as_bytes(), 1);
        }
        if readonly {
            env.variables
                .entry(key.to_vec())
                .or_insert(crate::env::Variable {
                    value: None,
                    exported: false,
                    readonly: false,
                })
                .readonly = true;
        } else {
            env.mark_exported(key.to_vec());
        }
    }
    Outcome::default()
}
pub fn unset(args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    let mut functions = false;
    let mut i = 0;
    while let Some(a) = args.get(i) {
        match a.as_slice() {
            b"-f" => functions = true,
            b"-v" => functions = false,
            b"--" => {
                i += 1;
                break;
            }
            a if a.starts_with(b"-") => return Outcome::error(b"unset", b"invalid option", 2),
            _ => break,
        }
        i += 1;
    }
    for n in args.get(i..).unwrap_or_default() {
        if !valid_name(n) {
            return Outcome::error(b"unset", b"invalid name", 2);
        }
        if functions {
            env.functions.remove(n);
        } else {
            if env.is_readonly(n) {
                return Outcome::error(b"unset", b"readonly variable", 1);
            }
            env.variables.remove(n);
        }
    }
    Outcome::default()
}
pub fn set(args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    if args.is_empty() {
        return Outcome {
            stdout: listing(env, None),
            ..Outcome::default()
        };
    }
    let mut i = 0;
    while let Some(a) = args.get(i) {
        if a == b"--" {
            env.positional = args.get(i + 1..).unwrap_or_default().to_vec();
            return Outcome::default();
        }
        if a == b"-" {
            env.options.xtrace = false;
            i += 1;
            break;
        }
        let Some((&sign, letters)) = a.split_first() else {
            break;
        };
        if !matches!(sign, b'-' | b'+') || letters.is_empty() {
            break;
        }
        let on = sign == b'-';
        for &flag in letters {
            if flag == b'o' {
                if let Some(n) = args.get(i + 1) {
                    let Ok(n) = std::str::from_utf8(n) else {
                        return Outcome::error(b"set", b"invalid option", 2);
                    };
                    if !env.set_option(n, on) {
                        return Outcome::error(b"set", b"unknown option", 2);
                    }
                    i += 1;
                } else {
                    let mut stdout = Vec::new();
                    for name in crate::env::options::NAMES {
                        let value = env.option(name).unwrap_or(false);
                        if on {
                            stdout.extend(
                                format!("{name:<16}{}\n", if value { "on" } else { "off" })
                                    .as_bytes(),
                            );
                        } else {
                            stdout.extend(
                                format!("set {}o {name}\n", if value { "-" } else { "+" })
                                    .as_bytes(),
                            );
                        }
                    }
                    return Outcome {
                        stdout,
                        ..Outcome::default()
                    };
                }
            } else if !env.set_flag(flag, on) {
                return Outcome::error(b"set", b"invalid option", 2);
            }
        }
        i += 1;
    }
    if i < args.len() {
        env.positional = args.get(i..).unwrap_or_default().to_vec();
    }
    Outcome::default()
}
