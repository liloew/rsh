use std::path::Path;
use std::process::Command;

#[test]
fn parser_goldens_and_cli_dump() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/parser");
    let mut sources: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sh"))
        .collect();
    sources.sort();
    assert!(!sources.is_empty());
    for source in sources {
        let bytes = std::fs::read(&source).unwrap();
        let tree =
            rsh::parser::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", source.display()));
        let actual = format!("{tree:#?}\n");
        let expected = source.with_extension("ast");
        if std::env::var_os("UPDATE_GOLDEN").as_deref() == Some(std::ffi::OsStr::new("1")) {
            std::fs::write(&expected, &actual).unwrap();
        }
        assert_eq!(
            actual,
            std::fs::read_to_string(expected).unwrap(),
            "{}",
            source.display()
        );
        let cli = Command::new(env!("CARGO_BIN_EXE_rsh"))
            .args(["--dump-ast", "-c"])
            .arg(std::str::from_utf8(&bytes).unwrap())
            .env_clear()
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(
            cli.status.success(),
            "{}: {:?}",
            source.display(),
            cli.stderr
        );
        assert_eq!(cli.stdout, actual.as_bytes());
        assert!(cli.stderr.is_empty());
        assert_eq!(
            rsh::parser::parse(tree.to_string().as_bytes()).unwrap(),
            tree
        );
    }
}

#[test]
fn dump_reports_errors_without_executing() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsh"))
        .args(["--dump-ast", "-c", "\n ;"])
        .env_clear()
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"rsh: line 2: syntax error near unexpected token ';'\n"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rsh"))
        .args(["--dump-ast", "-c", "if a; then"])
        .env_clear()
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.starts_with(b"rsh: parser: incomplete input:"));
}
