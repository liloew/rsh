use rsh::{exec, parser};

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    let dump_ast = first.as_deref() == Some(std::ffi::OsStr::new("--dump-ast"));
    let option = if dump_ast { args.next() } else { first };
    let source = match (option, args.next(), args.next()) {
        (Some(option), Some(source), None) if option == "-c" => source,
        _ => {
            eprintln!("rsh: arguments: usage: rsh [--dump-ast] -c command");
            return ExitCode::from(2);
        }
    };
    if dump_ast {
        use std::os::unix::ffi::OsStrExt;
        return match parser::parse(source.as_bytes()) {
            Ok(script) => {
                // Debug preserves byte values without requiring UTF-8.
                use std::io::Write;
                match writeln!(std::io::stdout().lock(), "{script:#?}") {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(error) => {
                        eprintln!("rsh: dump-ast: {error}");
                        ExitCode::from(1)
                    }
                }
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(2)
            }
        };
    }
    let Some(command) = parser::parse_m0(&source) else {
        return ExitCode::SUCCESS;
    };
    match exec::run(&command) {
        Ok(status) => ExitCode::from(status),
        Err(error) => {
            eprintln!("rsh: {}: {error}", command.program.to_string_lossy());
            ExitCode::from(1)
        }
    }
}
