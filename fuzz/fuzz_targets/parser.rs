#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    // M6 corpus seeds include function headers/bodies/redirections, recursive calls,
    // nested loops, case patterns, missing separators and incomplete definitions.
    // Arbitrary input is parsed and serialized only, never executed.
    // Heredoc corpus seeds exercise queued delimiters, nested substitutions,
    // binary bodies, <<- tab stripping, incomplete EOF, and source round-trips.
    if let Ok(tree) = rsh::parser::parse(input) {
        for item in &tree.0 {
            for pipe in
                std::iter::once(&item.and_or.first).chain(item.and_or.rest.iter().map(|(_, p)| p))
            {
                for command in &pipe.commands {
                    if let rsh::ast::Command::FunctionDef { name, .. } = command {
                        assert!(rsh::env::valid_name(name));
                        assert!(!rsh::builtins::is_special_name(name));
                    }
                }
            }
        }
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
