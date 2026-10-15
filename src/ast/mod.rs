//! Plain command data (SPEC section 2).

use std::ffi::OsString;

/// Temporary M0 command representation, before the structured shell AST.
#[derive(Debug)]
pub struct SimpleCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
}
