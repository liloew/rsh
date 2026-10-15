//! Command execution (SPEC section 2).

use crate::ast::ExternalCommand;

pub fn run(command: &ExternalCommand) -> std::io::Result<u8> {
    crate::sys::run_external(&command.program, &command.args)
}
