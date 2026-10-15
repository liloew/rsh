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
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    if first.as_deref() == Some(OsStr::new("--m3-worker")) {
        return worker(args.collect(), true);
    }
    if first.as_deref() == Some(OsStr::new("--subshell-worker")) {
        return worker(args.collect(), false);
    }
    let dump = first.as_deref() == Some(OsStr::new("--dump-ast"));
    let first = if dump { args.next() } else { first };
    let mut source = Vec::new();
    match first.as_deref() {
        Some(option) if option == "-c" => {
            let Some(command) = args.next() else {
                return Ok(usage());
            };
            if args.next().is_some() {
                return Ok(usage());
            }
            source.extend_from_slice(command.as_bytes());
        }
        option if option.is_none() || option == Some(OsStr::new("-s")) => {
            if args.next().is_some() {
                return Ok(usage());
            }
            std::io::stdin().read_to_end(&mut source)?;
        }
        Some(path) if !path.as_bytes().starts_with(b"-") => {
            if args.next().is_some() {
                return Ok(usage());
            }
            source = std::fs::read(path)?;
        }
        _ => return Ok(usage()),
    }
    evaluate(&source, dump, ShellEnv::inherited()?, false)
}
fn usage() -> u8 {
    eprintln!("rsh: arguments: usage: rsh [--dump-ast] [-c command | -s | script]");
    2
}
fn evaluate(source: &[u8], dump: bool, env: ShellEnv, background: bool) -> std::io::Result<u8> {
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
    let mut env = ShellEnv::inherited()?;
    env.variables.clear();
    env.last_status = status;
    let mut closed = 0;
    for arg in args.iter().skip(2) {
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
            },
            b'e' | b'v' => rsh::env::Variable {
                value: Some(value),
                exported: *flag == b'e',
            },
            _ => return Ok(usage()),
        };
        env.variables.insert(name, var);
    }
    let source = if let Some(bytes) = source.as_bytes().strip_prefix(b"@") {
        let Some(fd) = std::str::from_utf8(bytes)
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
        else {
            return Ok(usage());
        };
        rsh::sys::read_worker_source(fd)?
    } else {
        source.as_bytes().to_vec()
    };
    rsh::sys::close_stdio(closed);
    evaluate(&source, false, env, background)
}
