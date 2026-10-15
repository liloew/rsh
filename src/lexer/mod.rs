//! Byte-oriented token recognition (SPEC section 3), without expansion or parsing.
//!
//! TODO(M4): collect here-document bodies after the delimiter's newline, including
//! quoted delimiters and <<- tab stripping. Only their operators are recognized now.

use crate::ast::{CommandSub, CommandSubStyle, GlobChar, ParamExpr, Word, WordPart};

/// Bound recursive quote/substitution scanning, including the resulting tree depth.
pub const MAX_NESTING: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub offset: usize,
    /// One-based physical line and byte column (tabs count as one byte).
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    /// Exclusive end; the original token spelling is input[start.offset..end.offset].
    pub end: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Word(Word),
    IoNumber(Vec<u8>),
    Newline,
    Operator(Operator),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    AndIf,
    OrIf,
    DoubleSemicolon,
    Semicolon,
    Ampersand,
    Pipe,
    LeftParen,
    RightParen,
    Less,
    Greater,
    HereDoc,
    Append,
    DupInput,
    DupOutput,
    ReadWrite,
    HereDocStrip,
    Clobber,
}

const OPERATORS: &[(&[u8], Operator)] = &[
    (b"<<-", Operator::HereDocStrip),
    (b"&&", Operator::AndIf),
    (b"||", Operator::OrIf),
    (b";;", Operator::DoubleSemicolon),
    (b"<<", Operator::HereDoc),
    (b">>", Operator::Append),
    (b"<&", Operator::DupInput),
    (b">&", Operator::DupOutput),
    (b"<>", Operator::ReadWrite),
    (b">|", Operator::Clobber),
    (b";", Operator::Semicolon),
    (b"&", Operator::Ampersand),
    (b"|", Operator::Pipe),
    (b"(", Operator::LeftParen),
    (b")", Operator::RightParen),
    (b"<", Operator::Less),
    (b">", Operator::Greater),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteReason {
    pub construct: &'static str,
    pub opened_at: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LexError {
    #[error("rsh: lexer: incomplete {0:?}")]
    Incomplete(IncompleteReason),
    #[error("rsh: lexer: nesting limit ({MAX_NESTING}) exceeded at {0:?}")]
    NestingLimit(Position),
}

/// Tokenize arbitrary bytes. Reserved words remain ordinary WORD tokens.
pub fn lex(input: &[u8]) -> Result<Vec<Token>, LexError> {
    let mut lexer = Lexer {
        input,
        position: Position {
            offset: 0,
            line: 1,
            column: 1,
        },
    };
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next_token(0)? {
        tokens.push(token);
    }
    Ok(tokens)
}

#[derive(Clone)]
struct Lexer<'a> {
    input: &'a [u8],
    position: Position,
}

impl Lexer<'_> {
    fn peek(&self) -> Option<u8> {
        self.input.get(self.position.offset).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.position.offset += 1;
        if byte == b'\n' {
            self.position.line += 1;
            self.position.column = 1;
        } else {
            self.position.column += 1;
        }
        Some(byte)
    }

    fn starts(&self, bytes: &[u8]) -> bool {
        self.input
            .get(self.position.offset..)
            .is_some_and(|tail| tail.starts_with(bytes))
    }

    fn continuations(&mut self) {
        while self.starts(b"\\\n") {
            self.bump();
            self.bump();
        }
    }

    /// Match across removed line continuations, without changing physical positions.
    fn logical_match(&self, bytes: &[u8]) -> Option<usize> {
        let mut offset = self.position.offset;
        for byte in bytes {
            while self
                .input
                .get(offset..)
                .is_some_and(|tail| tail.starts_with(b"\\\n"))
            {
                offset += 2;
            }
            if self.input.get(offset) != Some(byte) {
                return None;
            }
            offset += 1;
        }
        Some(offset)
    }

    fn advance_to(&mut self, end: usize) {
        while self.position.offset < end && self.bump().is_some() {}
    }

    fn source(&self, start: usize, end: usize) -> Vec<u8> {
        self.input.get(start..end).unwrap_or_default().to_vec()
    }

    fn incomplete(&self, construct: &'static str, opened_at: Position) -> LexError {
        LexError::Incomplete(IncompleteReason {
            construct,
            opened_at,
        })
    }

    fn check_depth(&self, depth: usize) -> Result<(), LexError> {
        if depth >= MAX_NESTING {
            Err(LexError::NestingLimit(self.position))
        } else {
            Ok(())
        }
    }

    fn next_token(&mut self, depth: usize) -> Result<Option<Token>, LexError> {
        loop {
            self.continuations();
            match self.peek() {
                Some(b' ' | b'\t') => {
                    self.bump();
                }
                Some(b'#') => {
                    // Comments end at the physical newline, even after a backslash.
                    while self.peek().is_some_and(|byte| byte != b'\n') {
                        self.bump();
                    }
                }
                _ => break,
            }
        }
        let start = self.position;
        let Some(byte) = self.peek() else {
            return Ok(None);
        };
        let kind = if byte == b'\n' {
            self.bump();
            TokenKind::Newline
        } else if let Some((end, operator)) = OPERATORS
            .iter()
            .find_map(|(text, op)| self.logical_match(text).map(|end| (end, *op)))
        {
            self.advance_to(end);
            TokenKind::Operator(operator)
        } else {
            let word = Word(self.parts(false, depth)?);
            match word.0.as_slice() {
                [WordPart::Literal(digits)]
                    if !digits.is_empty()
                        && digits.iter().all(u8::is_ascii_digit)
                        && matches!(self.peek(), Some(b'<' | b'>')) =>
                {
                    TokenKind::IoNumber(digits.clone())
                }
                _ => TokenKind::Word(word),
            }
        };
        Ok(Some(Token {
            kind,
            span: Span {
                start,
                end: self.position,
            },
        }))
    }

    fn literal(parts: &mut Vec<WordPart>, byte: u8) {
        if let Some(WordPart::Literal(bytes)) = parts.last_mut() {
            bytes.push(byte);
        } else {
            parts.push(WordPart::Literal(vec![byte]));
        }
    }

    fn parts(&mut self, double: bool, depth: usize) -> Result<Vec<WordPart>, LexError> {
        self.check_depth(depth)?;
        let mut parts = Vec::new();
        loop {
            self.continuations();
            let Some(byte) = self.peek() else {
                break;
            };
            if (double && byte == b'"') || (!double && is_boundary(byte)) {
                break;
            }
            let opened = self.position;
            match byte {
                b'\\' => {
                    self.bump();
                    let next = self
                        .peek()
                        .ok_or_else(|| self.incomplete("backslash", opened))?;
                    if !double || matches!(next, b'$' | b'`' | b'"' | b'\\') {
                        self.bump();
                        parts.push(WordPart::Escaped(next));
                    } else {
                        Self::literal(&mut parts, b'\\');
                    }
                }
                b'\'' if !double => parts.push(self.single_quote()?),
                b'"' => {
                    self.bump();
                    let inner = self.parts(true, depth + 1)?;
                    if self.bump() != Some(b'"') {
                        return Err(self.incomplete("double quote", opened));
                    }
                    parts.push(WordPart::DoubleQuoted(inner));
                }
                b'$' => {
                    if let Some(part) = self.dollar(depth + 1, double)? {
                        parts.push(part);
                    } else {
                        Self::literal(&mut parts, b'$');
                    }
                }
                b'`' => parts.push(self.backtick()?),
                b'~' if !double && parts.is_empty() => {
                    self.bump();
                    let mut user = Vec::new();
                    loop {
                        self.continuations();
                        match self.peek() {
                            Some(c)
                                if !is_boundary(c)
                                    && !matches!(
                                        c,
                                        b'/' | b':' | b'\'' | b'"' | b'\\' | b'$' | b'`'
                                    ) =>
                            {
                                user.push(c);
                                self.bump();
                            }
                            _ => break,
                        }
                    }
                    // A quoted portion prevents this word prefix being a tilde-prefix.
                    if matches!(self.peek(), Some(b'\'' | b'"' | b'\\' | b'$' | b'`')) {
                        Self::literal(&mut parts, b'~');
                        for c in user {
                            Self::literal(&mut parts, c);
                        }
                    } else {
                        parts.push(WordPart::Tilde(if user.is_empty() {
                            None
                        } else {
                            Some(user)
                        }));
                    }
                }
                b'*' | b'?' | b'[' if !double => {
                    self.bump();
                    parts.push(WordPart::Glob(match byte {
                        b'*' => GlobChar::Star,
                        b'?' => GlobChar::Question,
                        _ => GlobChar::OpenBracket,
                    }));
                }
                _ => {
                    self.bump();
                    Self::literal(&mut parts, byte);
                }
            }
        }
        Ok(parts)
    }

    fn single_quote(&mut self) -> Result<WordPart, LexError> {
        let opened = self.position;
        self.bump();
        let start = self.position.offset;
        while let Some(byte) = self.peek() {
            if byte == b'\'' {
                let value = self.source(start, self.position.offset);
                self.bump();
                return Ok(WordPart::SingleQuoted(value));
            }
            self.bump();
        }
        Err(self.incomplete("single quote", opened))
    }

    fn dollar(&mut self, depth: usize, double: bool) -> Result<Option<WordPart>, LexError> {
        self.check_depth(depth)?;
        let opened = self.position;
        self.bump();
        self.continuations();
        match self.peek() {
            Some(b'{') => {
                self.bump();
                let quoted = double && !self.parameter_pattern();
                let source = self.parameter(opened, depth, quoted)?;
                Ok(Some(WordPart::Param(ParamExpr::Braced(source))))
            }
            Some(b'(') => {
                self.bump();
                if let Some(end) = self.logical_match(b"(") {
                    self.advance_to(end);
                    Ok(Some(WordPart::Arith(self.arithmetic(opened, depth)?)))
                } else {
                    Ok(Some(WordPart::CommandSub(CommandSub {
                        style: CommandSubStyle::DollarParen,
                        source: self.command_sub(opened, depth)?,
                    })))
                }
            }
            Some(byte) if byte.is_ascii_alphabetic() || byte == b'_' => {
                let mut name = Vec::new();
                loop {
                    self.continuations();
                    match self.peek() {
                        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
                            name.push(c);
                            self.bump();
                        }
                        _ => break,
                    }
                }
                Ok(Some(WordPart::Param(ParamExpr::Name(name))))
            }
            Some(byte)
                if byte.is_ascii_digit()
                    || matches!(byte, b'@' | b'*' | b'#' | b'?' | b'-' | b'$' | b'!') =>
            {
                self.bump();
                Ok(Some(WordPart::Param(ParamExpr::Special(byte))))
            }
            _ => Ok(None),
        }
    }

    /// Only identify the parameter prefix/operator needed for quote recognition.
    /// No expression AST or expansion is produced.
    fn parameter_pattern(&self) -> bool {
        let mut probe = self.clone();
        probe.continuations();
        match probe.peek() {
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => loop {
                probe.continuations();
                match probe.peek() {
                    Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
                        probe.bump();
                    }
                    _ => break,
                }
            },
            Some(c) if c.is_ascii_digit() => loop {
                probe.continuations();
                if probe.peek().is_some_and(|c| c.is_ascii_digit()) {
                    probe.bump();
                } else {
                    break;
                }
            },
            Some(_) => {
                probe.bump();
            }
            None => {}
        }
        probe.continuations();
        matches!(probe.peek(), Some(b'%' | b'#'))
    }

    /// Find the matching brace while honoring the enclosing quote context.
    fn parameter(
        &mut self,
        opened: Position,
        depth: usize,
        quoted: bool,
    ) -> Result<Vec<u8>, LexError> {
        self.check_depth(depth)?;
        let start = self.position.offset;
        loop {
            match self.peek() {
                None => return Err(self.incomplete("parameter expansion", opened)),
                Some(b'}') => {
                    let body = self.source(start, self.position.offset);
                    self.bump();
                    return Ok(body);
                }
                Some(b'\\') => {
                    self.bump();
                    if !quoted
                        || matches!(self.peek(), Some(b'$' | b'`' | b'"' | b'\\' | b'\n' | b'}'))
                    {
                        self.bump();
                    }
                }
                Some(b'\'') if !quoted => {
                    self.single_quote()?;
                }
                Some(b'"') => {
                    let quote = self.position;
                    self.bump();
                    self.parts(true, depth + 1)?;
                    if self.bump() != Some(b'"') {
                        return Err(self.incomplete("double quote", quote));
                    }
                }
                Some(b'$') => {
                    self.dollar(depth + 1, quoted)?;
                }
                Some(b'`') => {
                    self.backtick()?;
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    fn command_sub(&mut self, opened: Position, depth: usize) -> Result<Vec<u8>, LexError> {
        self.check_depth(depth)?;
        let start = self.position.offset;
        let mut parens = 0usize;
        loop {
            let Some(token) = self.next_token(depth + 1)? else {
                return Err(self.incomplete("command substitution", opened));
            };
            match token.kind {
                TokenKind::Operator(Operator::LeftParen) => parens += 1,
                TokenKind::Operator(Operator::RightParen) if parens == 0 => {
                    // next_token may have skipped leading whitespace/comments.
                    return Ok(self.source(start, token.span.start.offset));
                }
                TokenKind::Operator(Operator::RightParen) => parens -= 1,
                _ => {}
            }
        }
    }

    fn arithmetic(&mut self, opened: Position, depth: usize) -> Result<Vec<u8>, LexError> {
        self.check_depth(depth)?;
        let start = self.position.offset;
        let mut parens = 0usize;
        loop {
            match self.peek() {
                None => return Err(self.incomplete("arithmetic expansion", opened)),
                Some(b')') if parens == 0 => {
                    let end = self.position.offset;
                    if let Some(after) = self.logical_match(b"))") {
                        self.advance_to(after);
                        return Ok(self.source(start, end));
                    }
                    self.bump();
                }
                Some(b'(') => {
                    parens += 1;
                    self.bump();
                }
                Some(b')') => {
                    parens -= 1;
                    self.bump();
                }
                Some(b'$') => {
                    self.dollar(depth + 1, true)?;
                }
                Some(b'`') => {
                    self.backtick()?;
                }
                Some(b'\\') => {
                    self.bump();
                    self.bump();
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    fn backtick(&mut self) -> Result<WordPart, LexError> {
        let opened = self.position;
        self.bump();
        let start = self.position.offset;
        loop {
            match self.peek() {
                None => return Err(self.incomplete("backtick substitution", opened)),
                Some(b'`') => {
                    let source = self.source(start, self.position.offset);
                    self.bump();
                    return Ok(WordPart::CommandSub(CommandSub {
                        style: CommandSubStyle::Backtick,
                        source,
                    }));
                }
                Some(b'\\') => {
                    self.bump();
                    if matches!(self.peek(), Some(b'$' | b'`' | b'\\' | b'\n')) {
                        self.bump();
                    }
                }
                _ => {
                    self.bump();
                }
            }
        }
    }
}

fn is_boundary(byte: u8) -> bool {
    matches!(
        byte,
        b' ' | b'\t' | b'\n' | b'&' | b'|' | b';' | b'(' | b')' | b'<' | b'>'
    )
}

#[cfg(test)]
mod tests;
