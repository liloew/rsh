mod support;
use support::*;

#[test]
fn exec_goldens() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/exec");
    let mut files: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sh"))
        .collect();
    files.sort();
    for file in files {
        let dir = TempDir::new();
        dir.fixtures();
        let source = std::fs::read_to_string(&file).unwrap();
        let name = file.file_stem().unwrap().to_str().unwrap();
        let mut command = dir.command(env!("CARGO_BIN_EXE_rsh"));
        command.args(["-c", &source]);
        let (output, _) = capture(command);
        let stdout = normalize_stdout(&dir, name, &output.stdout);
        let stderr = normalize_stderr(&dir, name, &output.stderr);
        let status = format!("{}\n", output.status.code().unwrap());
        // The fixtures have explicit, independently written expectations.
        assert_eq!(
            stdout,
            std::fs::read_to_string(file.with_extension("out")).unwrap(),
            "{name}: stdout"
        );
        assert_eq!(
            stderr,
            std::fs::read_to_string(file.with_extension("err")).unwrap(),
            "{name}: stderr"
        );
        assert_eq!(
            status,
            std::fs::read_to_string(file.with_extension("status")).unwrap(),
            "{name}: status"
        );
    }
}

#[test]
fn script_and_stdin_modes() {
    use std::io::Write;
    use std::process::Stdio;
    let dir = TempDir::new();
    dir.fixtures();
    std::fs::write(
        dir.0.join("script"),
        b"/usr/bin/printf 'script\\n'; exit 7\n",
    )
    .unwrap();
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.arg("script");
    let (out, _) = capture(cmd);
    assert_eq!(out.stdout, b"script\n");
    assert_eq!(out.status.code(), Some(7));
    for args in [vec![], vec!["-s"]] {
        let mut child = dir
            .command(env!("CARGO_BIN_EXE_rsh"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"/usr/bin/printf 'stdin\\n'; exit 4")
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.stdout, b"stdin\n");
        assert_eq!(out.status.code(), Some(4));
    }
}

#[test]
fn path_lookup_and_shadowing() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new();
    dir.fixtures();
    std::fs::write(dir.0.join("true"), b"#!/bin/sh\nexit 99\n").unwrap();
    std::fs::set_permissions(dir.0.join("true"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.args(["-c", "PATH=; true; /usr/bin/printf '%s\n' \"$?\"; ./true"]);
    let (out, _) = capture(cmd);
    assert_eq!(out.stdout, b"0\n");
    assert_eq!(out.status.code(), Some(99));
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.args(["-c", "PATH=; true | ./true"]);
    let (out, _) = capture(cmd);
    assert_eq!(out.status.code(), Some(99));
    assert!(out.stderr.is_empty());
}

#[test]
fn background_returns_before_child_and_preserves_unexported_variables() {
    let dir = TempDir::new();
    dir.fixtures();
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.args(["-c", "sleep 1 & true"]);
    let (out, elapsed) = capture(cmd);
    assert!(out.status.success());
    assert!(
        elapsed < std::time::Duration::from_millis(700),
        "{elapsed:?}"
    );
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.args(["-c", "A=local; /usr/bin/printenv A || true &"]);
    let (out, _) = capture(cmd);
    assert!(out.stdout.is_empty());
    assert!(out.stderr.is_empty());
}

#[test]
fn reaping_probe_runs_in_its_own_process() {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["--ignored", "--exact", "reaping_probe", "--nocapture"]);
    let (out, _) = capture(command);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
#[ignore = "invoked by the isolated reaping test"]
fn reaping_probe() {
    let dir = TempDir::new();
    dir.fixtures();
    let mut env = rsh::env::ShellEnv::inherited().unwrap();
    env.cwd = dir.0.clone();
    env.variables.clear();
    env.set(b"PATH".to_vec(), b"/usr/bin:/bin".to_vec());
    env.mark_exported(b"PATH".to_vec());
    let mut shell = rsh::exec::Shell::new(env, env!("CARGO_BIN_EXE_rsh").into());
    let tree = rsh::parser::parse(b"sleep 0.1 | true; yes | head -1").unwrap();
    let start = std::time::Instant::now();
    assert_eq!(shell.run(&tree), 0);
    assert!(start.elapsed() >= std::time::Duration::from_millis(90));
    assert!(
        !rsh::sys::has_children().unwrap(),
        "pipeline left a running or zombie child"
    );
    let tree = rsh::parser::parse(b"true & sleep 0.1").unwrap();
    assert_eq!(shell.run(&tree), 0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while shell.background_count() != 0 && std::time::Instant::now() < deadline {
        shell.reap_background();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(shell.background_count(), 0);
    assert!(!rsh::sys::has_children().unwrap());
}

#[test]
fn path_permission_fallback_is_scoped_and_byte_environment_survives() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new();
    dir.fixtures();
    std::fs::create_dir(dir.0.join("first")).unwrap();
    std::fs::create_dir(dir.0.join("second")).unwrap();
    std::fs::write(dir.0.join("first/tool"), b"not executable").unwrap();
    std::fs::set_permissions(
        dir.0.join("first/tool"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    std::fs::write(dir.0.join("second/tool"), b"#!/bin/sh\nexit 23\n").unwrap();
    std::fs::set_permissions(
        dir.0.join("second/tool"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.args([
        "-c",
        "PATH=first:second tool; printf '%s\n' \"$?\"; printf '%s\n' \"$PATH\"",
    ]);
    let (out, _) = capture(cmd);
    assert_eq!(out.stdout, b"23\n/usr/bin:/bin\n");
    assert!(out.stderr.is_empty());
    std::fs::write(
        dir.0.join("bytes-script"),
        b"X='\xff'; export X; printenv X",
    )
    .unwrap();
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.arg("bytes-script");
    let (out, _) = capture(cmd);
    assert_eq!(out.stdout, b"\xff\n");
    assert!(out.status.success());
    std::fs::write(dir.0.join("nul-script"), b"/usr/bin/printf '\0'").unwrap();
    let mut cmd = dir.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.arg("nul-script");
    let (out, _) = capture(cmd);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.windows(3).any(|s| s == b"NUL"));
}
