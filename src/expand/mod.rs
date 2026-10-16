//! Execution-time, byte-preserving expansion (SPEC 5; POSIX 2.6).
pub mod arith;
pub mod pattern;
use crate::ast::{GlobChar, ParamExpr, Word, WordPart};
use crate::env::ShellEnv;
use std::os::unix::ffi::OsStrExt;
#[derive(Debug, thiserror::Error)]
#[error("rsh: expansion: {message}")]
pub struct ExpandError {
    pub message: String,
    pub status: u8,
    pub abort_line: bool,
}
impl ExpandError {
    pub fn readonly(s: impl Into<String>) -> Self {
        Self {
            message: s.into(),
            status: 1,
            abort_line: false,
        }
    }
    pub fn new(s: impl Into<String>) -> Self {
        Self {
            message: s.into(),
            status: 2,
            abort_line: false,
        }
    }
}
pub type Substitute<'a> = dyn FnMut(&[u8], &ShellEnv) -> Result<(Vec<u8>, u8), ExpandError> + 'a;
#[derive(Clone, Copy, Debug)]
struct Byte {
    value: u8,
    quoted: bool,
    split: bool,
    empty_quote: bool,
}
#[derive(Clone, Default, Debug)]
struct Field {
    bytes: Vec<Byte>,
}
impl Field {
    fn append(&mut self, s: &[u8], quoted: bool, split: bool) {
        if s.is_empty() && quoted {
            self.bytes.push(Byte {
                value: 0,
                quoted: true,
                split: false,
                empty_quote: true,
            });
        }
        self.bytes.extend(s.iter().map(|&value| Byte {
            value,
            quoted,
            split,
            empty_quote: false,
        }));
    }
    fn raw(&self) -> Vec<u8> {
        self.bytes
            .iter()
            .filter(|b| !b.empty_quote)
            .map(|b| b.value)
            .collect()
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Argument,
    Scalar,
    Assignment,
    HereDoc,
    HereString,
    Pattern,
}
pub fn word(
    word: &Word,
    env: &mut ShellEnv,
    sub: &mut Substitute<'_>,
    mode: Mode,
) -> Result<Vec<Vec<u8>>, ExpandError> {
    let fields = render(&word.0, env, sub, false, mode, 0)?;
    let mut out = Vec::new();
    for field in fields {
        let fields = if mode == Mode::Argument {
            split(field, env.get(b"IFS").unwrap_or(b" \t\n"))
        } else {
            vec![field]
        };
        for f in fields {
            if mode == Mode::Argument && !env.noglob {
                out.extend(glob(&f, env)?);
            } else {
                out.push(f.raw());
            }
        }
    }
    Ok(out)
}
pub fn scalar(
    word: &Word,
    env: &mut ShellEnv,
    sub: &mut Substitute<'_>,
    mode: Mode,
) -> Result<Vec<u8>, ExpandError> {
    Ok(self::word(word, env, sub, mode)?.join(b" ".as_slice()))
}
/// Preserve quote bits through expansion for case and parameter patterns alike.
pub fn compile_pattern(
    word: &Word,
    env: &mut ShellEnv,
    sub: &mut Substitute<'_>,
) -> Result<Vec<pattern::Atom>, ExpandError> {
    let fields = render(&word.0, env, sub, false, Mode::Pattern, 0)?;
    Ok(pattern::compile(
        &fields
            .iter()
            .flat_map(|f| {
                f.bytes
                    .iter()
                    .filter(|b| !b.empty_quote)
                    .map(|b| (b.value, b.quoted))
            })
            .collect::<Vec<_>>(),
    ))
}
// Pure compatibility helper for conservative descriptor reservation. Never invokes substitutions.
pub fn basic_word(word: &Word, env: &ShellEnv) -> Result<Vec<u8>, ExpandError> {
    let mut env = env.clone();
    scalar(
        word,
        &mut env,
        &mut |_, _| Err(ExpandError::new("substitution requires execution context")),
        Mode::Scalar,
    )
}
fn push_value(fields: &mut Vec<Field>, value: &[u8], quoted: bool, split: bool) {
    if fields.is_empty() {
        fields.push(Field::default());
    }
    if let Some(f) = fields.last_mut() {
        f.append(value, quoted, split);
    }
}
fn render(
    parts: &[WordPart],
    env: &mut ShellEnv,
    sub: &mut Substitute<'_>,
    quoted: bool,
    mode: Mode,
    depth: usize,
) -> Result<Vec<Field>, ExpandError> {
    if depth > 64 {
        return Err(ExpandError::new("expansion nesting limit"));
    }
    let mut fields = vec![Field::default()];
    for (part_index, part) in parts.iter().enumerate() {
        match part {
            WordPart::Literal(s) => {
                if mode == Mode::Assignment && !quoted {
                    assignment_literal(
                        &mut fields,
                        s,
                        env,
                        parts.get(part_index + 1).is_none(),
                        part_index == 0
                            || matches!(parts.get(part_index.saturating_sub(1)),Some(WordPart::Literal(v)) if v.last()==Some(&b':')),
                    );
                } else {
                    push_value(&mut fields, s, quoted, false);
                }
            }
            WordPart::SingleQuoted(s) => {
                push_value(&mut fields, s, true, false);
            }
            WordPart::Escaped(b) => push_value(&mut fields, &[*b], true, false),
            WordPart::DoubleQuoted(ps) => {
                let inner = render(ps, env, sub, true, mode, depth + 1)?;
                let only_at = matches!(ps.as_slice(), [WordPart::Param(ParamExpr::Special(b'@'))])
                    || matches!(ps.as_slice(),[WordPart::Param(ParamExpr::Braced(s))] if s==b"@");
                if !only_at && inner.iter().all(|f| f.bytes.is_empty()) {
                    push_value(&mut fields, b"", true, false);
                }
                merge(&mut fields, inner);
            }
            WordPart::Glob(g) => push_value(
                &mut fields,
                &[match g {
                    GlobChar::Star => b'*',
                    GlobChar::Question => b'?',
                    GlobChar::OpenBracket => b'[',
                }],
                quoted,
                false,
            ),
            WordPart::Tilde(user) => {
                let mut spelling = b"~".to_vec();
                if let Some(u) = user {
                    spelling.extend(u);
                }
                let v = home(user.as_deref(), env).unwrap_or(spelling);
                push_value(&mut fields, &v, true, false);
            }
            WordPart::CommandSub(cmd) => {
                let source = if cmd.style == crate::ast::CommandSubStyle::Backtick {
                    decode_backtick(&cmd.source, quoted)
                } else {
                    cmd.source.clone()
                };
                let (mut value, status) = sub(&source, env)?;
                while value.last() == Some(&b'\n') {
                    value.pop();
                }
                value.retain(|b| *b != 0);
                env.substitution_status = Some(status);
                push_value(&mut fields, &value, quoted, true);
            }
            WordPart::Arith(source) => {
                // Arithmetic source may itself contain parameter/command substitutions.
                let arithmetic_parts = crate::lexer::here_parts(source, 0)
                    .map_err(|e| ExpandError::new(e.to_string()))?;
                let value = render(&arithmetic_parts, env, sub, true, Mode::Scalar, depth + 1)?
                    .into_iter()
                    .flat_map(|f| f.raw())
                    .collect::<Vec<_>>();
                let n = arith::eval(&value, env)?;
                push_value(&mut fields, n.to_string().as_bytes(), quoted, true);
            }
            WordPart::Param(p) => {
                let expr = match p {
                    ParamExpr::Name(n) | ParamExpr::Braced(n) => n.clone(),
                    ParamExpr::Special(c) => vec![*c],
                };
                if (expr == b"@" && matches!(mode, Mode::Argument | Mode::HereString))
                    || (expr == b"*" && !quoted && mode == Mode::Argument)
                {
                    let values = env.positional.clone();
                    for (i, value) in values.iter().enumerate() {
                        if i > 0 {
                            fields.push(Field::default());
                        }
                        push_value(&mut fields, value, quoted, true);
                    }
                } else {
                    let value = parameter(&expr, env, sub, quoted, mode, depth + 1)?;
                    merge(&mut fields, value);
                }
            }
        }
    }
    Ok(fields)
}
fn merge(fields: &mut Vec<Field>, mut values: Vec<Field>) {
    if values.is_empty() {
        return;
    }
    if let Some(first) = values.first_mut()
        && let Some(last) = fields.last_mut()
    {
        last.bytes.append(&mut first.bytes);
    }
    fields.extend(values.into_iter().skip(1));
}
fn home(user: Option<&[u8]>, env: &ShellEnv) -> Option<Vec<u8>> {
    if user.is_none()
        && let Some(h) = env.get(b"HOME")
    {
        return Some(h.to_vec());
    }
    crate::sys::user_home(user)
}
fn assignment_literal(
    fields: &mut Vec<Field>,
    s: &[u8],
    env: &ShellEnv,
    allow_end: bool,
    allow_start: bool,
) {
    let mut i = 0;
    while let Some(&b) = s.get(i) {
        let at_start = if i == 0 {
            allow_start
        } else {
            s.get(i - 1) == Some(&b':')
        };
        if b == b'~' && at_start {
            let end = s
                .get(i + 1..)
                .and_then(|s| s.iter().position(|b| *b == b':' || *b == b'/'))
                .map(|j| i + 1 + j)
                .unwrap_or(s.len());
            if end == s.len() && !allow_end {
                push_value(fields, &[b], false, false);
                i += 1;
                continue;
            }
            let user = s.get(i + 1..end).unwrap_or_default();
            if let Some(h) = home(if user.is_empty() { None } else { Some(user) }, env) {
                push_value(fields, &h, true, false);
                i = end;
                continue;
            }
        }
        push_value(fields, &[b], false, false);
        i += 1;
    }
}
fn parameter_value(name: &[u8], env: &ShellEnv) -> Option<Vec<u8>> {
    Some(match name {
        b"?" => env.last_status.to_string().into_bytes(),
        b"#" => env.positional.len().to_string().into_bytes(),
        b"$" => env.shell_pid.to_string().into_bytes(),
        b"!" => return env.last_background.map(|p| p.to_string().into_bytes()),
        b"-" => env.option_letters(),
        b"0" => env.arg0.clone(),
        b"@" | b"*" => {
            let sep = env
                .get(b"IFS")
                .unwrap_or(b" \t\n")
                .get(..1)
                .unwrap_or_default();
            env.positional.join(sep)
        }
        n if !n.is_empty() && n.iter().all(u8::is_ascii_digit) => {
            let i = std::str::from_utf8(n).ok()?.parse::<usize>().ok()?;
            if i == 0 {
                env.arg0.clone()
            } else {
                return env.positional.get(i - 1).cloned();
            }
        }
        _ => return env.get(name).map(<[u8]>::to_vec),
    })
}
fn parameter(
    expr: &[u8],
    env: &mut ShellEnv,
    sub: &mut Substitute<'_>,
    quoted: bool,
    mode: Mode,
    depth: usize,
) -> Result<Vec<Field>, ExpandError> {
    let length = expr.first() == Some(&b'#') && expr.len() > 1;
    let source = if length {
        expr.get(1..).unwrap_or_default()
    } else {
        expr
    };
    let end = if source
        .first()
        .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
    {
        source
            .iter()
            .position(|b| !b.is_ascii_alphanumeric() && *b != b'_')
            .unwrap_or(source.len())
    } else if source.first().is_some_and(u8::is_ascii_digit) {
        source
            .iter()
            .position(|b| !b.is_ascii_digit())
            .unwrap_or(source.len())
    } else {
        1.min(source.len())
    };
    let name = source.get(..end).unwrap_or_default();
    let tail = source.get(end..).unwrap_or_default();
    let value = parameter_value(name, env);
    if name.is_empty() {
        return Err(ExpandError::new("bad substitution"));
    }
    let mut f = Field::default();
    if length {
        if value.is_none() && env.options.nounset && name != b"@" && name != b"*" {
            return Err(ExpandError::new("parameter not set"));
        }
        if !tail.is_empty() {
            return Err(ExpandError::new("bad substitution"));
        }
        let n = if name == b"@" || name == b"*" {
            env.positional.len()
        } else {
            value.as_ref().map_or(0, Vec::len)
        };
        f.append(n.to_string().as_bytes(), quoted, true);
        return Ok(vec![f]);
    }
    if tail.is_empty() {
        if value.is_none() && env.options.nounset && name != b"@" && name != b"*" {
            return Err(ExpandError::new(format!(
                "{}: parameter not set",
                String::from_utf8_lossy(name)
            )));
        }
        f.append(value.as_deref().unwrap_or_default(), quoted, true);
        return Ok(vec![f]);
    }
    let colon = tail.first() == Some(&b':');
    let tail = if colon {
        tail.get(1..).unwrap_or_default()
    } else {
        tail
    };
    let Some((&op, rest)) = tail.split_first() else {
        return Err(ExpandError::new("bad substitution"));
    };
    let missing = value.is_none() || colon && value.as_ref().is_some_and(Vec::is_empty);
    match op {
        b'-' | b'+' | b'=' | b'?' => {
            let use_word = if op == b'+' { !missing } else { missing };
            if use_word {
                let v = expand_fragment(rest, env, sub, quoted, mode, depth + 1)?;
                let raw = v.iter().flat_map(Field::raw).collect::<Vec<_>>();
                if op == b'?' {
                    return Err(ExpandError::new(format!(
                        "{}: {}",
                        String::from_utf8_lossy(name),
                        if raw.is_empty() {
                            "parameter not set".into()
                        } else {
                            String::from_utf8_lossy(&raw).into_owned()
                        }
                    )));
                }
                if op == b'=' {
                    if !crate::env::valid_name(name) {
                        return Err(ExpandError::new("cannot assign special parameter"));
                    }
                    env.assign(name.to_vec(), raw)
                        .map_err(ExpandError::readonly)?;
                }
                return Ok(v);
            }
            f.append(
                if op != b'+' {
                    value.as_deref().unwrap_or_default()
                } else {
                    b""
                },
                quoted,
                true,
            );
        }
        b'%' | b'#' if !colon => {
            let longest = rest.first() == Some(&op);
            let pat = if longest {
                rest.get(1..).unwrap_or_default()
            } else {
                rest
            };
            let pattern = expand_fragment(pat, env, sub, false, Mode::Pattern, depth + 1)?;
            let pattern = pattern::compile(
                &pattern
                    .iter()
                    .flat_map(|f| {
                        f.bytes
                            .iter()
                            .filter(|b| !b.empty_quote)
                            .map(|b| (b.value, b.quoted))
                    })
                    .collect::<Vec<_>>(),
            );
            let v = value.unwrap_or_default();
            let mut candidates: Vec<usize> = (0..=v.len()).collect();
            if (op == b'#') == longest {
                candidates.reverse();
            }
            let cut = candidates.into_iter().find(|&i| {
                pattern::matches(
                    &pattern,
                    if op == b'#' {
                        v.get(..i).unwrap_or_default()
                    } else {
                        v.get(i..).unwrap_or_default()
                    },
                )
            });
            let result = if let Some(i) = cut {
                if op == b'#' {
                    v.get(i..).unwrap_or_default()
                } else {
                    v.get(..i).unwrap_or_default()
                }
            } else {
                &v
            };
            f.append(result, quoted, true);
        }
        _ => return Err(ExpandError::new("bad substitution")),
    }
    Ok(vec![f])
}
fn expand_fragment(
    s: &[u8],
    env: &mut ShellEnv,
    sub: &mut Substitute<'_>,
    quoted: bool,
    mode: Mode,
    depth: usize,
) -> Result<Vec<Field>, ExpandError> {
    let parts = crate::lexer::expansion_parts(s, quoted && mode != Mode::Pattern)
        .map_err(|e| ExpandError::new(e.to_string()))?;
    let mut result = render(&parts, env, sub, quoted, mode, depth + 1)?;
    // Operand literals are part of an expansion result and are eligible for splitting.
    for f in &mut result {
        for b in &mut f.bytes {
            if !b.quoted {
                b.split = true;
            }
        }
    }
    Ok(result)
}
fn split(field: Field, ifs: &[u8]) -> Vec<Field> {
    if ifs.is_empty() {
        return if field.bytes.is_empty() {
            Vec::new()
        } else {
            vec![field]
        };
    }
    let is_sep = |b: &Byte| b.split && !b.quoted && ifs.contains(&b.value);
    let white = |b: &Byte| is_sep(b) && matches!(b.value, b' ' | b'\t' | b'\n');
    let mut out = Vec::new();
    let mut current = Field::default();
    let mut i = 0;
    while field.bytes.get(i).is_some_and(white) {
        i += 1;
    }
    while let Some(b) = field.bytes.get(i) {
        if is_sep(b) {
            let mut nonwhite = !white(b);
            i += 1;
            if !nonwhite {
                while field.bytes.get(i).is_some_and(white) {
                    i += 1;
                }
                if field.bytes.get(i).is_some_and(|b| is_sep(b) && !white(b)) {
                    nonwhite = true;
                    i += 1;
                }
            }
            while field.bytes.get(i).is_some_and(white) {
                i += 1;
            }
            if nonwhite || !current.bytes.is_empty() {
                out.push(current);
                current = Field::default();
            }
        } else {
            current.bytes.push(*b);
            i += 1;
        }
    }
    if !current.bytes.is_empty() {
        out.push(current);
    }
    out
}
fn glob(field: &Field, env: &ShellEnv) -> Result<Vec<Vec<u8>>, ExpandError> {
    let cwd = &env.cwd;
    let cleaned = Field {
        bytes: field
            .bytes
            .iter()
            .filter(|b| !b.empty_quote)
            .copied()
            .collect(),
    };
    let field = &cleaned;
    if !field
        .bytes
        .iter()
        .any(|b| !b.quoted && matches!(b.value, b'*' | b'?' | b'['))
    {
        return Ok(vec![field.raw()]);
    }
    let absolute = field.bytes.first().is_some_and(|b| b.value == b'/');
    let trailing = field.bytes.last().is_some_and(|b| b.value == b'/');
    let mut paths = vec![if absolute { b"/".to_vec() } else { Vec::new() }];
    let mut has_magic = false;
    for component in field
        .bytes
        .split(|b| b.value == b'/')
        .filter(|s| !s.is_empty())
    {
        let pat = pattern::compile(
            &component
                .iter()
                .map(|b| (b.value, b.quoted))
                .collect::<Vec<_>>(),
        );
        let magic = pat.iter().any(|a| !matches!(a, pattern::Atom::Byte(_)));
        has_magic |= magic;
        let mut next = Vec::new();
        for base in paths {
            let path = if absolute {
                std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&base))
            } else {
                cwd.join(std::ffi::OsStr::from_bytes(&base))
            };
            let names = if magic {
                std::fs::read_dir(path)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .map(|e| e.file_name().as_bytes().to_vec())
                    .filter(|n| {
                        (env.options.dotglob && n != b"." && n != b".."
                            || !n.starts_with(b".")
                            || component.first().is_some_and(|b| b.value == b'.'))
                            && pattern::matches(&pat, n)
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![component.iter().map(|b| b.value).collect()]
            };
            for name in names {
                let mut v = base.clone();
                if !v.is_empty() && !v.ends_with(b"/") {
                    v.push(b'/');
                }
                v.extend(name);
                next.push(v);
            }
        }
        paths = next;
    }
    paths.retain(|p| {
        let p = if absolute {
            std::path::PathBuf::from(std::ffi::OsStr::from_bytes(p))
        } else {
            cwd.join(std::ffi::OsStr::from_bytes(p))
        };
        if trailing {
            p.is_dir()
        } else {
            p.symlink_metadata().is_ok()
        }
    });
    if paths.is_empty() {
        if !has_magic {
            return Ok(vec![field.raw()]);
        }
        if env.options.failglob {
            return Err(ExpandError {
                message: "no match: pathname expansion".to_owned(),
                status: 1,
                abort_line: true,
            });
        }
        Ok(if env.options.nullglob {
            Vec::new()
        } else {
            vec![field.raw()]
        })
    } else {
        if trailing {
            for p in &mut paths {
                p.push(b'/');
            }
        }
        paths.sort();
        Ok(paths)
    }
}
fn decode_backtick(source: &[u8], quoted: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&b) = source.get(i) {
        if b == b'\\'
            && let Some(&n) = source.get(i + 1)
            && (matches!(n, b'$' | b'`' | b'\\' | b'\n') || quoted && n == b'"')
        {
            i += 2;
            if n != b'\n' {
                out.push(n);
            }
            continue;
        }
        out.push(b);
        i += 1;
    }
    out
}

/// Quote-protected regular expression characters must remain literal ERE text.
pub fn regex_pattern(
    word: &Word,
    env: &mut ShellEnv,
    sub: &mut Substitute<'_>,
) -> Result<Vec<u8>, ExpandError> {
    let fields = render(&word.0, env, sub, false, Mode::Pattern, 0)?;
    let mut out = Vec::new();
    for byte in fields
        .iter()
        .flat_map(|field| &field.bytes)
        .filter(|b| !b.empty_quote)
    {
        if byte.quoted && b".[]\\*^$()+?{}|".contains(&byte.value) {
            out.push(b'\\');
        }
        out.push(byte.value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn splitting() {
        for (s, ifs, want) in [
            (":a::b:", ":", vec!["", "a", "", "b"]),
            ("  a \t b  ", " \t\n", vec!["a", "b"]),
            (" a ", "", vec![" a "]),
        ] {
            let mut f = Field::default();
            f.append(s.as_bytes(), false, true);
            assert_eq!(
                split(f, ifs.as_bytes())
                    .iter()
                    .map(|f| String::from_utf8(f.raw()).unwrap())
                    .collect::<Vec<_>>(),
                want
            );
        }
    }
    #[test]
    fn split_join_idempotence() {
        for n in 0..256u32 {
            let s = (0..8)
                .map(|i| if n & (1 << i) == 0 { 'a' } else { ' ' })
                .collect::<String>();
            let s = s.trim();
            let mut f = Field::default();
            f.append(s.as_bytes(), false, true);
            let joined = split(f, b" ")
                .iter()
                .map(Field::raw)
                .collect::<Vec<_>>()
                .join(b" ".as_slice());
            let mut f = Field::default();
            f.append(&joined, false, true);
            assert_eq!(
                split(f, b" ")
                    .iter()
                    .map(Field::raw)
                    .collect::<Vec<_>>()
                    .join(b" ".as_slice()),
                joined
            );
        }
    }
    #[test]
    fn canonical_split_join_identity() {
        for n in 1..128 {
            let fields = (0..n % 12 + 1)
                .map(|i| vec![b'a' + (i % 26) as u8; i + 1])
                .collect::<Vec<_>>();
            let original = fields.join(b" ".as_slice());
            let mut field = Field::default();
            field.append(&original, false, true);
            assert_eq!(
                split(field, b" ")
                    .iter()
                    .map(Field::raw)
                    .collect::<Vec<_>>()
                    .join(b" ".as_slice()),
                original
            );
        }
    }
    #[test]
    fn arbitrary_word_bytes_terminate() {
        let mut env = ShellEnv::inherited().unwrap();
        env.noglob = true;
        let mut seed = 71u64;
        for len in 0..1000 {
            let mut bytes = Vec::new();
            for _ in 0..len % 64 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                bytes.push((seed >> 32) as u8);
            }
            if let Ok(tokens) = crate::lexer::lex(&bytes) {
                for token in tokens {
                    if let crate::lexer::TokenKind::Word(w) = token.kind {
                        let _ = word(
                            &w,
                            &mut env,
                            &mut |_, _| Ok((Vec::new(), 0)),
                            Mode::Argument,
                        );
                    }
                }
            }
        }
    }
}
