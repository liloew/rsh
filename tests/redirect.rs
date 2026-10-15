mod support;
use std::path::Path;
use support::*;

fn fixture(dir: &TempDir) {
    use std::os::unix::fs::PermissionsExt;
    dir.fixtures();
    std::fs::write(
        dir.0.join("both"),
        b"#!/bin/sh\nprintf 'out\\n'\nprintf 'err\\n' >&2\n",
    )
    .unwrap();
    std::fs::set_permissions(dir.0.join("both"), std::fs::Permissions::from_mode(0o755)).unwrap();
}
fn stderr(bytes: &[u8], dir: &TempDir) -> String {
    let text = dir.normalize(bytes);
    text.lines()
        .map(|line| {
            let low = line.to_lowercase();
            if low.contains("no such file") || low.contains("cannot open missing") {
                "missing\n".to_owned()
            } else if low.contains("bad file descriptor")
                || low.contains("bad fd")
                || low.contains("ambiguous redirect")
            {
                "bad-fd\n".to_owned()
            } else {
                format!("{line}\n")
            }
        })
        .collect()
}
#[test]
fn redirect_goldens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/redirect");
    let mut files: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sh"))
        .collect();
    files.sort();
    for file in files {
        let dir = TempDir::new();
        fixture(&dir);
        let source = std::fs::read_to_string(&file).unwrap();
        let mut command = dir.command(env!("CARGO_BIN_EXE_rsh"));
        command.args(["-c", &source]);
        let (out, _) = capture(command);
        assert_eq!(
            dir.normalize(&out.stdout),
            std::fs::read_to_string(file.with_extension("out")).unwrap(),
            "{}: stdout",
            file.display()
        );
        assert_eq!(
            stderr(&out.stderr, &dir),
            std::fs::read_to_string(file.with_extension("err")).unwrap(),
            "{}: stderr",
            file.display()
        );
        assert_eq!(
            format!("{}\n", out.status.code().unwrap()),
            std::fs::read_to_string(file.with_extension("status")).unwrap(),
            "{}: status",
            file.display()
        );
        let ast = rsh::parser::parse(source.as_bytes()).unwrap();
        assert_eq!(rsh::parser::parse(&ast.to_source_bytes()).unwrap(), ast);
    }
}
#[test]
fn redirect_differential() {
    for shell in ["dash", "bash"] {
        if std::process::Command::new(shell)
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skip: missing {shell}");
            return;
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/diff/redirect");
    for file in std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()) {
        let dir = TempDir::new();
        fixture(&dir);
        let source = std::fs::read_to_string(&file).unwrap();
        let mut command = dir.command(env!("CARGO_BIN_EXE_rsh"));
        command.args(["-c", &source]);
        let (actual, _) = capture(command);
        for shell in ["dash", "bash"] {
            let mut command = dir.command(shell);
            if shell == "bash" {
                command.arg("--posix");
            }
            command.args(["-c", &source]);
            let (expected, _) = capture(command);
            let name = file.file_stem().unwrap().to_str().unwrap();
            if name == "missing-status" || name == "special-error" {
                assert!(actual.stdout.is_empty() && expected.stdout.is_empty());
                assert_eq!(
                    actual.status.code(),
                    Some(if name == "special-error" { 2 } else { 1 })
                );
                assert_eq!(
                    expected.status.code(),
                    Some(if shell == "dash" { 2 } else { 1 })
                );
                assert_eq!(stderr(&actual.stderr, &dir), stderr(&expected.stderr, &dir));
                continue;
            }
            if name == "invalid-dup" {
                // User-required strict fd operands: dash aborts with syntax status 2;
                // bash --posix instead implements its >&filename extension.
                assert_eq!(actual.stdout, b"1\nrestored\n");
                assert_eq!(actual.status.code(), Some(0));
                assert_eq!(stderr(&actual.stderr, &dir), "bad-fd\n");
                if shell == "dash" {
                    assert!(expected.stdout.is_empty());
                    assert_eq!(expected.status.code(), Some(2));
                } else {
                    assert_eq!(expected.stdout, b"0\nrestored\n");
                    assert_eq!(expected.status.code(), Some(0));
                    assert!(expected.stderr.is_empty());
                }
                continue;
            }
            if name == "closed" && shell == "dash" {
                // dash reports redirection failure as 2; the requested rsh status is 1.
                assert_eq!(actual.stdout, b"1\ngood\n");
                assert_eq!(expected.stdout, b"2\ngood\n");
                assert_eq!(actual.status.code(), expected.status.code());
                assert_eq!(stderr(&actual.stderr, &dir), stderr(&expected.stderr, &dir));
                continue;
            }
            assert_eq!(
                dir.normalize(&actual.stdout),
                dir.normalize(&expected.stdout),
                "{}: {shell}: stdout",
                file.display()
            );
            assert_eq!(
                stderr(&actual.stderr, &dir),
                stderr(&expected.stderr, &dir),
                "{}: {shell}: stderr",
                file.display()
            );
            assert_eq!(
                actual.status.code(),
                expected.status.code(),
                "{}: {shell}: status",
                file.display()
            );
        }
    }
}
#[test]
fn large_heredocs_and_nested_worker_sources_do_not_block() {
    let dir = TempDir::new();
    fixture(&dir);
    for command in ["cat", "true", "(cat)", "{ cat; } | cat"] {
        let body = "x".repeat(1_100_000) + "\n";
        let source = format!("{command} <<END\n{body}END\n");
        std::fs::write(dir.0.join("large"), source).unwrap();
        let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
        cmd.arg("large").env("TMPDIR", &dir.0);
        let (out, _) = capture(cmd);
        assert!(
            out.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            out.stdout.len(),
            if command == "true" { 0 } else { body.len() }
        );
    }
    let source = format!("{{ cat <<END\n{}\nEND\n}} | cat\n", "y".repeat(1_100_000));
    std::fs::write(dir.0.join("large"), source).unwrap();
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.arg("large").env("TMPDIR", &dir.0);
    let (out, _) = capture(cmd);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout.len(), 1_100_001);
    assert!(std::fs::read_dir(&dir.0).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("rsh-heredoc-")
    }));
}
#[test]
fn nested_substitution_heredoc_is_syntax_only_until_m5() {
    let source = "echo \"$(cat <<END\n) literal punctuation\nEND\n)\"\n";
    let ast = rsh::parser::parse(source.as_bytes()).unwrap();
    assert_eq!(rsh::parser::parse(ast.to_string().as_bytes()).unwrap(), ast);
    let dir = TempDir::new();
    fixture(&dir);
    for shell in ["dash", "bash"] {
        let mut cmd = dir.command(shell);
        if shell == "bash" {
            cmd.arg("--posix");
        }
        cmd.args(["-n", "-c", source]);
        match cmd.output() {
            Ok(out) => assert!(out.status.success(), "{shell}: {:?}", out.stderr),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => eprintln!("skip missing {shell}"),
            Err(e) => panic!("{e}"),
        }
    }
}

#[test]
fn parent_fd_guard_probe_in_isolated_process() {
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args(["--ignored", "--exact", "parent_fd_probe", "--nocapture"]);
    let (out, _) = capture(cmd);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
#[ignore = "run in isolation to avoid changing concurrent test threads' descriptors"]
fn parent_fd_probe() {
    let dir = TempDir::new();
    fixture(&dir);
    let mut env = rsh::env::ShellEnv::inherited().unwrap();
    env.cwd = dir.0.clone();
    env.noclobber = true;
    let mut shell = rsh::exec::Shell::new(env, env!("CARGO_BIN_EXE_rsh").into());
    std::fs::write(dir.0.join("keep"), b"old").unwrap();
    assert_eq!(shell.run(&rsh::parser::parse(b"pwd >keep").unwrap()), 1);
    assert_eq!(std::fs::read(dir.0.join("keep")).unwrap(), b"old");
    assert_eq!(shell.run(&rsh::parser::parse(b"pwd >|keep").unwrap()), 0);
    let source =
        b"pwd >one; pwd >two 2>error <absent; pwd >three; { exec 20>high; pwd; } >group; pwd >four";
    assert_eq!(shell.run(&rsh::parser::parse(source).unwrap()), 0);
    let expected = dir.0.to_string_lossy().into_owned() + "\n";
    for file in ["keep", "one", "three", "four", "group"] {
        assert_eq!(
            std::fs::read(dir.0.join(file)).unwrap(),
            expected.as_bytes()
        );
    }
    // Closed stdio is restored after a regular builtin, including failed write paths.
    assert_eq!(
        shell.run(&rsh::parser::parse(b"pwd 0<&- 1>&- 2>&-").unwrap()),
        1
    );
    assert_eq!(shell.run(&rsh::parser::parse(b"pwd >five").unwrap()), 0);
    assert_eq!(
        std::fs::read(dir.0.join("five")).unwrap(),
        expected.as_bytes()
    );
}

#[test]
#[cfg(target_os = "linux")]
fn internal_descriptors_do_not_survive_exec_on_linux() {
    let dir = TempDir::new();
    fixture(&dir);
    for body in ["ignored\n".to_owned(), "x".repeat(1_100_000) + "\n"] {
        let source = format!("{{ ls /proc/self/fd <<'E'; }} >fds\n{body}E\ncat fds");
        std::fs::write(dir.0.join("fd-script"), source).unwrap();
        let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
        cmd.arg("fd-script");
        let (out, _) = capture(cmd);
        assert!(out.status.success(), "{:?}", out.stderr);
        let fds: Vec<i32> = String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .map(|s| s.parse().unwrap())
            .collect();
        assert!(fds.iter().all(|fd| *fd <= 3), "leaked descriptors: {fds:?}");
    }
}

#[test]
fn stdin_restored_after_builtin_and_failed_redirection() {
    use std::io::Write;
    use std::process::Stdio;
    let dir = TempDir::new();
    fixture(&dir);
    std::fs::write(dir.0.join("f"), b"file-data\n").unwrap();
    let mut child = dir
        .command(env!("CARGO_BIN_EXE_rsh"))
        .args(["-c", "pwd <f >p 2>e; pwd 2>>e <missing; cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin-data\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"stdin-data\n");
    assert!(out.stderr.is_empty());
}
#[test]
fn closed_stdio_survives_internal_subshell_reexec() {
    let dir = TempDir::new();
    fixture(&dir);
    for source in ["(pwd) 1>&-", "(cat) 0<&-"] {
        let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
        cmd.args(["-c", source]);
        let (out, _) = capture(cmd);
        assert_eq!(out.status.code(), Some(1), "{source}: {:?}", out.stderr);
        assert!(out.stdout.is_empty());
    }
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.args(["-c", "(echo high) 20>f; cat f"]);
    let (out, _) = capture(cmd);
    assert!(out.status.success(), "{:?}", out.stderr);
    assert_eq!(out.stdout, b"high\n");
}
#[test]
fn incomplete_document_reference_difference_is_explicit() {
    let source = "cat <<END\ntext\n";
    assert!(matches!(
        rsh::parser::parse(source.as_bytes()),
        Err(rsh::parser::ParseError::Incomplete(_))
    ));
    let dir = TempDir::new();
    fixture(&dir);
    for shell in ["dash", "bash"] {
        let mut cmd = dir.command(shell);
        if shell == "bash" {
            cmd.arg("--posix");
        }
        cmd.args(["-c", source]);
        match cmd.output() {
            Ok(out) => {
                assert_eq!(out.status.code(), Some(0));
                assert_eq!(out.stdout, b"text\n");
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => eprintln!("skip missing {shell}"),
            Err(e) => panic!("{e}"),
        }
    }
}
#[test]
fn heredoc_byte_data_and_left_to_right_partial_side_effects() {
    let dir = TempDir::new();
    fixture(&dir);
    std::fs::write(dir.0.join("script"), b"cat <<'E'\n\xff\0\xfe\nE\n").unwrap();
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.arg("script");
    let (out, _) = capture(cmd);
    assert!(out.status.success(), "{:?}", out.stderr);
    assert_eq!(out.stdout, b"\xff\0\xfe\n");
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.args(["-c", "echo do-not-run >created 2>&invalid"]);
    let (out, _) = capture(cmd);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(std::fs::read(dir.0.join("created")).unwrap(), b"");
}

#[test]
fn hidden_restore_descriptors_cannot_be_used_by_dynamic_dup_operands() {
    let dir = TempDir::new();
    fixture(&dir);
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.args(["-c", "FD=21; (touch marker) >&$FD"]);
    let (out, _) = capture(cmd);
    assert_eq!(out.status.code(), Some(1));
    assert!(!dir.0.join("marker").exists());
    assert_eq!(stderr(&out.stderr, &dir), "bad-fd\n");
    for command in ["echo must-not-escape", "pwd"] {
        let source = format!("FD=21; {{ {command} >&$FD; printf '%s\\n' \"$?\"; }} >out; cat out");
        let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
        cmd.args(["-c", &source]);
        let (out, _) = capture(cmd);
        assert_eq!(out.stdout, b"1\n", "{source}: {:?}", out.stderr);
        assert_eq!(stderr(&out.stderr, &dir), "bad-fd\n");
    }
}
