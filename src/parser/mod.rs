//! Hand-written recursive descent for SPEC 4.1; no execution or expansion.
use crate::ast::*;
use crate::lexer::{self, LexError, Operator as Op, Token, TokenKind};

pub const MAX_NESTING: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("rsh: parser: incomplete input: {0}")]
    Incomplete(String),
    #[error("rsh: line {line}: syntax error near unexpected token '{token}'")]
    Syntax {
        line: usize,
        col: usize,
        token: String,
    },
    #[error("rsh: parser: nesting limit exceeded")]
    NestingLimit,
}

pub fn parse(input: &[u8]) -> Result<Script, ParseError> {
    let tokens = lexer::lex(input).map_err(|error| match error {
        LexError::Incomplete(reason) => ParseError::Incomplete(format!(
            "{} at {}:{}",
            reason.construct, reason.opened_at.line, reason.opened_at.column
        )),
        LexError::NestingLimit(_) => ParseError::NestingLimit,
    })?;
    let mut parser = Parser {
        tokens,
        index: 0,
        input,
    };
    let script = parser.list(&[], None, true, 0)?;
    if parser.peek().is_some() {
        return Err(parser.error("end of input"));
    }
    Ok(script)
}

struct Parser<'a> {
    tokens: Vec<Token>,
    index: usize,
    input: &'a [u8],
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }
    fn advance(&mut self) {
        if self.peek().is_some() {
            self.index += 1;
        }
    }
    fn op(&self, op: Op) -> bool {
        self.peek()
            .is_some_and(|t| t.kind == TokenKind::Operator(op))
    }
    fn take_op(&mut self, op: Op) -> bool {
        if self.op(op) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn keyword(&self, word: &[u8]) -> bool {
        matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Word(Word(parts))) if matches!(parts.as_slice(), [WordPart::Literal(bytes)] if bytes == word))
    }
    fn take_keyword(&mut self, word: &[u8]) -> bool {
        if self.keyword(word) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn newline(&self) -> bool {
        self.peek().is_some_and(|t| t.kind == TokenKind::Newline)
    }
    fn linebreak(&mut self) {
        while self.newline() {
            self.advance();
        }
    }
    fn error(&self, expected: &str) -> ParseError {
        match self.peek() {
            None => ParseError::Incomplete(format!("expected {expected}")),
            Some(token) => {
                let spelling = if token.kind == TokenKind::Newline {
                    "newline".to_owned()
                } else {
                    String::from_utf8_lossy(
                        self.input
                            .get(token.span.start.offset..token.span.end.offset)
                            .unwrap_or_default(),
                    )
                    .into_owned()
                };
                ParseError::Syntax {
                    line: token.span.start.line,
                    col: token.span.start.column,
                    token: spelling,
                }
            }
        }
    }
    fn expect_keyword(&mut self, word: &[u8]) -> Result<(), ParseError> {
        if self.take_keyword(word) {
            Ok(())
        } else {
            Err(self.error(&String::from_utf8_lossy(word)))
        }
    }
    fn expect_op(&mut self, op: Op) -> Result<(), ParseError> {
        if self.take_op(op) {
            Ok(())
        } else {
            Err(self.error("operator"))
        }
    }
    fn word(&mut self) -> Result<Word, ParseError> {
        if let Some(Token {
            kind: TokenKind::Word(word),
            ..
        }) = self.peek()
        {
            let word = word.clone();
            self.advance();
            Ok(word)
        } else {
            Err(self.error("word"))
        }
    }
    fn stopped(&self, stops: &[&[u8]], close: Option<Op>) -> bool {
        stops.iter().any(|word| self.keyword(word)) || close.is_some_and(|op| self.op(op))
    }
    fn list(
        &mut self,
        stops: &[&[u8]],
        close: Option<Op>,
        empty: bool,
        depth: usize,
    ) -> Result<Script, ParseError> {
        self.linebreak();
        let mut items = Vec::new();
        if self.peek().is_none() || self.stopped(stops, close) {
            return if empty {
                Ok(Script(items))
            } else {
                Err(self.error("command"))
            };
        }
        loop {
            let and_or = self.and_or(depth)?;
            let (separator, separated) = if self.take_op(Op::Ampersand) {
                (Separator::Background, true)
            } else {
                (
                    Separator::Sequential,
                    self.take_op(Op::Semicolon) || self.newline(),
                )
            };
            items.push(ListItem { and_or, separator });
            if separated {
                self.linebreak();
            }
            if self.peek().is_none() || self.stopped(stops, close) {
                break;
            }
            if !separated {
                return Err(self.error("separator"));
            }
        }
        Ok(Script(items))
    }
    fn and_or(&mut self, depth: usize) -> Result<AndOr, ParseError> {
        let first = self.pipeline(depth)?;
        let mut rest = Vec::new();
        loop {
            let op = if self.take_op(Op::AndIf) {
                AndOrOp::And
            } else if self.take_op(Op::OrIf) {
                AndOrOp::Or
            } else {
                break;
            };
            self.linebreak();
            rest.push((op, self.pipeline(depth)?));
        }
        Ok(AndOr { first, rest })
    }
    fn pipeline(&mut self, depth: usize) -> Result<Pipeline, ParseError> {
        let negated = self.take_keyword(b"!");
        let mut commands = vec![self.command(depth)?];
        while self.take_op(Op::Pipe) {
            self.linebreak();
            commands.push(self.command(depth)?);
        }
        Ok(Pipeline { negated, commands })
    }
    fn command(&mut self, depth: usize) -> Result<Command, ParseError> {
        if depth >= MAX_NESTING {
            return Err(ParseError::NestingLimit);
        }
        let compound = if self.take_op(Op::LeftParen) {
            let body = self.list(&[], Some(Op::RightParen), false, depth + 1)?;
            self.expect_op(Op::RightParen)?;
            Some(CompoundCommand::Subshell(body))
        } else if self.take_keyword(b"{") {
            let body = self.list(&[b"}"], None, false, depth + 1)?;
            self.expect_keyword(b"}")?;
            Some(CompoundCommand::Brace(body))
        } else if self.take_keyword(b"if") {
            Some(self.if_clause(depth + 1)?)
        } else if self.take_keyword(b"while") {
            Some(self.loop_clause(false, depth + 1)?)
        } else if self.take_keyword(b"until") {
            Some(self.loop_clause(true, depth + 1)?)
        } else if self.take_keyword(b"for") {
            Some(self.for_clause(depth + 1)?)
        } else if self.take_keyword(b"case") {
            Some(self.case_clause(depth + 1)?)
        } else {
            None
        };
        if let Some(compound) = compound {
            let mut redirs = Vec::new();
            while self.redir_start() {
                redirs.push(self.redirection()?);
            }
            Ok(Command::Compound(Box::new(compound), redirs))
        } else {
            // TODO(M6): NAME '(' ')' compound_command function definitions.
            if [
                b"then".as_slice(),
                b"elif",
                b"else",
                b"fi",
                b"do",
                b"done",
                b"esac",
                b"in",
                b"}",
                b"!",
            ]
            .iter()
            .any(|w| self.keyword(w))
            {
                return Err(self.error("command"));
            }
            self.simple()
        }
    }
    fn simple(&mut self) -> Result<Command, ParseError> {
        let mut parts = Vec::new();
        let mut command_word = false;
        loop {
            if self.redir_start() {
                parts.push(SimplePart::Redirection(self.redirection()?));
            } else if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Word(_))) {
                let word = self.word()?;
                if !command_word && let Some(assignment) = assignment(&word) {
                    parts.push(SimplePart::Assignment(assignment));
                } else {
                    command_word = true;
                    parts.push(SimplePart::Word(word));
                }
            } else {
                break;
            }
        }
        if parts.is_empty() {
            return Err(self.error("command"));
        }
        Ok(Command::Simple(SimpleCommand { parts }))
    }
    fn redir_start(&self) -> bool {
        matches!(self.peek().map(|t| &t.kind), Some(TokenKind::IoNumber(_)))
            || [
                Op::Less,
                Op::Greater,
                Op::Append,
                Op::DupInput,
                Op::DupOutput,
                Op::ReadWrite,
                Op::Clobber,
                Op::HereDoc,
                Op::HereDocStrip,
            ]
            .iter()
            .any(|op| self.op(*op))
    }
    fn redirection(&mut self) -> Result<Redirection, ParseError> {
        let fd = if let Some(Token {
            kind: TokenKind::IoNumber(digits),
            ..
        }) = self.peek()
        {
            let digits = digits.clone();
            self.advance();
            Some(digits)
        } else {
            None
        };
        let op = match self.peek().map(|t| &t.kind) {
            Some(TokenKind::Operator(Op::Less)) => RedirOp::Input,
            Some(TokenKind::Operator(Op::Greater)) => RedirOp::Output,
            Some(TokenKind::Operator(Op::Append)) => RedirOp::Append,
            Some(TokenKind::Operator(Op::DupInput)) => RedirOp::DupInput,
            Some(TokenKind::Operator(Op::DupOutput)) => RedirOp::DupOutput,
            Some(TokenKind::Operator(Op::ReadWrite)) => RedirOp::ReadWrite,
            Some(TokenKind::Operator(Op::Clobber)) => RedirOp::Clobber,
            // TODO(M4): parse here-document delimiters and collect their bodies.
            _ => return Err(self.error("supported redirection")),
        };
        self.advance();
        Ok(Redirection {
            fd,
            op,
            target: self.word()?,
        })
    }
    fn if_clause(&mut self, depth: usize) -> Result<CompoundCommand, ParseError> {
        let mut branches = Vec::new();
        loop {
            let condition = self.list(&[b"then"], None, false, depth)?;
            self.expect_keyword(b"then")?;
            let body = self.list(&[b"elif", b"else", b"fi"], None, false, depth)?;
            branches.push(IfBranch { condition, body });
            if !self.take_keyword(b"elif") {
                break;
            }
        }
        let else_body = if self.take_keyword(b"else") {
            Some(self.list(&[b"fi"], None, false, depth)?)
        } else {
            None
        };
        self.expect_keyword(b"fi")?;
        Ok(CompoundCommand::If {
            branches,
            else_body,
        })
    }
    fn loop_clause(&mut self, until: bool, depth: usize) -> Result<CompoundCommand, ParseError> {
        let condition = self.list(&[b"do"], None, false, depth)?;
        self.expect_keyword(b"do")?;
        let body = self.list(&[b"done"], None, false, depth)?;
        self.expect_keyword(b"done")?;
        Ok(if until {
            CompoundCommand::Until { condition, body }
        } else {
            CompoundCommand::While { condition, body }
        })
    }
    fn for_clause(&mut self, depth: usize) -> Result<CompoundCommand, ParseError> {
        let name = match self.peek().map(|t| &t.kind) {
            Some(TokenKind::Word(Word(parts))) => match parts.as_slice() {
                [WordPart::Literal(name)] if valid_name(name) => name.clone(),
                _ => return Err(self.error("for variable name")),
            },
            _ => return Err(self.error("for variable name")),
        };
        self.advance();
        let had_newline = self.newline();
        self.linebreak();
        let words = if self.take_keyword(b"in") {
            let mut words = Vec::new();
            while matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Word(_))) {
                words.push(self.word()?);
            }
            if !self.take_op(Op::Semicolon) && !self.newline() {
                return Err(self.error("for separator"));
            }
            self.linebreak();
            Some(words)
        } else {
            // The no-`in` forms are NAME do_group or NAME sequential_sep do_group.
            // A newline already consumed a sequential_sep; a second ';' is invalid.
            if !had_newline && self.take_op(Op::Semicolon) {
                self.linebreak();
            }
            None
        };
        self.expect_keyword(b"do")?;
        let body = self.list(&[b"done"], None, false, depth)?;
        self.expect_keyword(b"done")?;
        Ok(CompoundCommand::For { name, words, body })
    }
    fn case_clause(&mut self, depth: usize) -> Result<CompoundCommand, ParseError> {
        let word = self.word()?;
        self.linebreak();
        self.expect_keyword(b"in")?;
        self.linebreak();
        let mut items = Vec::new();
        while !self.keyword(b"esac") {
            self.take_op(Op::LeftParen);
            let mut patterns = vec![self.word()?];
            while self.take_op(Op::Pipe) {
                patterns.push(self.word()?);
            }
            self.expect_op(Op::RightParen)?;
            let body = self.list(&[b"esac"], Some(Op::DoubleSemicolon), true, depth)?;
            items.push(CaseItem { patterns, body });
            if self.take_op(Op::DoubleSemicolon) {
                self.linebreak();
            } else {
                break;
            }
        }
        self.expect_keyword(b"esac")?;
        Ok(CompoundCommand::Case { word, items })
    }
}

fn valid_name(name: &[u8]) -> bool {
    name.first()
        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
        && name.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

fn assignment(word: &Word) -> Option<Assignment> {
    let (first, rest) = word.0.split_first()?;
    let WordPart::Literal(bytes) = first else {
        return None;
    };
    let index = bytes.iter().position(|byte| *byte == b'=')?;
    let name = bytes.get(..index)?;
    if !valid_name(name) {
        return None;
    }
    let suffix = bytes.get(index + 1..)?;
    let mut value = Vec::new();
    if !suffix.is_empty() {
        value.push(WordPart::Literal(suffix.to_vec()));
    }
    value.extend_from_slice(rest);
    Some(Assignment {
        name: name.to_vec(),
        value: Word(value),
    })
}
// M0 execution bootstrap; retained until M3 wires execution to the new AST.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};

/// M0 accepts only literal words separated by spaces or tabs.
/// The M2 parser is available separately; execution integration is deferred to M3.
pub fn parse_m0(source: &OsStr) -> Option<ExternalCommand> {
    let mut words = source
        .as_bytes()
        .split(|byte| matches!(byte, b' ' | b'\t'))
        .filter(|word| !word.is_empty())
        .map(|word| OsString::from_vec(word.to_vec()));
    Some(ExternalCommand {
        program: words.next()?,
        args: words.collect(),
    })
}

#[cfg(test)]
mod tests;
