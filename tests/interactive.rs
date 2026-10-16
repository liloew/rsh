mod support;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use support::{TempDir, capture};

#[test]
fn config_aliases_and_errors_are_scoped_to_interactive_startup() {
    let d = TempDir::new();
    d.fixtures();
    let config = d.0.join("config/rsh");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("config.sh"),
        "alias hello='printf CONFIG'; STARTED=yes\necho )\n",
    )
    .unwrap();
    let mut cmd = d.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.env("XDG_CONFIG_HOME", d.0.join("config")).args([
        "-i",
        "-c",
        "hello; printf ':%s\\n' $STARTED",
    ]);
    let (out, _) = capture(cmd);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"CONFIG:yes\n");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("config.sh:2: warning:"), "{err}");
    let mut cmd = d.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.env("XDG_CONFIG_HOME", d.0.join("config"))
        .args(["-c", "printf '%s' ${STARTED-unset}"]);
    assert_eq!(capture(cmd).0.stdout, b"unset");
}

#[test]
fn login_profile_and_vi_startup_options() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::write(d.0.join("home/.profile"), "PROFILE_LOADED=yes\n").unwrap();
    for arg0 in [false, true] {
        let mut cmd = d.command(env!("CARGO_BIN_EXE_rsh"));
        if arg0 {
            cmd.arg0("-rsh");
        } else {
            cmd.arg("-l");
        }
        cmd.args(["--vi", "-c", "printf '%s\\n' $PROFILE_LOADED; set -o"]);
        let (out, _) = capture(cmd);
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.starts_with("yes\n"), "{text}");
        assert!(
            text.lines()
                .any(|s| s.starts_with("vi") && s.ends_with("on")),
            "{text}"
        );
    }
}

#[test]
fn non_tty_i_falls_back_to_byte_script_input_without_configuration() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::create_dir_all(d.0.join("home/.config/rsh")).unwrap();
    std::fs::write(
        d.0.join("home/.config/rsh/config.sh"),
        "echo WRONG-CONFIG\n",
    )
    .unwrap();
    let mut child = d
        .command(env!("CARGO_BIN_EXE_rsh"))
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"printf '%s' '\xff'\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, [255]);
    assert!(out.stderr.is_empty());
}

#[test]
fn nested_startup_syntax_errors_name_the_actual_file_and_line() {
    let d = TempDir::new();
    d.fixtures();
    let config = d.0.join("config/rsh");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.sh"), b"# config\n. ./nested.sh\n").unwrap();
    let mut source = "# sourced file\n".repeat(59);
    source.push_str("if false; then\n  :\n  cat > > word\nfi\n");
    std::fs::write(d.0.join("nested.sh"), source).unwrap();
    let mut cmd = d.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.env("XDG_CONFIG_HOME", d.0.join("config"))
        .args(["-i", "-c", "echo alive"]);
    let output = capture(cmd).0;
    assert!(output.status.success());
    assert_eq!(output.stdout, b"alive\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nested.sh:62: warning: rsh: line 62:"),
        "{stderr}"
    );
    assert!(!stderr.contains("config.sh:62:"), "{stderr}");
}
