//! A byte-oriented Unix shell interpreter and interactive layer.
//!
//! [`lexer`] and [`parser`] retain structured words; [`exec`] invokes [`expand`]
//! only at execution time. Raw operating-system calls and unsafe code live in
//! [`sys`]. The Rust API is experimental; SPEC.md defines shell-level behavior.
//!
//! Parsing does not execute commands or load user startup files:
//!
//! ```
//! # fn main() -> Result<(), rsh::parser::ParseError> {
//! let tree = rsh::parser::parse(b"printf '%s\\n' hello")?;
//! let source = tree.to_source_bytes();
//! assert_eq!(rsh::parser::parse(&source)?, tree);
//! # Ok(())
//! # }
//! ```
pub mod ast;
pub mod builtins;
pub mod env;
pub mod exec;
pub mod expand;
pub mod interactive;
pub mod jobs;
pub mod lexer;
pub mod parser;
pub mod redirect;
pub mod sys;
