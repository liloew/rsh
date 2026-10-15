//! Canonical source rendering. Formatting and separators are not AST semantics.
use super::*;
use std::fmt;

/// Lossless source serialization, including non-UTF-8 words.
/// Display is a textual preview for non-UTF-8 data; use this byte API for reparsing.
impl Script {
    pub fn to_source_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        script(self, &mut out, &mut Vec::new());
        out
    }
}

impl fmt::Display for Script {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.to_source_bytes()))
    }
}

impl Word {
    pub fn to_source_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        parts(&self.0, &mut out);
        out
    }
}
impl fmt::Display for Word {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = Vec::new();
        parts(&self.0, &mut out);
        f.write_str(&String::from_utf8_lossy(&out))
    }
}

fn script<'a>(value: &'a Script, out: &mut Vec<u8>, pending: &mut Vec<&'a HereDoc>) {
    for item in &value.0 {
        pipeline(&item.and_or.first, out, pending);
        for (op, pipe) in &item.and_or.rest {
            out.extend_from_slice(match op {
                AndOrOp::And => b" && ",
                AndOrOp::Or => b" || ",
            });
            pipeline(pipe, out, pending);
        }
        out.extend_from_slice(match item.separator {
            Separator::Sequential => b"; ",
            Separator::Background => b" & ",
        });
        out.push(b'\n');
        for document in pending.drain(..) {
            out.extend_from_slice(&document.raw);
            out.extend_from_slice(&document.delimiter);
            out.push(b'\n');
        }
    }
}

fn pipeline<'a>(value: &'a Pipeline, out: &mut Vec<u8>, pending: &mut Vec<&'a HereDoc>) {
    if value.negated {
        out.extend_from_slice(b"! ");
    }
    for (i, command) in value.commands.iter().enumerate() {
        if i != 0 {
            out.extend_from_slice(b" | ");
        }
        match command {
            Command::Simple(simple) => {
                for (i, part) in simple.parts.iter().enumerate() {
                    if i != 0 {
                        out.push(b' ');
                    }
                    match part {
                        SimplePart::Assignment(a) => {
                            out.extend_from_slice(&a.name);
                            out.push(b'=');
                            parts(&a.value.0, out);
                        }
                        SimplePart::Word(w) => parts(&w.0, out),
                        SimplePart::Redirection(r) => redirection(r, out, pending),
                    }
                }
            }
            Command::Compound(compound, redirs) => {
                compound_source(compound, out, pending);
                for r in redirs {
                    out.push(b' ');
                    redirection(r, out, pending);
                }
            }
        }
    }
}

fn redirection<'a>(value: &'a Redirection, out: &mut Vec<u8>, pending: &mut Vec<&'a HereDoc>) {
    if let Some(fd) = &value.fd {
        out.extend_from_slice(fd);
    }
    out.extend_from_slice(match &value.op {
        RedirOp::Input => b"<",
        RedirOp::Output => b">",
        RedirOp::Append => b">>",
        RedirOp::DupInput => b"<&",
        RedirOp::DupOutput => b">&",
        RedirOp::ReadWrite => b"<>",
        RedirOp::Clobber => b">|",
        RedirOp::HereDoc(document) => {
            pending.push(document);
            if document.strip_tabs { b"<<-" } else { b"<<" }
        }
    });
    out.push(b' ');
    parts(&value.target.0, out);
}

fn compound_source<'a>(
    value: &'a CompoundCommand,
    out: &mut Vec<u8>,
    pending: &mut Vec<&'a HereDoc>,
) {
    match value {
        CompoundCommand::Brace(body) => {
            out.extend_from_slice(b"{ ");
            script(body, out, pending);
            out.push(b'}');
        }
        CompoundCommand::Subshell(body) => {
            // POSIX 2.9.4.1: avoid ambiguous adjacent leading parentheses.
            out.extend_from_slice(b"( ");
            script(body, out, pending);
            out.push(b')');
        }
        CompoundCommand::If {
            branches,
            else_body,
        } => {
            for (i, branch) in branches.iter().enumerate() {
                out.extend_from_slice(if i == 0 { b"if " } else { b"elif " });
                script(&branch.condition, out, pending);
                out.extend_from_slice(b"then ");
                script(&branch.body, out, pending);
            }
            if let Some(body) = else_body {
                out.extend_from_slice(b"else ");
                script(body, out, pending);
            }
            out.extend_from_slice(b"fi");
        }
        CompoundCommand::While { condition, body } | CompoundCommand::Until { condition, body } => {
            out.extend_from_slice(if matches!(value, CompoundCommand::While { .. }) {
                b"while "
            } else {
                b"until "
            });
            script(condition, out, pending);
            out.extend_from_slice(b"do ");
            script(body, out, pending);
            out.extend_from_slice(b"done");
        }
        CompoundCommand::For { name, words, body } => {
            out.extend_from_slice(b"for ");
            out.extend_from_slice(name);
            if let Some(words) = words {
                out.extend_from_slice(b" in");
                for w in words {
                    out.push(b' ');
                    parts(&w.0, out);
                }
            }
            out.extend_from_slice(b"; do ");
            script(body, out, pending);
            out.extend_from_slice(b"done");
        }
        CompoundCommand::Case { word, items } => {
            out.extend_from_slice(b"case ");
            parts(&word.0, out);
            out.extend_from_slice(b" in ");
            for item in items {
                out.push(b'(');
                for (i, pattern) in item.patterns.iter().enumerate() {
                    if i != 0 {
                        out.push(b'|');
                    }
                    parts(&pattern.0, out);
                }
                out.extend_from_slice(b") ");
                script(&item.body, out, pending);
                out.extend_from_slice(b";; ");
            }
            out.extend_from_slice(b"esac");
        }
    }
}

fn parts(value: &[WordPart], out: &mut Vec<u8>) {
    for part in value {
        match part {
            WordPart::Literal(bytes) => out.extend_from_slice(bytes),
            WordPart::Escaped(byte) => {
                out.push(b'\\');
                out.push(*byte);
            }
            WordPart::SingleQuoted(bytes) => {
                out.push(b'\'');
                out.extend_from_slice(bytes);
                out.push(b'\'');
            }
            WordPart::DoubleQuoted(inner) => {
                out.push(b'"');
                parts(inner, out);
                out.push(b'"');
            }
            WordPart::Param(param) => {
                out.push(b'$');
                match param {
                    ParamExpr::Name(name) => out.extend_from_slice(name),
                    ParamExpr::Special(byte) => out.push(*byte),
                    ParamExpr::Braced(body) => {
                        out.push(b'{');
                        out.extend_from_slice(body);
                        out.push(b'}');
                    }
                }
            }
            WordPart::CommandSub(sub) => match sub.style {
                CommandSubStyle::DollarParen => {
                    out.extend_from_slice(b"$(");
                    out.extend_from_slice(&sub.source);
                    out.push(b')');
                }
                CommandSubStyle::Backtick => {
                    out.push(b'`');
                    out.extend_from_slice(&sub.source);
                    out.push(b'`');
                }
            },
            WordPart::Arith(body) => {
                out.extend_from_slice(b"$((");
                out.extend_from_slice(body);
                out.extend_from_slice(b"))");
            }
            WordPart::Tilde(user) => {
                out.push(b'~');
                if let Some(user) = user {
                    out.extend_from_slice(user);
                }
            }
            WordPart::Glob(glob) => out.push(match glob {
                GlobChar::Star => b'*',
                GlobChar::Question => b'?',
                GlobChar::OpenBracket => b'[',
            }),
        }
    }
}
