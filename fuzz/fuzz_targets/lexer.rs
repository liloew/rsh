#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    if let Ok(tokens) = rsh::lexer::lex(input) {
        let mut previous_end = 0;
        for token in tokens {
            assert!(token.span.start.offset >= previous_end);
            assert!(token.span.end.offset > token.span.start.offset);
            assert!(token.span.end.offset <= input.len());
            previous_end = token.span.end.offset;
        }
    }
});
