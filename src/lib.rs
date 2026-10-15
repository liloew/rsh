//! POSIX shell components. The M0 CLI is intentionally separate from the M1 lexer.
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
