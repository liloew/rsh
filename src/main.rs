mod ast;
mod builtins;
mod env;
mod exec;
mod expand;
mod interactive;
mod jobs;
mod lexer;
mod parser;
mod redirect;
mod sys;

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let source = match (args.next(), args.next(), args.next()) {
        (Some(option), Some(source), None) if option == "-c" => source,
        _ => {
            eprintln!("rsh: arguments: usage: rsh -c command");
            return ExitCode::from(2);
        }
    };
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
