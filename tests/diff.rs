mod support;
use std::process::{Command, Output};
use support::TempDir;

fn run(shell: &str, posix: bool, source: &str) -> std::io::Result<Output> {
    // Even the legacy smoke cases must not source a developer's startup files.
    let fixture = TempDir::new();
    fixture.fixtures();
    let mut command = fixture.command(shell);
    if posix {
        command.arg("--posix");
    }
    command.args(["-c", source]).output()
}

#[test]
fn m0_external_commands_match_reference_shells() {
    let references = [("dash", false), ("bash", true)];
    for (shell, posix) in references {
        match run(shell, posix, "") {
            Ok(output) => assert!(output.status.success()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("skipping differential tests: {shell} is not installed");
                return;
            }
            Err(error) => panic!("cannot run {shell}: {error}"),
        }
    }
    for source in include_str!("diff/m0.sh")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let actual = run(env!("CARGO_BIN_EXE_rsh"), false, source).unwrap();
        for (shell, posix) in references {
            let expected = run(shell, posix, source).unwrap();
            assert_eq!(actual.stdout, expected.stdout, "stdout: {shell}: {source}");
            assert_eq!(actual.stderr, expected.stderr, "stderr: {shell}: {source}");
            assert_eq!(
                actual.status.code(),
                expected.status.code(),
                "status: {shell}: {source}"
            );
        }
    }
}

/// M2 compares syntax acceptance only: --dump-ast and shell -n never execute.
#[test]
fn parser_syntax_matches_reference_shells() {
    let references = [("dash", false), ("bash", true)];
    for (shell, posix) in references {
        if let Err(error) = run(shell, posix, "") {
            if error.kind() == std::io::ErrorKind::NotFound {
                eprintln!("skipping parser differential tests: {shell} is not installed");
                return;
            }
            panic!("cannot run {shell}: {error}");
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut cases: Vec<(Vec<u8>, bool)> = Vec::new();
    for (source, accepted) in [
        (include_str!("diff/parser/valid.sh"), true),
        (include_str!("diff/parser/invalid.sh"), false),
    ] {
        cases.extend(
            source
                .lines()
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(|line| (line.as_bytes().to_vec(), accepted)),
        );
    }
    for entry in std::fs::read_dir(root.join("tests/golden/parser")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "sh") {
            cases.push((std::fs::read(path).unwrap(), true));
        }
    }
    for (source, accepted) in cases {
        let text = std::str::from_utf8(&source).unwrap();
        let actual = Command::new(env!("CARGO_BIN_EXE_rsh"))
            .args(["--dump-ast", "-c", text])
            .env_clear()
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert_eq!(
            actual.status.success(),
            accepted,
            "rsh: {text}: {:?}",
            actual.stderr
        );
        for (shell, posix) in references {
            let mut reference = Command::new(shell);
            if posix {
                reference.arg("--posix");
            }
            let expected = reference
                .args(["-n", "-c", text])
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("LC_ALL", "C")
                .output()
                .unwrap();
            // Bash 3.2 rejects repeated pipeline negation, while Bash 5.3
            // accepts it even in POSIX mode. Both are reference alternatives;
            // the rsh assertion above still requires SPEC 4.1's single '!'.
            if shell == "bash" && text == "! ! a" {
                assert!(
                    matches!(expected.status.code(), Some(0 | 2)),
                    "bash: {text}: {:?}",
                    expected
                );
                assert!(expected.stdout.is_empty());
                continue;
            }
            // Explicitly documented reference-shell disagreements; rsh follows grammar.
            let reference_accepts = match (shell, text) {
                ("bash", "for 1; do :; done" | "for 'i'; do :; done") => true,
                ("dash", "case x in |) ;; esac") => true,
                _ => accepted,
            };
            assert_eq!(
                expected.status.success(),
                reference_accepts,
                "{shell}: {text}: {:?}",
                expected.stderr
            );
            assert!(expected.stdout.is_empty());
        }
    }
}

#[test]
fn for_newline_semicolon_uses_posix_grammar_when_shells_disagree() {
    // POSIX 2.10.2: NAME sequential_sep do_group has one separator, not newline + ';'.
    // dash accepts this extension; bash --posix rejects it. rsh follows POSIX.
    let source = b"for i\n; do :; done";
    assert!(matches!(
        rsh::parser::parse(source),
        Err(rsh::parser::ParseError::Syntax { .. })
    ));
}
