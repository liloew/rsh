//! Byte-oriented C-locale patterns shared by pathname and parameter matching.
#[derive(Clone, Debug)]
pub enum Atom {
    Byte(u8),
    Any,
    Star,
    Class {
        negative: bool,
        bytes: Vec<(u8, bool)>,
    },
}
fn class(name: &[u8], b: u8) -> bool {
    match name {
        b"alpha" => b.is_ascii_alphabetic(),
        b"alnum" => b.is_ascii_alphanumeric(),
        b"digit" => b.is_ascii_digit(),
        b"lower" => b.is_ascii_lowercase(),
        b"upper" => b.is_ascii_uppercase(),
        b"space" => b.is_ascii_whitespace(),
        b"blank" => matches!(b, b' ' | b'\t'),
        b"xdigit" => b.is_ascii_hexdigit(),
        b"cntrl" => b.is_ascii_control(),
        b"print" => (32..127).contains(&b),
        b"graph" => (33..127).contains(&b),
        b"punct" => b.is_ascii_punctuation(),
        _ => false,
    }
}
fn member(bytes: &[(u8, bool)], b: u8) -> bool {
    let mut i = 0;
    while let Some(&(c, _)) = bytes.get(i) {
        if bytes.get(i..i + 2) == Some(&[(b'[', false), (b':', false)])
            && let Some(end) = bytes.get(i + 2..).and_then(|s| {
                s.windows(2)
                    .position(|s| s == [(b':', false), (b']', false)])
            })
        {
            if class(
                &bytes
                    .get(i + 2..i + 2 + end)
                    .unwrap_or_default()
                    .iter()
                    .map(|x| x.0)
                    .collect::<Vec<_>>(),
                b,
            ) {
                return true;
            }
            i += end + 4;
        } else if bytes.get(i + 1) == Some(&(b'-', false))
            && let Some(&(end, _)) = bytes.get(i + 2)
        {
            if c <= b && b <= end {
                return true;
            }
            i += 3;
        } else {
            if c == b {
                return true;
            }
            i += 1;
        }
    }
    false
}
pub fn compile(bytes: &[(u8, bool)]) -> Vec<Atom> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&(b, q)) = bytes.get(i) {
        match (b, q) {
            (b'*', false) => out.push(Atom::Star),
            (b'?', false) => out.push(Atom::Any),
            (b'[', false) => {
                let start = i;
                let mut j = i + 1;
                let negative = matches!(bytes.get(j), Some((b'!' | b'^', false)));
                if negative {
                    j += 1;
                }
                let first = j;
                if bytes.get(j).is_some_and(|x| x.0 == b']') {
                    j += 1;
                }
                while let Some(&(c, quoted)) = bytes.get(j) {
                    if c == b'[' && !quoted && bytes.get(j + 1).is_some_and(|x| x.0 == b':') {
                        j += 2;
                        while j < bytes.len()
                            && !(bytes.get(j).is_some_and(|x| x.0 == b':')
                                && bytes.get(j + 1).is_some_and(|x| x.0 == b']'))
                        {
                            j += 1;
                        }
                        j += 2;
                        continue;
                    }
                    if c == b']' && !quoted {
                        break;
                    }
                    j += 1;
                }
                if j < bytes.len() {
                    out.push(Atom::Class {
                        negative,
                        bytes: bytes.get(first..j).unwrap_or_default().to_vec(),
                    });
                    i = j;
                } else {
                    i = start;
                    out.push(Atom::Byte(b));
                }
            }
            _ => out.push(Atom::Byte(b)),
        }
        i += 1;
    }
    out
}
pub fn matches(pattern: &[Atom], text: &[u8]) -> bool {
    // Iterative dynamic programming bounds stack use even for adversarial patterns.
    let mut prev = vec![false; text.len() + 1];
    if let Some(v) = prev.first_mut() {
        *v = true;
    }
    for atom in pattern {
        let mut next = vec![false; text.len() + 1];
        for i in 0..=text.len() {
            let value = match atom {
                Atom::Star => {
                    prev.get(i).copied().unwrap_or(false)
                        || i > 0 && next.get(i - 1).copied().unwrap_or(false)
                }
                _ => {
                    i > 0
                        && prev.get(i - 1).copied().unwrap_or(false)
                        && text.get(i - 1).is_some_and(|&b| match atom {
                            Atom::Byte(c) => *c == b,
                            Atom::Any => true,
                            Atom::Class { negative, bytes } => member(bytes, b) != *negative,
                            _ => false,
                        })
                }
            };
            if let Some(v) = next.get_mut(i) {
                *v = value;
            }
        }
        prev = next;
    }
    prev.last().copied().unwrap_or(false)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patterns() {
        for (p, t, want) in [
            ("a*?", "abc", true),
            ("[!a-c]", "z", true),
            ("[[:alpha:]]", "7", false),
            ("[[:digit:]]", "7", true),
            ("[", "[", true),
        ] {
            assert_eq!(
                matches(
                    &compile(&p.bytes().map(|b| (b, false)).collect::<Vec<_>>()),
                    t.as_bytes()
                ),
                want
            );
        }
    }
    #[test]
    fn protected_brackets_ranges_and_classes() {
        let p = compile(&[
            (b'[', false),
            (b'a', false),
            (b'-', true),
            (b'z', false),
            (b']', false),
        ]);
        assert!(matches(&p, b"-"));
        assert!(!matches(&p, b"m"));
        for p in [
            b"[[:upper:]]".as_slice(),
            b"[[:alpha:]]",
            b"[[:alnum:]]",
            b"[[:graph:]]",
            b"[[:print:]]",
            b"[[:xdigit:]]",
        ] {
            assert!(matches(
                &compile(&p.iter().map(|&b| (b, false)).collect::<Vec<_>>()),
                b"A"
            ));
        }
        assert!(matches(&compile(&[(b'*', true)]), b"*"));
        assert!(!matches(&compile(&[(b'*', true)]), b"abc"));
        assert!(matches(
            &compile(&b"[^a]".iter().map(|&b| (b, false)).collect::<Vec<_>>()),
            b"z"
        ));
    }
}
