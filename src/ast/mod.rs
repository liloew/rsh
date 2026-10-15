//! Plain command data (SPEC section 2).

use std::ffi::OsString;

/// Temporary M0 command representation, before the structured shell AST.
#[derive(Debug)]
pub struct SimpleCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
}

/// Unexpanded lexical word. Bytes and quote boundaries survive tokenization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word(pub Vec<WordPart>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordPart {
    Literal(Vec<u8>),
    /// The protected byte, without its introducing backslash.
    Escaped(u8),
    SingleQuoted(Vec<u8>),
    DoubleQuoted(Vec<WordPart>),
    Param(ParamExpr),
    CommandSub(CommandSub),
    Arith(Vec<u8>),
    Tilde(Option<Vec<u8>>),
    Glob(GlobChar),
}

/// Parameter syntax is retained, not evaluated or parsed as an expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamExpr {
    Name(Vec<u8>),
    Special(u8),
    Braced(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSub {
    pub style: CommandSubStyle,
    /// Source between delimiters; a future parser will consume these bytes.
    pub source: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSubStyle {
    DollarParen,
    Backtick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobChar {
    Star,
    Question,
    OpenBracket,
}
