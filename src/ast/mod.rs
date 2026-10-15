//! Plain command data (SPEC section 2).

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
    /// Source between delimiters, parsed when command substitution executes.
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

/// Lists normalize newlines/semicolons to Sequential and retain background jobs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Script(pub Vec<ListItem>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub and_or: AndOr,
    pub separator: Separator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Separator {
    Sequential,
    Background,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndOr {
    pub first: Pipeline,
    /// Equal precedence, in source order (left associative).
    pub rest: Vec<(AndOrOp, Pipeline)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndOrOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    pub negated: bool,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Simple(SimpleCommand),
    Compound(Box<CompoundCommand>, Vec<Redirection>),
    // TODO(M6): function definitions and their attached redirections.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleCommand {
    /// Preserve assignment/word/redirection order for later execution.
    pub parts: Vec<SimplePart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimplePart {
    Assignment(Assignment),
    Word(Word),
    Redirection(Redirection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub name: Vec<u8>,
    pub value: Word,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirection {
    /// Retain digits as bytes; descriptor range checking belongs to execution.
    pub fd: Option<Vec<u8>>,
    pub op: RedirOp,
    pub target: Word,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirOp {
    Input,
    Output,
    Append,
    DupInput,
    DupOutput,
    ReadWrite,
    Clobber,
    HereDoc(Box<HereDoc>),
}
/// The lexer collects bytes; execution expands `parts` for unquoted delimiters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HereDoc {
    pub delimiter: Vec<u8>,
    pub quoted: bool,
    pub strip_tabs: bool,
    pub raw: Vec<u8>,
    pub parts: Vec<WordPart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompoundCommand {
    Brace(Script),
    Subshell(Script),
    If {
        branches: Vec<IfBranch>,
        else_body: Option<Script>,
    },
    While {
        condition: Script,
        body: Script,
    },
    Until {
        condition: Script,
        body: Script,
    },
    For {
        name: Vec<u8>,
        words: Option<Vec<Word>>,
        body: Script,
    },
    Case {
        word: Word,
        items: Vec<CaseItem>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfBranch {
    pub condition: Script,
    pub body: Script,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseItem {
    pub patterns: Vec<Word>,
    pub body: Script,
}

mod display;
