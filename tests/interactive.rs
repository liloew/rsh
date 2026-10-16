mod support;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use support::{TempDir, capture};

#[test]
fn config_aliases_and_errors_apply_to_all_top_level_modes() {
    let d = TempDir::new();
    d.fixtures();
    let config = d.0.join("config/rsh");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("config.rsh"),
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
    assert!(err.contains("config.rsh:2: warning:"), "{err}");
    let mut cmd = d.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.env("XDG_CONFIG_HOME", d.0.join("config"))
        .args(["-c", "printf '%s' ${STARTED-unset}"]);
    assert_eq!(capture(cmd).0.stdout, b"yes");
}

#[test]
fn login_modes_ignore_legacy_profiles_and_preserve_vi_options() {
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
        cmd.args([
            "--vi",
            "-c",
            "printf '%s\\n' ${PROFILE_LOADED-unset}; set -o",
        ]);
        let (out, _) = capture(cmd);
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.starts_with("unset\n"), "{text}");
        assert!(
            text.lines()
                .any(|s| s.starts_with("vi") && s.ends_with("on")),
            "{text}"
        );
    }
}

#[test]
fn non_tty_i_loads_configuration_and_uses_byte_script_input() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::create_dir_all(d.0.join("home/.config/rsh")).unwrap();
    std::fs::write(
        d.0.join("home/.config/rsh/config.rsh"),
        "PIPED_CONFIG=yes\n",
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
        .write_all(b"printf '%s:' \"$PIPED_CONFIG\"; printf '%s' '\xff'\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"yes:\xff");
    assert!(out.stderr.is_empty());
}

#[test]
fn nested_startup_syntax_errors_name_the_actual_file_and_line() {
    let d = TempDir::new();
    d.fixtures();
    let config = d.0.join("config/rsh");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.rsh"), b"# config\n. ./nested.sh\n").unwrap();
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
    assert!(!stderr.contains("config.rsh:62:"), "{stderr}");
}

#[test]
fn recursive_config_loading_is_ordered_in_all_top_level_modes() {
    let d = TempDir::new();
    d.fixtures();
    let root = d.0.join("config/rsh");
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::write(root.join("00.rsh"), b"ORDER=A; cd child\n").unwrap();
    std::fs::write(root.join("nested/10.rsh"), b"ORDER=${ORDER}B\n").unwrap();
    std::fs::write(root.join("z-rsh"), b"ORDER=${ORDER}Z\n").unwrap();
    std::fs::write(root.join("config.sh"), b"echo WRONG-CONFIG\n").unwrap();
    std::fs::write(root.join("motd"), b"not shell syntax: <<<\n").unwrap();
    std::os::unix::fs::symlink(&root, root.join("nested/loop")).unwrap();
    for file in [".profile", ".bash_profile", ".bashrc"] {
        std::fs::write(
            d.0.join("home").join(file),
            b"PROFILE_LOADED=yes; echo WRONG-PROFILE\n",
        )
        .unwrap();
    }
    let body = "printf '%s:%s\\n' \"$ORDER\" \"${PROFILE_LOADED-unset}\"";
    std::fs::write(d.0.join("main-script"), body).unwrap();
    for args in [
        vec!["-c", body],
        vec!["-i", "-c", body],
        vec!["-l", "-c", body],
        vec!["-i", "-l", "-c", body],
        vec!["main-script"],
    ] {
        let mut command = d.command(env!("CARGO_BIN_EXE_rsh"));
        command
            .env("XDG_CONFIG_HOME", d.0.join("config"))
            .env("BASH_ENV", d.0.join("home/.bashrc"))
            .env("ENV", d.0.join("home/.profile"))
            .args(args);
        let out = capture(command).0;
        assert!(out.status.success(), "{out:?}");
        assert_eq!(out.stdout, b"ABZ:unset\n");
        assert!(
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .all(|line| line.starts_with("rsh: job control:")),
            "{out:?}"
        );
    }
}
#[test]
fn workers_inherit_config_without_reloading_and_errors_do_not_stop_later_files() {
    let d = TempDir::new();
    d.fixtures();
    let root = d.0.join("home/.config/rsh");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("00-load.rsh"),
        b"LOAD_COUNT=$((LOAD_COUNT+1)); printf 'loaded\\n' >> \"$CONFIG_LOG\"\n",
    )
    .unwrap();
    std::fs::write(root.join("10-error.rsh"), b"echo )\n").unwrap();
    std::fs::write(root.join("20-good.rsh"), b"GOOD=yes\n").unwrap();
    let mut command = d.command(env!("CARGO_BIN_EXE_rsh"));
    command.env("CONFIG_LOG",d.0.join("loads")).args(["-c","echo $GOOD:$LOAD_COUNT; (echo $LOAD_COUNT); echo $(echo $LOAD_COUNT); echo $LOAD_COUNT | cat"]);
    let out = capture(command).0;
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, b"yes:1\n1\n1\n1\n");
    assert_eq!(std::fs::read(d.0.join("loads")).unwrap(), b"loaded\n");
    assert!(String::from_utf8_lossy(&out.stderr).contains("10-error.rsh:1:"));
}
#[test]
fn discovery_handles_non_utf8_file_names_and_symlinks_without_cycles() {
    use std::os::unix::ffi::OsStrExt;
    let d = TempDir::new();
    let root = d.0.join("config");
    std::fs::create_dir(&root).unwrap();
    // Linux permits raw byte names; macOS filesystems reject invalid UTF-8.
    let name = if cfg!(target_os = "linux") {
        b"\xff.rsh".as_slice()
    } else {
        b"z.rsh".as_slice()
    };
    let binary = root.join(std::ffi::OsStr::from_bytes(name));
    std::fs::write(&binary, b":").unwrap();
    let target = root.join("target.txt");
    std::fs::write(&target, b":").unwrap();
    std::os::unix::fs::symlink(&target, root.join("a.rsh")).unwrap();
    std::os::unix::fs::symlink(&root, root.join("loop.rsh")).unwrap();
    let (files, errors) = rsh::sys::rsh_config_files(&root);
    assert!(errors.is_empty());
    assert_eq!(files, vec![root.join("a.rsh"), binary]);
}
