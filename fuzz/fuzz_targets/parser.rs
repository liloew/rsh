#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    // Heredoc corpus seeds exercise queued delimiters, nested substitutions,
    // binary bodies, <<- tab stripping, incomplete EOF, and source round-trips.
    if let Ok(tree) = rsh::parser::parse(input) {
        let canonical = tree.to_source_bytes();
        assert_eq!(rsh::parser::parse(&canonical).as_ref(), Ok(&tree));
        if std::str::from_utf8(&canonical).is_ok() {
            assert_eq!(
                rsh::parser::parse(tree.to_string().as_bytes()).as_ref(),
                Ok(&tree)
            );
        }
    }
});
