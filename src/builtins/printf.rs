//! Byte-oriented printf; allocations/width are bounded independently of numeric input.
use super::Outcome;
const LIMIT: usize = 1_048_576;
pub fn escapes(s: &[u8], b_format: bool) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&c) = s.get(i) {
        i += 1;
        if c != b'\\' {
            out.push(c);
            continue;
        }
        let Some(&c) = s.get(i) else {
            out.push(b'\\');
            break;
        };
        i += 1;
        match c {
            b'c' => return (out, true),
            b'a' => out.push(7),
            b'b' => out.push(8),
            b'f' => out.push(12),
            b'n' => out.push(10),
            b'r' => out.push(13),
            b't' => out.push(9),
            b'v' => out.push(11),
            b'\\' => out.push(92),
            b'0'..=b'7' => {
                let mut n = if b_format && c == b'0' { 0 } else { c - b'0' };
                let max = if b_format && c == b'0' { 3 } else { 2 };
                for _ in 0..max {
                    if let Some(&c @ b'0'..=b'7') = s.get(i) {
                        n = n.wrapping_mul(8).wrapping_add(c - b'0');
                        i += 1;
                    } else {
                        break;
                    }
                }
                out.push(n);
            }
            _ => {
                out.push(b'\\');
                out.push(c);
            }
        }
    }
    (out, false)
}
fn numeric(s: &[u8]) -> (u64, bool) {
    let s = s.trim_ascii();
    if s.is_empty() {
        return (0, true);
    }
    if matches!(s.first(), Some(b'\'' | b'"')) {
        return (u64::from(s.get(1).copied().unwrap_or(0)), s.len() > 1);
    }
    let (negative, s) = if let Some(v) = s.strip_prefix(b"-") {
        (true, v)
    } else {
        (false, s.strip_prefix(b"+").unwrap_or(s))
    };
    let (radix, s) = if let Some(s) = s.strip_prefix(b"0x").or_else(|| s.strip_prefix(b"0X")) {
        (16, s)
    } else if s.len() > 1 && s.first() == Some(&b'0') {
        (8, s)
    } else {
        (10, s)
    };
    let mut n = 0u64;
    let mut count = 0;
    let mut good = true;
    for &b in s {
        let Some(d) = (b as char).to_digit(radix) else {
            good = false;
            break;
        };
        let (v, overflow) = n.overflowing_mul(u64::from(radix));
        let (v, overflow2) = v.overflowing_add(u64::from(d));
        n = v;
        good &= !overflow && !overflow2;
        count += 1;
    }
    if count == 0 {
        good = false;
    }
    (if negative { n.wrapping_neg() } else { n }, good)
}
fn arg<'a>(args: &'a [Vec<u8>], index: &mut usize) -> &'a [u8] {
    let v = args.get(*index).map(Vec::as_slice).unwrap_or_default();
    if *index < args.len() {
        *index += 1;
    }
    v
}
fn width_argument(s: &[u8], out: &mut Outcome) -> i64 {
    let (n, ok) = numeric(s);
    if !ok {
        out.status = 1;
        out.stderr
            .extend(b"rsh: printf: invalid width or precision\n");
    }
    n as i64
}
pub fn run(args: &[Vec<u8>]) -> Outcome {
    let args = if args.first().is_some_and(|a| a == b"--") {
        args.get(1..).unwrap_or_default()
    } else {
        args
    };
    let Some(format) = args.first() else {
        return Outcome::error(b"printf", b"format required", 2);
    };
    let args = args.get(1..).unwrap_or_default();
    let mut outcome = Outcome::default();
    let mut index = 0;
    loop {
        let before = index;
        let mut i = 0;
        while let Some(&b) = format.get(i) {
            if b != b'%' {
                let end = format
                    .get(i..)
                    .and_then(|s| s.iter().position(|b| *b == b'%'))
                    .map(|n| i + n)
                    .unwrap_or(format.len());
                let (bytes, stop) = escapes(format.get(i..end).unwrap_or_default(), false);
                outcome.stdout.extend(bytes);
                if stop {
                    return outcome;
                }
                i = end;
                continue;
            }
            i += 1;
            if format.get(i) == Some(&b'%') {
                outcome.stdout.push(b'%');
                i += 1;
                continue;
            }
            let mut flags = Vec::new();
            while format.get(i).is_some_and(|b| b"-+ #0".contains(b)) {
                if let Some(b) = format.get(i) {
                    flags.push(*b);
                }
                i += 1;
            }
            let mut width = 0usize;
            if format.get(i) == Some(&b'*') {
                let a = arg(args, &mut index);
                let n = width_argument(a, &mut outcome);
                if n < 0 {
                    flags.push(b'-');
                }
                width = n.unsigned_abs().min(LIMIT as u64) as usize;
                i += 1;
            } else {
                while let Some(&b @ b'0'..=b'9') = format.get(i) {
                    width = width
                        .saturating_mul(10)
                        .saturating_add(usize::from(b - b'0'))
                        .min(LIMIT);
                    i += 1;
                }
            }
            let mut precision = None;
            if format.get(i) == Some(&b'.') {
                i += 1;
                let mut p = 0usize;
                if format.get(i) == Some(&b'*') {
                    let a = arg(args, &mut index);
                    let n = width_argument(a, &mut outcome);
                    if n >= 0 {
                        precision = Some((n as usize).min(LIMIT));
                    }
                    i += 1;
                } else {
                    while let Some(&b @ b'0'..=b'9') = format.get(i) {
                        p = p
                            .saturating_mul(10)
                            .saturating_add(usize::from(b - b'0'))
                            .min(LIMIT);
                        i += 1;
                    }
                    precision = Some(p);
                }
            }
            let Some(&conversion) = format.get(i) else {
                return Outcome::error(b"printf", b"incomplete conversion", 1);
            };
            i += 1;
            let a = arg(args, &mut index);
            let mut prefix = Vec::new();
            let mut stop = false;
            let mut value = match conversion {
                b's' => a
                    .get(..precision.unwrap_or(a.len()).min(a.len()))
                    .unwrap_or_default()
                    .to_vec(),
                b'c' => vec![a.first().copied().unwrap_or(0)],
                b'b' => {
                    let (mut v, s) = escapes(a, true);
                    stop = s;
                    if let Some(p) = precision {
                        v.truncate(p);
                    }
                    v
                }
                b'd' | b'i' | b'o' | b'u' | b'x' | b'X' => {
                    let (n, mut ok) = numeric(a);
                    let signed = matches!(conversion, b'd' | b'i');
                    let input_negative = a.trim_ascii().first() == Some(&b'-');
                    let input_magnitude = if input_negative { n.wrapping_neg() } else { n };
                    if signed && input_magnitude > (i64::MAX as u64) + u64::from(input_negative) {
                        ok = false;
                    }
                    if !ok {
                        outcome.status = 1;
                        outcome
                            .stderr
                            .extend_from_slice(b"rsh: printf: invalid numeric value: ");
                        outcome.stderr.extend(a);
                        outcome.stderr.push(b'\n');
                    }
                    let negative = signed && (n as i64) < 0;
                    let magnitude = if negative { n.wrapping_neg() } else { n };
                    if negative {
                        prefix.push(b'-');
                    } else if signed && flags.contains(&b'+') {
                        prefix.push(b'+');
                    } else if signed && flags.contains(&b' ') {
                        prefix.push(b' ');
                    }
                    let text = match conversion {
                        b'o' => format!("{magnitude:o}"),
                        b'x' => format!("{magnitude:x}"),
                        b'X' => format!("{magnitude:X}"),
                        _ => magnitude.to_string(),
                    };
                    let mut v = if precision == Some(0) && n == 0 {
                        Vec::new()
                    } else {
                        text.into_bytes()
                    };
                    let p = precision.unwrap_or(0);
                    if v.len() < p {
                        let mut zeros = vec![b'0'; p - v.len()];
                        zeros.extend(v);
                        v = zeros;
                    }
                    if flags.contains(&b'#') {
                        match conversion {
                            b'o' if !v.starts_with(b"0") => prefix.push(b'0'),
                            b'x' if n != 0 => prefix.extend(b"0x"),
                            b'X' if n != 0 => prefix.extend(b"0X"),
                            _ => {}
                        }
                    }
                    v
                }
                _ => return Outcome::error(b"printf", b"unsupported conversion", 1),
            };
            let padding = width.saturating_sub(prefix.len() + value.len());
            let numeric = matches!(conversion, b'd' | b'i' | b'o' | b'u' | b'x' | b'X');
            if flags.contains(&b'-') {
                outcome.stdout.extend(prefix);
                outcome.stdout.append(&mut value);
                outcome.stdout.extend(std::iter::repeat_n(b' ', padding));
            } else if numeric && flags.contains(&b'0') && precision.is_none() {
                outcome.stdout.extend(prefix);
                outcome.stdout.extend(std::iter::repeat_n(b'0', padding));
                outcome.stdout.append(&mut value);
            } else {
                outcome.stdout.extend(std::iter::repeat_n(b' ', padding));
                outcome.stdout.extend(prefix);
                outcome.stdout.append(&mut value);
            }
            if stop {
                return outcome;
            }
        }
        if index >= args.len() || before == index {
            break;
        }
    }
    outcome
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formatting() {
        let a = [
            b"%#08x %.*s:%d\\n".to_vec(),
            b"31".to_vec(),
            b"2".to_vec(),
            b"abc".to_vec(),
            b"-3".to_vec(),
        ];
        assert_eq!(run(&a).stdout, b"0x00001f ab:-3\n");
        assert_eq!(
            run(&[
                b"%s:%s;".to_vec(),
                b"a".to_vec(),
                b"b".to_vec(),
                b"c".to_vec()
            ])
            .stdout,
            b"a:b;c:;"
        );
    }
}
