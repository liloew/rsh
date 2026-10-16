use rsh::{env::ShellEnv, exec::Shell, parser};
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(status) => ExitCode::from(status),
        Err(error) => {
            eprintln!("rsh: input: {error}");
            ExitCode::from(1)
        }
    }
}
fn run() -> std::io::Result<u8> {
    rsh::sys::capture_inherited_ignored();
    let mut args = std::env::args_os().skip(1).peekable();
    if args
        .peek()
        .is_some_and(|s| s == "--m3-worker" || s == "--subshell-worker")
    {
        let kind = args.next();
        return worker(
            args.collect(),
            kind.as_deref() == Some(OsStr::new("--m3-worker")),
        );
    }
    let mut env = ShellEnv::inherited()?;
    rsh::sys::watch_interrupt()?;
    if let Some(a) = std::env::args_os().next() {
        env.arg0 = a.as_bytes().to_vec();
    }
    let mut dump = false;
    let mut mode = None;
    let mut end_options = false;
    while let Some(arg) = args.peek() {
        let b = arg.as_bytes();
        if b == b"--dump-ast" {
            dump = true;
            args.next();
            continue;
        }
        if b == b"--" {
            end_options = true;
            args.next();
            break;
        }
        if !b.starts_with(b"-") || b == b"-" {
            break;
        }
        let arg = args.next().unwrap_or_default();
        let bytes = arg.as_bytes();
        for &flag in bytes.get(1..).unwrap_or_default() {
            match flag {
                b'c' | b's' => mode = Some(flag),
                b'o' => {
                    let Some(n) = args.next() else {
                        return Ok(usage());
                    };
                    let Some(n) = n.to_str() else {
                        return Ok(usage());
                    };
                    if !env.set_option(n, true) {
                        return Ok(usage());
                    }
                }
                c => {
                    if !env.set_flag(c, true) {
                        return Ok(usage());
                    }
                }
            }
        }
        if mode == Some(b'c') {
            break;
        }
    }
    let stdin_mode = mode != Some(b'c') && (mode == Some(b's') || args.peek().is_none());
    if stdin_mode && rsh::sys::terminal(0) {
        env.options.interactive = true;
    }
    if env.options.interactive {
        env.options.monitor = true;
    }
    let first = match mode {
        Some(b'c') => Some(OsString::from("-c")),
        Some(b's') => Some(OsString::from("-s")),
        _ => args.next(),
    };
    let mut source = Vec::new();
    match first.as_deref() {
        Some(option) if option == "-c" => {
            let Some(command) = args.next() else {
                return Ok(usage());
            };
            if let Some(arg0) = args.next() {
                env.arg0 = arg0.as_bytes().to_vec();
            }
            env.positional = args.map(|a| a.as_bytes().to_vec()).collect();
            source.extend_from_slice(command.as_bytes());
        }
        option if option.is_none() || option == Some(OsStr::new("-s")) => {
            env.positional = args.map(|a| a.as_bytes().to_vec()).collect();
            if !dump && !env.options.noexec {
                let mut shell = Shell::new(env, std::env::current_exe()?);
                return Ok(shell.run_stdin());
            }
            std::io::stdin().read_to_end(&mut source)?;
        }
        Some(path) if end_options || !path.as_bytes().starts_with(b"-") => {
            env.arg0 = path.as_bytes().to_vec();
            env.positional = args.map(|a| a.as_bytes().to_vec()).collect();
            source = std::fs::read(path)?;
        }
        _ => return Ok(usage()),
    }
    evaluate(&source, dump, env, false, false)
}
fn usage() -> u8 {
    eprintln!("rsh: arguments: usage: rsh [--dump-ast] [-c command | -s | script]");
    2
}
fn evaluate(
    source: &[u8],
    dump: bool,
    env: ShellEnv,
    background: bool,
    worker: bool,
) -> std::io::Result<u8> {
    if !dump && !worker {
        let mut shell = Shell::new(env, std::env::current_exe()?);
        shell.background_signals = background;
        return Ok(shell.run_source(source));
    }
    let script = match parser::parse(source) {
        Ok(script) => script,
        Err(error) => {
            eprintln!("{error}");
            return Ok(2);
        }
    };
    if dump {
        writeln!(std::io::stdout().lock(), "{script:#?}")?;
        return Ok(0);
    }
    let mut shell = Shell::new(env, std::env::current_exe()?);
    shell.background_signals = background;
    Ok(shell.run(&script))
}
fn worker(args: Vec<OsString>, background: bool) -> std::io::Result<u8> {
    let (Some(source), Some(status)) = (args.first(), args.get(1)) else {
        return Ok(usage());
    };
    let Ok(status) = status.to_string_lossy().parse::<u8>() else {
        return Ok(usage());
    };
    let Some(fd) = source
        .as_bytes()
        .strip_prefix(b"@")
        .and_then(|v| std::str::from_utf8(v).ok())
        .and_then(|v| v.parse::<i32>().ok())
    else {
        return Ok(usage());
    };
    let payload = rsh::sys::read_worker_source(fd)?;
    let mut snapshot = rsh::env::decode_snapshot(&payload)?;
    let Some(source) = snapshot.pop() else {
        return Ok(usage());
    };
    let mut direct_worker = false;
    let mut env = ShellEnv::inherited()?;
    rsh::sys::watch_interrupt()?;
    env.variables.clear();
    env.last_status = status;
    let mut closed = 0;
    let mut lines = Vec::new();
    for arg in snapshot.iter().map(|v| OsStr::from_bytes(v)) {
        let bytes = arg.as_bytes();
        if let Some(v) = bytes.strip_prefix(b"L=") {
            lines = v.to_vec();
            continue;
        }

        if let Some(v) = bytes.strip_prefix(b"O=") {
            if let Some(i) = v.iter().position(|b| *b == b'=')
                && let Ok(name) = std::str::from_utf8(v.get(..i).unwrap_or_default())
            {
                env.set_option(name, v.get(i + 1..) == Some(b"1"));
            }
            continue;
        }
        if bytes == b"T=1" {
            env.trap_listing_only = true;
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"E=") {
            env.errexit_ignored = v == b"1";
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"s=") {
            env.source_depth = std::str::from_utf8(v)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"r=") {
            if let Some(var) = env.variables.get_mut(v) {
                var.readonly = true;
            }
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"A=") {
            if let Some(i) = v.iter().position(|b| *b == b'=') {
                env.aliases.insert(
                    v.get(..i).unwrap_or_default().to_vec(),
                    v.get(i + 1..).unwrap_or_default().to_vec(),
                );
            }
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"t=") {
            if let Some(i) = v.iter().position(|b| *b == b'=')
                && let Some(n) = std::str::from_utf8(v.get(..i).unwrap_or_default())
                    .ok()
                    .and_then(|s| s.parse().ok())
            {
                let action = v.get(i + 1..).unwrap_or_default().to_vec();
                if action.is_empty() {
                    rsh::sys::trap_signal(n, Some(&action))?;
                }
                env.traps.insert(n, action);
            }
            continue;
        }

        if let Some(v) = bytes.strip_prefix(b"h=") {
            env.function_depth = std::str::from_utf8(v)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            continue;
        }
        if bytes == b"J=1" {
            direct_worker = true;
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"f=") {
            let fields = rsh::env::decode_snapshot(v)?;
            let (Some(locations), Some(source)) = (fields.first(), fields.get(1)) else {
                return Ok(usage());
            };
            let mut tree =
                parser::parse(source).map_err(|e| std::io::Error::other(e.to_string()))?;
            parser::locations::restore(&mut tree, locations);
            if let Some(rsh::ast::Command::FunctionDef { name, definition }) =
                tree.0.first().and_then(|i| i.and_or.first.commands.first())
            {
                env.functions
                    .insert(name.clone(), std::rc::Rc::new(definition.clone()));
            } else {
                return Ok(usage());
            }
            continue;
        }

        if let Some(v) = bytes.strip_prefix(b"d=") {
            let p = std::path::PathBuf::from(OsStr::from_bytes(v));
            if !p.is_absolute() {
                return Ok(usage());
            }
            env.cwd = p;
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"a=") {
            env.positional.push(v.to_vec());
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"z=") {
            env.arg0 = v.to_vec();
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"g=") {
            env.noglob = v == b"1";
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"p=") {
            if let Some(n) = std::str::from_utf8(v).ok().and_then(|s| s.parse().ok()) {
                env.shell_pid = n;
            }
            continue;
        }
        if let Some(v) = bytes.strip_prefix(b"b=") {
            env.last_background = std::str::from_utf8(v).ok().and_then(|s| s.parse().ok());
            continue;
        }
        if let Some(value) = arg.as_bytes().strip_prefix(b"c=") {
            let Some(mask) = std::str::from_utf8(value)
                .ok()
                .and_then(|s| s.parse::<u8>().ok())
            else {
                return Ok(usage());
            };
            closed = mask;
            continue;
        }
        if arg == "n=1" || arg == "n=0" {
            env.noclobber = arg == "n=1";
            continue;
        }
        let Some((flag, bytes)) = arg.as_bytes().split_first() else {
            return Ok(usage());
        };
        let Some(equal) = bytes.iter().position(|byte| *byte == b'=') else {
            return Ok(usage());
        };
        let name = bytes.get(..equal).unwrap_or_default().to_vec();
        let value = bytes.get(equal + 1..).unwrap_or_default().to_vec();
        let var = match flag {
            b'u' => rsh::env::Variable {
                value: None,
                exported: true,
                readonly: false,
            },
            b'e' | b'v' => rsh::env::Variable {
                value: Some(value),
                exported: *flag == b'e',
                readonly: false,
            },
            _ => return Ok(usage()),
        };
        env.variables.insert(name, var);
    }
    rsh::sys::close_stdio(closed);
    let mut script = match parser::parse(&source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return Ok(2);
        }
    };
    parser::locations::restore(&mut script, &lines);
    let mut shell = Shell::new(env, std::env::current_exe()?);
    shell.background_signals = background;
    shell.direct_worker = direct_worker;
    Ok(shell.run(&script))
}
