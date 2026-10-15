use std::fmt::Write;
use std::path::Path;

fn dump(source: &[u8]) -> String {
    let mut output = String::new();
    match rsh::lexer::lex(source) {
        Ok(tokens) => {
            for token in tokens {
                writeln!(
                    output,
                    "{}:{}-{}:{} {:?}",
                    token.span.start.line,
                    token.span.start.column,
                    token.span.end.line,
                    token.span.end.column,
                    token.kind
                )
                .unwrap();
            }
        }
        Err(error) => writeln!(output, "{error:?}").unwrap(),
    }
    output
}

#[test]
fn lexer_goldens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/lexer");
    let mut sources: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sh"))
        .collect();
    sources.sort();
    assert!(!sources.is_empty());
    for source in sources {
        let actual = dump(&std::fs::read(&source).unwrap());
        let expected = source.with_extension("tokens");
        if std::env::var_os("UPDATE_GOLDEN").as_deref() == Some(std::ffi::OsStr::new("1")) {
            std::fs::write(&expected, &actual).unwrap();
        }
        assert_eq!(
            actual,
            std::fs::read_to_string(expected).unwrap(),
            "{}",
            source.display()
        );
    }
}
