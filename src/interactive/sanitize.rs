//! Terminal-safe MOTD text. No IO; bounded callers may pass arbitrary bytes.
pub fn sanitize(input: &[u8], colors: bool) -> Vec<u8> {
    let mut text = String::new();
    let mut rest = input;
    while !rest.is_empty() {
        match std::str::from_utf8(rest) {
            Ok(valid) => {
                text.push_str(valid);
                break;
            }
            Err(error) => {
                let (valid, invalid) = rest.split_at(error.valid_up_to());
                if let Ok(valid) = std::str::from_utf8(valid) {
                    text.push_str(valid);
                }
                let count = error.error_len().unwrap_or(invalid.len());
                if let Some(&byte) = invalid.first() {
                    if (0x80..=0x9f).contains(&byte) {
                        text.push(char::from(byte));
                    } else {
                        text.push('\u{fffd}');
                    }
                }
                rest = invalid.get(count..).unwrap_or_default();
            }
        }
    }
    let mut chars = text.chars().peekable();
    let mut output = String::new();
    while let Some(c) = chars.next() {
        let control = if c == '\x1b' { chars.next() } else { Some(c) };
        match control {
            Some('[') if c == '\x1b' => csi(&mut chars, colors, &mut output),
            Some('\u{9b}') => csi(&mut chars, colors, &mut output),
            Some(']' | 'P' | 'X' | '^' | '_') if c == '\x1b' => {
                string_control(&mut chars, control == Some(']'))
            }
            Some('N' | 'O') if c == '\x1b' => {
                chars.next();
            }
            Some('\u{90}' | '\u{98}' | '\u{9d}' | '\u{9e}' | '\u{9f}') => {
                string_control(&mut chars, control == Some('\u{9d}'))
            }
            _ if c == '\x1b' => {
                // ESC intermediates followed by a final byte (e.g. charset selection).
                if control.is_some_and(|x| (' '..='/').contains(&x)) {
                    while chars.next().is_some_and(|x| (' '..='/').contains(&x)) {}
                }
            }
            _ if c == '\n' || c == '\t' || !c.is_control() => output.push(c),
            _ => {}
        }
    }
    output.into_bytes()
}
fn csi(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, colors: bool, output: &mut String) {
    let mut parameters = String::new();
    let mut valid = true;
    for c in chars.by_ref() {
        if ('@'..='~').contains(&c) {
            if colors && valid && c == 'm' {
                output.push_str("\x1b[");
                output.push_str(&parameters);
                output.push('m');
            }
            return;
        }
        valid &= c.is_ascii_digit() || c == ';' || c == ':';
        parameters.push(c);
    }
}
fn string_control(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, osc: bool) {
    while let Some(c) = chars.next() {
        if (osc && c == '\x07') || c == '\u{9c}' {
            break;
        }
        if c == '\x1b' && chars.peek() == Some(&'\\') {
            chars.next();
            break;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_colors_and_plain_mode() {
        assert_eq!(sanitize(b"hello\n\tworld", true), b"hello\n\tworld");
        assert_eq!(sanitize(b"\x1b[31mred\x1b[0m", true), b"\x1b[31mred\x1b[0m");
        assert_eq!(sanitize(b"\x1b[31mred\x1b[0m", false), b"red");
    }
    #[test]
    fn controls_and_strings_are_discarded() {
        assert_eq!(
            sanitize(b"a\x1b]0;title\x07b\x1b[2J\x1b[1;1Hc\x1bPdata\x1b\\d", true),
            b"abcd"
        );
        assert_eq!(sanitize(b"a\r\x08\0\x07\x7f\x85b\x1b", true), b"ab");
        assert_eq!(sanitize(b"a\x9dtitle\x9cb\xc2\x9bc", true), b"ab");
        assert_eq!(sanitize(b"a\x1b[?25mb", true), b"ab");
        assert_eq!(sanitize(b"a\x1bPdata\x07more\x1b\\b\x1bOA", true), b"ab");
    }
    #[test]
    fn invalid_utf8_and_truncated_sequences() {
        assert_eq!(sanitize(b"a\xffb\xe4\xb8", true), "a�b�".as_bytes());
        let mut bytes = vec![b'x'; 65533];
        bytes.extend(b"\x1b[3");
        assert_eq!(sanitize(&bytes, true), vec![b'x'; 65533]);
        assert_eq!(sanitize(b"hello\x1b]unfinished", true), b"hello");
        assert_eq!(sanitize(b"\xe4\xb8\xad\xc2\x85z", true), "中z".as_bytes());
    }
}
