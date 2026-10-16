use super::completion::{Context, PathCache};
use crate::lexer::{self, Operator, TokenKind};
use std::sync::{Arc, Mutex};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Plain,
    Command,
    Unknown,
    String,
    Expansion,
    Operator,
    Redirect,
    Comment,
    Incomplete,
    Keyword,
}
impl Kind {
    pub fn style(self) -> nu_ansi_term::Style {
        use nu_ansi_term::{Color, Style};
        match self {
            Self::Plain => Style::new(),
            Self::Command => Color::Green.bold(),
            Self::Unknown | Self::Incomplete => Color::Red.normal(),
            Self::String => Color::Yellow.normal(),
            Self::Expansion => Color::Magenta.normal(),
            Self::Operator => Color::Cyan.bold(),
            Self::Redirect => Color::Blue.bold(),
            Self::Comment => Color::DarkGray.normal(),
            Self::Keyword => Color::Purple.bold(),
        }
    }
}
fn keyword(s: &[u8]) -> bool {
    matches!(
        s,
        b"if"
            | b"then"
            | b"elif"
            | b"else"
            | b"fi"
            | b"while"
            | b"until"
            | b"do"
            | b"done"
            | b"for"
            | b"in"
            | b"case"
            | b"esac"
            | b"{"
            | b"}"
            | b"!"
    )
}
pub fn classify(line: &str, known: impl Fn(&str) -> bool) -> Vec<Kind> {
    let mut kinds = vec![Kind::Plain; line.len()];
    let input = line.as_bytes();
    let parsed = crate::parser::parse(input);
    let mut end = input.len();
    let tokens = loop {
        match lexer::lex(input.get(..end).unwrap_or_default()) {
            Ok(t) => break t,
            Err(_) => {
                if end == 0 {
                    break Vec::new();
                }
                end -= 1;
            }
        }
    };
    let mut command = true;
    let mut target = false;
    for t in tokens {
        let start = t.span.start.offset;
        let end = t.span.end.offset.min(input.len());
        let raw = input.get(start..end).unwrap_or_default();
        let kind = match t.kind {
            TokenKind::Newline => {
                command = true;
                Kind::Operator
            }
            TokenKind::IoNumber(_) => Kind::Redirect,
            TokenKind::Operator(op) => {
                if matches!(
                    op,
                    Operator::Less
                        | Operator::Greater
                        | Operator::Append
                        | Operator::Clobber
                        | Operator::DupInput
                        | Operator::DupOutput
                        | Operator::ReadWrite
                        | Operator::HereDoc
                        | Operator::HereString
                        | Operator::HereDocStrip
                ) {
                    target = true;
                    Kind::Redirect
                } else {
                    command = true;
                    Kind::Operator
                }
            }
            _ => {
                if target {
                    target = false;
                    Kind::Plain
                } else if command && keyword(raw) {
                    command = !matches!(raw, b"for" | b"case");
                    Kind::Keyword
                } else if command && raw.contains(&b'=') {
                    Kind::Plain
                } else if command {
                    command = false;
                    if let Ok(name) = std::str::from_utf8(raw) {
                        if known(name) {
                            Kind::Command
                        } else {
                            Kind::Unknown
                        }
                    } else {
                        Kind::Plain
                    }
                } else {
                    Kind::Plain
                }
            }
        };
        if let Some(slice) = kinds.get_mut(start..end) {
            slice.fill(kind);
        }
        if matches!(
            t.kind,
            TokenKind::Word(_) | TokenKind::HereDocDelimiter { .. }
        ) {
            let mut quote = None;
            let mut escaped = false;
            let mut expansion_until = 0;
            for i in start..end {
                let Some(&b) = input.get(i) else {
                    break;
                };
                let k = if i < expansion_until {
                    Kind::Expansion
                } else if quote == Some(b'\'') {
                    if b == b'\'' {
                        quote = None;
                    }
                    Kind::String
                } else if escaped {
                    escaped = false;
                    Kind::String
                } else if b == b'\\' {
                    escaped = true;
                    Kind::String
                } else if matches!(b, b'\'' | b'"') {
                    if quote == Some(b) {
                        quote = None;
                    } else if quote.is_none() {
                        quote = Some(b);
                    }
                    Kind::String
                } else if quote != Some(b'\'') && b == b'`' {
                    expansion_until = input
                        .get(i + 1..end)
                        .and_then(|s| s.iter().position(|b| *b == b'`'))
                        .map_or(end, |n| i + n + 2);
                    Kind::Expansion
                } else if quote != Some(b'\'') && b == b'$' {
                    expansion_until = expansion_end(input, i, end);
                    Kind::Expansion
                } else if quote.is_some() {
                    Kind::String
                } else {
                    kind
                };
                if let Some(v) = kinds.get_mut(i) {
                    *v = k;
                }
            }
        }
    }
    // Comments are lexer gaps. A # in a WORD already has a non-plain token span.
    let mut comment = false;
    for (i, &b) in input.iter().enumerate() {
        if b == b'\n' {
            comment = false;
        }
        if b == b'#'
            && kinds.get(i) == Some(&Kind::Plain)
            && (i == 0 || input.get(i - 1).is_some_and(u8::is_ascii_whitespace))
        {
            comment = true;
        }
        if comment && let Some(k) = kinds.get_mut(i) {
            *k = Kind::Comment;
        }
    }
    if matches!(parsed, Err(crate::parser::ParseError::Incomplete(_))) {
        // The successful token prefix ends before an unfinished quote/substitution.
        if end < line.len()
            && let Some(tail) = kinds.get_mut(end..)
        {
            tail.fill(Kind::Incomplete);
        }
    }
    if let Err(crate::parser::ParseError::Syntax { line: row, col, .. }) = parsed {
        let mut row_now = 1;
        let mut col_now = 1;
        for (i, b) in input.iter().enumerate() {
            if row_now == row
                && col_now == col
                && let Some(k) = kinds.get_mut(i)
            {
                *k = Kind::Unknown;
                break;
            }
            if *b == b'\n' {
                row_now += 1;
                col_now = 1;
            } else {
                col_now += 1;
            }
        }
    }
    kinds
}
// Only styling boundaries are scanned here; tokenization and validity above
// always come from the shell lexer/parser. No expansion is evaluated.
fn expansion_end(input: &[u8], start: usize, end: usize) -> usize {
    let next = input.get(start + 1).copied();
    if matches!(next, Some(b'{' | b'(')) {
        let open = next.unwrap_or_default();
        let close = if open == b'{' { b'}' } else { b')' };
        let mut depth = 1usize;
        let mut quote = None;
        let mut escaped = false;
        for i in start + 2..end {
            let Some(&b) = input.get(i) else {
                break;
            };
            if escaped {
                escaped = false;
                continue;
            }
            if b == b'\\' && quote != Some(b'\'') {
                escaped = true;
                continue;
            }
            if matches!(b, b'\'' | b'"') {
                if quote == Some(b) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(b);
                }
                continue;
            }
            if quote.is_some() {
                continue;
            }
            if b == open {
                depth += 1;
            } else if b == close {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
        }
        end
    } else if next.is_some_and(|b| b.is_ascii_alphabetic() || b == b'_') {
        let mut i = start + 2;
        while i < end
            && input
                .get(i)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            i += 1;
        }
        i
    } else {
        (start + 2).min(end)
    }
}
pub struct Highlight {
    pub context: Arc<Mutex<Context>>,
    pub cache: PathCache,
}
impl reedline::Highlighter for Highlight {
    fn highlight(&self, line: &str, _cursor: usize) -> reedline::StyledText {
        let kinds = if let Ok(ctx) = self.context.lock() {
            self.cache.refresh(
                ctx.variables
                    .get("PATH")
                    .map(String::as_bytes)
                    .unwrap_or(b"/usr/bin:/bin"),
                &ctx.cwd,
            );
            let external = self.cache.commands();
            classify(line, |s| {
                ctx.commands.contains(s)
                    || external.contains(s)
                    || self.cache.known_path(s, &ctx.cwd)
            })
        } else {
            classify(line, |_| false)
        };
        let mut out = reedline::StyledText::new();
        let mut current = Kind::Plain;
        let mut text = String::new();
        for (i, c) in line.char_indices() {
            let kind = kinds.get(i).copied().unwrap_or(Kind::Plain);
            if kind != current && !text.is_empty() {
                out.push((current.style(), std::mem::take(&mut text)));
            }
            current = kind;
            text.push(c);
        }
        if !text.is_empty() {
            out.push((current.style(), text));
        }
        out
    }
}
