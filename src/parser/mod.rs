//! Source-to-command bootstrap for M0; the token-based parser belongs to M2.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};

use crate::ast::SimpleCommand;

/// M0 accepts only literal words separated by spaces or tabs.
/// Shell token recognition, quoting and operators are deferred to M1/M2.
pub fn parse_m0(source: &OsStr) -> Option<SimpleCommand> {
    let mut words = source
        .as_bytes()
        .split(|byte| matches!(byte, b' ' | b'\t'))
        .filter(|word| !word.is_empty())
        .map(|word| OsString::from_vec(word.to_vec()));
    Some(SimpleCommand {
        program: words.next()?,
        args: words.collect(),
    })
}
