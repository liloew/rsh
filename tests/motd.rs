mod support;
use std::os::unix::fs::PermissionsExt;
use support::*;
fn run(dir: &TempDir, source: &str) -> std::process::Output {
    let mut command = dir.command(env!("CARGO_BIN_EXE_rsh"));
    command
        .env("XDG_CONFIG_HOME", dir.0.join("config"))
        .args(["-c", source]);
    capture(command).0
}
fn setup() -> TempDir {
    let dir = TempDir::new();
    dir.fixtures();
    std::fs::create_dir_all(dir.0.join("config/rsh")).unwrap();
    dir
}
#[test]
fn user_file_manual_dispatch_and_plain_output() {
    let d = setup();
    std::fs::write(
        d.0.join("config/rsh/motd"),
        b"hello\x1b[31m red\x1b[0m\x1b]0;bad\x07\n",
    )
    .unwrap();
    let o = run(
        &d,
        "type motd; command -v motd; motd; motd | cat; motd() { echo custom; }; motd",
    );
    assert!(o.status.success(), "{o:?}");
    assert_eq!(
        o.stdout,
        b"motd is a shell builtin\nmotd\nhello red\nhello red\ncustom\n"
    );
    assert!(o.stderr.is_empty());
    let o = run(&d, "RSH_NO_MOTD=1 motd");
    assert_eq!(o.stdout, b"hello red\n");
}
#[test]
fn absent_files_and_home_fallback_and_relative_config() {
    let d = setup();
    let o = run(&d, "motd");
    assert!(o.status.success());
    assert!(o.stdout.is_empty());
    std::fs::create_dir_all(d.0.join("home/.config/rsh")).unwrap();
    std::fs::write(d.0.join("home/.config/rsh/motd"), b"home\n").unwrap();
    let o = run(&d, "XDG_CONFIG_HOME= motd; XDG_CONFIG_HOME=config motd");
    assert_eq!(o.stdout, b"home\n");
    assert!(o.status.success());
}
#[test]
fn oversized_file_is_bounded_and_special_files_never_block() {
    let d = setup();
    let path = d.0.join("config/rsh/motd");
    std::fs::write(&path, vec![b'x'; 65536]).unwrap();
    let o = run(&d, "motd");
    assert_eq!(o.stdout.len(), 65536);
    assert_eq!(o.status.code(), Some(0));
    std::fs::write(&path, vec![b'x'; 1024 * 1024]).unwrap();
    let o = run(&d, "motd");
    assert!(o.status.success());
    assert_eq!(o.stdout.len(), 65537);
    assert_eq!(o.stdout.last(), Some(&b'\n'));
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let o = run(&d, "motd");
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stderr.starts_with(b"rsh: motd: "));
    std::fs::remove_dir(&path).unwrap();
    let mut command = d.command("mkfifo");
    command.arg(&path);
    assert!(capture(command).0.status.success());
    let o = run(&d, "motd");
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stderr.starts_with(b"rsh: motd: "));
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"private\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o0)).unwrap();
    if std::process::Command::new("id")
        .arg("-u")
        .output()
        .unwrap()
        .stdout
        != b"0\n"
    {
        let o = run(&d, "motd");
        assert_eq!(o.status.code(), Some(1));
        assert!(o.stderr.starts_with(b"rsh: motd: "));
    }
}
#[test]
fn symlinks_world_writable_and_release_override_rejection() {
    let d = setup();
    let target = d.0.join("text");
    std::fs::write(&target, b"safe\x07\n").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o666)).unwrap();
    std::os::unix::fs::symlink(&target, d.0.join("config/rsh/motd")).unwrap();
    let o = run(&d, "motd");
    assert_eq!(o.stdout, b"safe\n");
    std::fs::remove_file(d.0.join("config/rsh/motd")).unwrap();
    let mut command = d.command(env!("CARGO_BIN_EXE_rsh"));
    command
        .env("XDG_CONFIG_HOME", d.0.join("config"))
        .env("RSH_TEST_SYSTEM_MOTD", target)
        .args(["-c", "motd"]);
    assert!(capture(command).0.stdout.is_empty());
}
#[test]
fn automatic_message_is_absent_in_command_script_and_piped_modes() {
    use std::io::Write;
    use std::process::Stdio;
    let d = setup();
    std::fs::write(d.0.join("config/rsh/motd"), b"UNEXPECTED\n").unwrap();
    std::fs::write(d.0.join("script"), b"printf 'ok\\n'\n").unwrap();
    for args in [
        vec!["-i", "-c", "echo ok"],
        vec!["-i", "script"],
        vec!["--no-motd", "-c", "motd"],
    ] {
        let mut command = d.command(env!("CARGO_BIN_EXE_rsh"));
        command
            .env("XDG_CONFIG_HOME", d.0.join("config"))
            .args(&args);
        let o = capture(command).0;
        assert!(o.status.success(), "{o:?}");
        assert_eq!(
            o.stdout,
            if args.contains(&"motd") {
                b"UNEXPECTED\n".as_slice()
            } else {
                b"ok\n"
            }
        );
    }
    let mut command = d.command(env!("CARGO_BIN_EXE_rsh"));
    let mut child = command
        .env("XDG_CONFIG_HOME", d.0.join("config"))
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"echo ok\n").unwrap();
    let o = child.wait_with_output().unwrap();
    assert_eq!(o.stdout, b"ok\n");
    assert!(o.status.success());
}

#[test]
fn socket_and_device_targets_are_rejected_without_reading() {
    let d = setup();
    let path = d.0.join("config/rsh/motd");
    let _socket = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let o = run(&d, "motd");
    assert_eq!(o.status.code(), Some(1));
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink("/dev/null", &path).unwrap();
    let o = run(&d, "motd");
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("not a regular file"));
}
