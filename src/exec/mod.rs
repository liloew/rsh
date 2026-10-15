//! Command execution (SPEC section 2).

use crate::ast::SimpleCommand;

pub fn run(command: &SimpleCommand) -> std::io::Result<u8> {
    crate::sys::run_external(&command.program, &command.args)
}
