mod support;
use support::*;
#[test]
fn execution_matches_dash_and_bash_posix() {
    for shell in ["dash", "bash"] {
        if std::process::Command::new(shell)
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skipping execution differential tests: {shell} is missing");
            return;
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/diff/exec");
    let mut files: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    for file in files {
        let dir = TempDir::new();
        dir.fixtures();
        let name = file.file_stem().unwrap().to_str().unwrap();
        let source = std::fs::read_to_string(&file).unwrap();
        let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
        cmd.args(["-c", &source]);
        let (actual, _) = capture(cmd);
        for shell in ["dash", "bash"] {
            let mut cmd = dir.command(shell);
            if shell == "bash" {
                cmd.arg("--posix");
            }
            cmd.args(["-c", &source]);
            let (expected, _) = capture(cmd);
            if name == "special-prefix" && shell == "bash" {
                // Explicit reference-shell disagreement, pinned by the fixture header.
                assert_eq!(actual.stdout, b"1\n");
                assert_eq!(actual.status.code(), Some(1));
                assert_eq!(expected.stdout, b"1\nA=1\n");
                assert_eq!(expected.status.code(), Some(0));
                assert!(actual.stderr.is_empty() && expected.stderr.is_empty());
                continue;
            }
            assert_eq!(
                normalize_stdout(&dir, name, &actual.stdout),
                normalize_stdout(&dir, name, &expected.stdout),
                "{name}: {shell}: stdout"
            );
            assert_eq!(
                normalize_stderr(&dir, name, &actual.stderr),
                normalize_stderr(&dir, name, &expected.stderr),
                "{name}: {shell}: stderr"
            );
            assert_eq!(
                shell_status(&actual.status),
                shell_status(&expected.status),
                "{name}: {shell}: status"
            );
        }
    }
}

fn shell_status(status: &std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}
