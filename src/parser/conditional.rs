//! Conditional operators are syntax here; operands remain unexpanded words.
use super::*;
impl Parser<'_> {
    pub(super) fn conditional_marker(&self, text: &[u8]) -> bool {
        self.keyword(text)
            || matches!((text, self.peek().map(|t| &t.kind)),
            (b"[[", Some(TokenKind::Word(Word(parts)))) if matches!(parts.as_slice(), [WordPart::Glob(GlobChar::OpenBracket), WordPart::Glob(GlobChar::OpenBracket)]))
            || matches!(
                (text, self.peek().map(|t| &t.kind)),
                (b"<", Some(TokenKind::Operator(Op::Less)))
                    | (b">", Some(TokenKind::Operator(Op::Greater)))
            )
    }
    pub(super) fn conditional(&mut self) -> Result<Conditional, ParseError> {
        // Bound total expression nodes as well as parenthesis/negation depth.
        let count = self
            .tokens
            .get(self.index..)
            .unwrap_or_default()
            .iter()
            .take_while(|t| self.input.get(t.span.start.offset..t.span.end.offset) != Some(b"]]"))
            .filter(|t| {
                !matches!(
                    t.kind,
                    TokenKind::Newline | TokenKind::Operator(Op::LeftParen | Op::RightParen)
                )
            })
            .take(MAX_NESTING + 1)
            .count();
        if count > MAX_NESTING {
            return Err(ParseError::NestingLimit);
        }
        self.linebreak();
        let expr = if self.conditional_marker(b"]]") {
            return Err(self.error("conditional expression"));
        } else {
            self.conditional_or(0)?
        };
        self.linebreak();
        if !self.conditional_marker(b"]]") {
            return Err(self.error("]]"));
        }
        self.advance();
        Ok(expr)
    }
    fn conditional_or(&mut self, depth: usize) -> Result<Conditional, ParseError> {
        let mut expr = self.conditional_and(depth)?;
        while self.take_op(Op::OrIf) {
            self.linebreak();
            expr = Conditional::Or(Box::new(expr), Box::new(self.conditional_and(depth)?));
        }
        Ok(expr)
    }
    fn conditional_and(&mut self, depth: usize) -> Result<Conditional, ParseError> {
        let mut expr = self.conditional_term(depth)?;
        while self.take_op(Op::AndIf) {
            self.linebreak();
            expr = Conditional::And(Box::new(expr), Box::new(self.conditional_term(depth)?));
        }
        Ok(expr)
    }
    fn conditional_term(&mut self, depth: usize) -> Result<Conditional, ParseError> {
        if depth >= MAX_NESTING {
            return Err(ParseError::NestingLimit);
        }
        self.linebreak();
        if self.take_keyword(b"!") {
            return Ok(Conditional::Not(Box::new(
                self.conditional_term(depth + 1)?,
            )));
        }
        if self.take_op(Op::LeftParen) {
            self.linebreak();
            let expr = self.conditional_or(depth + 1)?;
            self.expect_op(Op::RightParen)?;
            return Ok(expr);
        }
        if self.conditional_marker(b"]]") {
            return Err(self.error("conditional operand"));
        }
        let unary = [
            b"-a".as_slice(),
            b"-b",
            b"-c",
            b"-d",
            b"-e",
            b"-f",
            b"-g",
            b"-h",
            b"-k",
            b"-L",
            b"-N",
            b"-O",
            b"-G",
            b"-p",
            b"-r",
            b"-s",
            b"-S",
            b"-t",
            b"-u",
            b"-w",
            b"-x",
            b"-z",
            b"-n",
            b"-v",
            b"-o",
            b"-R",
        ];
        if let Some(op) = unary.iter().find(|op| self.keyword(op)).copied() {
            self.advance();
            return Ok(Conditional::Unary {
                op: op.to_vec(),
                word: self.conditional_word(false)?,
            });
        }
        let left = self.conditional_word(false)?;
        let ops = [
            b"=".as_slice(),
            b"==",
            b"!=",
            b"=~",
            b"<",
            b">",
            b"-eq",
            b"-ne",
            b"-lt",
            b"-le",
            b"-gt",
            b"-ge",
            b"-nt",
            b"-ot",
            b"-ef",
        ];
        if let Some(op) = ops.iter().find(|op| self.conditional_marker(op)).copied() {
            self.advance();
            let right = self.conditional_word(op == b"=~")?;
            return Ok(Conditional::Binary {
                left,
                op: op.to_vec(),
                right,
            });
        }
        if self.newline() {
            return Err(self.error("conditional operator"));
        }
        Ok(Conditional::Word(left))
    }
    fn conditional_word(&mut self, regex: bool) -> Result<Word, ParseError> {
        if self.conditional_marker(b"]]") {
            return Err(self.error("conditional operand"));
        }
        let mut parts = Vec::new();
        let mut end = None;
        loop {
            let Some(token) = self.peek() else {
                return if parts.is_empty() {
                    Err(self.error("conditional operand"))
                } else {
                    Ok(Word(parts))
                };
            };
            if end.is_some_and(|end| end != token.span.start.offset) {
                break;
            }
            match &token.kind {
                TokenKind::Word(word) => parts.extend(word.0.clone()),
                TokenKind::IoNumber(bytes) => parts.push(WordPart::Literal(bytes.clone())),
                TokenKind::Operator(_) if regex => {
                    let raw = self
                        .input
                        .get(token.span.start.offset..token.span.end.offset)
                        .unwrap_or_default();
                    parts.push(WordPart::Literal(raw.to_vec()));
                }
                _ if parts.is_empty() => return Err(self.error("conditional operand")),
                _ => break,
            }
            end = Some(token.span.end.offset);
            self.advance();
            if !regex {
                break;
            }
        }
        Ok(Word(parts))
    }
}
