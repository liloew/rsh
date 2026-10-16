//! POSIX argument-count rules before expression parsing resolve operator/data ambiguity.
use super::Outcome;
use crate::env::ShellEnv;
use std::os::unix::ffi::OsStrExt;
fn integer(s: &[u8]) -> Result<i64, &'static str> {
    std::str::from_utf8(s.trim_ascii())
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or("integer expression expected")
}
pub(crate) fn unary(op: &[u8], s: &[u8], env: &ShellEnv) -> Result<bool, &'static str> {
    // Joining an empty operand would silently turn it into the current directory.
    // File predicates must instead test the supplied empty pathname (false).
    if s.is_empty()
        && matches!(
            op,
            b"-e" | b"-f" | b"-d" | b"-r" | b"-w" | b"-x" | b"-s" | b"-L" | b"-h"
        )
    {
        return Ok(false);
    }
    let path = env.cwd.join(std::ffi::OsStr::from_bytes(s));
    let meta = std::fs::metadata(&path).ok();
    Ok(match op {
        b"-z" => s.is_empty(),
        b"-n" => !s.is_empty(),
        b"-e" => meta.is_some(),
        b"-f" => meta.is_some_and(|m| m.is_file()),
        b"-d" => meta.is_some_and(|m| m.is_dir()),
        b"-s" => meta.is_some_and(|m| m.len() > 0),
        b"-L" | b"-h" => std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()),
        b"-r" => crate::sys::accessible(&path, 4),
        b"-w" => crate::sys::accessible(&path, 2),
        b"-x" => crate::sys::accessible(&path, 1),
        b"-t" => integer(s)
            .ok()
            .and_then(|n| i32::try_from(n).ok())
            .is_some_and(crate::sys::terminal),
        _ => return Err("unknown unary operator"),
    })
}
fn binary(a: &[u8], op: &[u8], b: &[u8]) -> Option<Result<bool, &'static str>> {
    Some(match op {
        b"=" => Ok(a == b),
        b"!=" => Ok(a != b),
        b"-eq" | b"-ne" | b"-lt" | b"-le" | b"-gt" | b"-ge" => integer(a).and_then(|a| {
            integer(b).map(|b| match op {
                b"-eq" => a == b,
                b"-ne" => a != b,
                b"-lt" => a < b,
                b"-le" => a <= b,
                b"-gt" => a > b,
                _ => a >= b,
            })
        }),
        _ => return None,
    })
}
fn count(args: &[Vec<u8>], e: &ShellEnv) -> Option<Result<bool, &'static str>> {
    Some(match args {
        [] => Ok(false),
        [a] => Ok(!a.is_empty()),
        [a, b] if a == b"!" => Ok(b.is_empty()),
        [a, b] => unary(a, b, e),
        [a, op, b] if binary(a, op, b).is_some() => binary(a, op, b)?,
        [a, b, c] if a == b"(" && c == b")" => Ok(!b.is_empty()),
        [a, rest @ ..] if a == b"!" && args.len() <= 4 => evaluate(rest, e).map(|v| !v),
        [a, b, c, d] if a == b"(" && d == b")" => count(&[b.clone(), c.clone()], e)?,
        _ => return None,
    })
}
struct Parser<'a> {
    a: &'a [Vec<u8>],
    i: usize,
    e: &'a ShellEnv,
    depth: usize,
}
impl Parser<'_> {
    fn take(&mut self, s: &[u8]) -> bool {
        if self.a.get(self.i).is_some_and(|v| v == s) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn or(&mut self) -> Result<bool, &'static str> {
        let mut v = self.and()?;
        while self.take(b"-o") {
            v = self.and()? || v;
        }
        Ok(v)
    }
    fn and(&mut self) -> Result<bool, &'static str> {
        let mut v = self.term()?;
        while self.take(b"-a") {
            v = self.term()? && v;
        }
        Ok(v)
    }
    fn term(&mut self) -> Result<bool, &'static str> {
        self.depth += 1;
        if self.depth > 64 {
            return Err("expression nesting limit");
        }
        let r = if self.take(b"!") {
            self.term().map(|v| !v)
        } else if self.take(b"(") {
            let v = self.or()?;
            if !self.take(b")") {
                return Err("missing )");
            }
            Ok(v)
        } else {
            let a = self.a.get(self.i).ok_or("missing operand")?;
            self.i += 1;
            if let (Some(op), Some(b)) = (self.a.get(self.i), self.a.get(self.i + 1))
                && let Some(v) = binary(a, op, b)
            {
                self.i += 2;
                v
            } else if matches!(
                a.as_slice(),
                b"-e"
                    | b"-f"
                    | b"-d"
                    | b"-r"
                    | b"-w"
                    | b"-x"
                    | b"-s"
                    | b"-L"
                    | b"-h"
                    | b"-t"
                    | b"-z"
                    | b"-n"
            ) {
                let b = self.a.get(self.i).ok_or("missing operand")?;
                self.i += 1;
                unary(a, b, self.e)
            } else {
                Ok(!a.is_empty())
            }
        };
        self.depth -= 1;
        r
    }
}
fn evaluate(args: &[Vec<u8>], env: &ShellEnv) -> Result<bool, &'static str> {
    if let Some(v) = count(args, env) {
        v
    } else {
        let mut p = Parser {
            a: args,
            i: 0,
            e: env,
            depth: 0,
        };
        p.or().and_then(|v| {
            if p.i == args.len() {
                Ok(v)
            } else {
                Err("unexpected operand")
            }
        })
    }
}
pub fn run(args: &[Vec<u8>], bracket: bool, env: &ShellEnv) -> Outcome {
    let args = if bracket {
        if args.last().is_none_or(|v| v != b"]") {
            return Outcome::error(b"[", b"missing ]", 2);
        }
        args.get(..args.len().saturating_sub(1)).unwrap_or_default()
    } else {
        args
    };
    let result = evaluate(args, env);
    match result {
        Ok(v) => Outcome {
            status: u8::from(!v),
            ..Outcome::default()
        },
        Err(m) => Outcome::error(b"test", m.as_bytes(), 2),
    }
}
