//! Byte-at-a-time reads preserve subsequent consumers of nonseekable stdin.
use super::Outcome;
use crate::env::{ShellEnv, valid_name};
fn fields(bytes: &[(u8, bool)], ifs: &[u8], count: usize) -> Vec<Vec<u8>> {
    let sep = |b: &(u8, bool)| !b.1 && ifs.contains(&b.0);
    let white = |b: &(u8, bool)| sep(b) && matches!(b.0, b' ' | b'\t' | b'\n');
    let mut start = 0;
    let mut end = bytes.len();
    while bytes.get(start).is_some_and(white) {
        start += 1;
    }
    while end > start && bytes.get(end - 1).is_some_and(white) {
        end -= 1;
    }
    let mut out = Vec::new();
    for i in 0..count {
        if i + 1 == count {
            out.push(
                bytes
                    .get(start..end)
                    .unwrap_or_default()
                    .iter()
                    .map(|b| b.0)
                    .collect(),
            );
            break;
        }
        let mut pos = start;
        while pos < end && bytes.get(pos).is_some_and(|b| !sep(b)) {
            pos += 1;
        }
        out.push(
            bytes
                .get(start..pos)
                .unwrap_or_default()
                .iter()
                .map(|b| b.0)
                .collect(),
        );
        if pos < end {
            let ws = bytes.get(pos).is_some_and(white);
            pos += 1;
            if ws {
                while pos < end && bytes.get(pos).is_some_and(white) {
                    pos += 1;
                }
                if pos < end && bytes.get(pos).is_some_and(|b| sep(b) && !white(b)) {
                    pos += 1;
                }
            }
            while pos < end && bytes.get(pos).is_some_and(white) {
                pos += 1;
            }
        }
        start = pos;
    }
    out
}
pub fn run(args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    let raw = args.first().is_some_and(|a| a == b"-r");
    let args = if raw {
        args.get(1..).unwrap_or_default()
    } else {
        args
    };
    let args = if args.first().is_some_and(|a| a == b"--") {
        args.get(1..).unwrap_or_default()
    } else {
        args
    };
    if args.is_empty() || args.iter().any(|a| !valid_name(a)) {
        return Outcome::error(b"read", b"valid variable names required", 2);
    }
    let mut bytes = Vec::new();
    let mut eof = true;
    loop {
        match crate::sys::read_byte(0) {
            Ok(Some(b'\n')) => {
                if env.stdin_source {
                    env.stdin_lines += 1;
                }
                eof = false;
                break;
            }
            Ok(Some(b'\\')) if !raw => match crate::sys::read_byte(0) {
                Ok(Some(b'\n')) => {
                    if env.stdin_source {
                        env.stdin_lines += 1;
                    }
                }
                Ok(Some(b)) => bytes.push((b, true)),
                _ => break,
            },
            Ok(Some(b)) => {
                if b != 0 {
                    bytes.push((b, false));
                }
            }
            Ok(None) => break,
            Err(e) => return Outcome::error(b"read", e.to_string().as_bytes(), 1),
        }
    }
    let values = fields(&bytes, env.get(b"IFS").unwrap_or(b" \t\n"), args.len());
    for (name, value) in args.iter().zip(values) {
        if let Err(e) = env.assign(name.clone(), value) {
            return Outcome::error(b"read", e.as_bytes(), 1);
        }
    }
    Outcome {
        status: u8::from(eof),
        ..Outcome::default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remainder() {
        let s = b" a  b c ".iter().map(|b| (*b, false)).collect::<Vec<_>>();
        assert_eq!(fields(&s, b" ", 2), vec![b"a".to_vec(), b"b c".to_vec()]);
    }
}
