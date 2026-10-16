use crate::support::TempDir;
use expectrl::session::OsSession;
use expectrl::{ControlCode, Eof, Expect, Session};
use std::time::Duration;
fn fixture(config: &str) -> TempDir {
    let d = TempDir::new();
    d.fixtures();
    std::fs::create_dir_all(d.0.join("config/rsh")).unwrap();
    std::fs::write(d.0.join("config/rsh/motd"), b"DAILY-MESSAGE\n").unwrap();
    std::fs::write(d.0.join("config/rsh/config.sh"), config).unwrap();
    d
}
fn launch(d: &TempDir, args: &[&str], suppress: bool) -> OsSession {
    let mut cmd = d.command(env!("CARGO_BIN_EXE_rsh"));
    cmd.env("XDG_CONFIG_HOME", d.0.join("config"))
        .env("TERM", "dumb")
        .env("PS1", "MOTD> ")
        .args(args);
    if suppress {
        cmd.env("RSH_NO_MOTD", "1");
    }
    let mut s = Session::spawn(cmd).unwrap();
    s.set_expect_timeout(Some(Duration::from_secs(8)));
    s
}
fn prompt(s: &mut OsSession) -> Vec<u8> {
    s.expect("MOTD> ").unwrap().as_bytes().to_vec()
}
fn close(mut s: OsSession) {
    s.send_line("exit").unwrap();
    s.expect(Eof).unwrap();
}
fn occurrences(bytes: &[u8], text: &[u8]) -> usize {
    bytes.windows(text.len()).filter(|b| *b == text).count()
}
#[test]
fn startup_once_manual_and_worker_exclusion() {
    let _lock = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let d = fixture("");
    let mut s = launch(&d, &["-i"], false);
    assert_eq!(occurrences(&prompt(&mut s), b"DAILY-MESSAGE"), 1);
    s.send_line("motd").unwrap();
    assert_eq!(occurrences(&prompt(&mut s), b"DAILY-MESSAGE"), 1);
    s.send_line("(true); x=$(true); true").unwrap();
    assert_eq!(occurrences(&prompt(&mut s), b"DAILY-MESSAGE"), 0);
    close(s);
}
#[test]
fn automatic_suppression_preserves_manual_invocation() {
    let _lock = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for kind in 0..3 {
        let d = fixture("");
        if kind == 2 {
            std::fs::write(d.0.join("home/.hushlogin"), b"").unwrap();
        }
        let args = if kind == 0 {
            vec!["--no-motd", "-i"]
        } else {
            vec!["-i"]
        };
        let mut s = launch(&d, &args, kind == 1);
        assert_eq!(occurrences(&prompt(&mut s), b"DAILY-MESSAGE"), 0);
        s.send_line("motd").unwrap();
        assert_eq!(occurrences(&prompt(&mut s), b"DAILY-MESSAGE"), 1);
        close(s);
    }
}
#[test]
fn configured_function_override_empty_failure_and_interrupt() {
    let _lock = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for (config, expected) in [
        ("motd() { echo custom; }\n", "custom"),
        ("motd() { :; }\n", ""),
        (
            "motd() { echo startup-error >&2; false; }\n",
            "startup-error",
        ),
    ] {
        let d = fixture(config);
        let mut s = launch(&d, &["-i"], false);
        let output = prompt(&mut s);
        assert_eq!(occurrences(&output, b"DAILY-MESSAGE"), 0);
        if !expected.is_empty() {
            assert!(String::from_utf8_lossy(&output).contains(expected));
        }
        s.send_line("printf 'status:%s\\n' $?").unwrap();
        let output = prompt(&mut s);
        assert!(String::from_utf8_lossy(&output).contains("status:0"));
        close(s);
    }
    for config in [
        "motd() { echo READY; sleep 5; echo UNREACHABLE; }\n",
        "motd() { echo READY; ! sleep 5; echo UNREACHABLE; }\n",
        "motd() { echo READY; while :; do :; done; }\n",
    ] {
        let d = fixture(config);
        let mut s = launch(&d, &["-i"], false);
        s.expect("READY").unwrap();
        std::thread::sleep(Duration::from_millis(150));
        let start = std::time::Instant::now();
        s.send(ControlCode::ETX).unwrap();
        let interrupted = prompt(&mut s);
        assert!(!String::from_utf8_lossy(&interrupted).contains("UNREACHABLE"));
        assert!(start.elapsed() < Duration::from_secs(3));
        s.send_line("printf 'alive:%s\\n' $?; jobs").unwrap();
        let output = prompt(&mut s);
        assert!(String::from_utf8_lossy(&output).contains("alive:0"));
        assert!(!String::from_utf8_lossy(&output).contains("Stopped"));
        assert!(!String::from_utf8_lossy(&output).contains("UNREACHABLE"));
        close(s);
    }
}
#[test]
fn tty_command_and_script_modes_do_not_automatically_print() {
    let _lock = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let d = fixture("");
    std::fs::write(d.0.join("script"), b"echo finished\n").unwrap();
    for args in [vec!["-i", "-c", "echo finished"], vec!["-i", "script"]] {
        let mut s = launch(&d, &args, false);
        let output = s.expect(Eof).unwrap();
        assert!(!String::from_utf8_lossy(output.as_bytes()).contains("DAILY-MESSAGE"));
        assert!(String::from_utf8_lossy(output.as_bytes()).contains("finished"));
    }
}

#[test]
fn terminal_keeps_only_sgr_and_pipeline_is_plain() {
    let _lock = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let d = fixture("");
    std::fs::write(
        d.0.join("config/rsh/motd"),
        b"\x1b[31mred\x1b[0m\x1b]0;TITLE\x07\x1b[2J\n",
    )
    .unwrap();
    let mut s = launch(&d, &["-i"], false);
    let output = prompt(&mut s);
    assert!(output.windows(5).any(|x| x == b"\x1b[31m"));
    assert!(!output.windows(5).any(|x| x == b"TITLE"));
    assert!(!output.windows(4).any(|x| x == b"\x1b[2J"));
    s.send_line("motd | cat").unwrap();
    let output = prompt(&mut s);
    assert!(String::from_utf8_lossy(&output).contains("red"));
    assert!(!output.contains(&0x1b));
    close(s);
}
#[test]
#[ignore = "manual startup timing reference; no CI timing assertion"]
fn absent_file_startup_timing() {
    let _lock = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let d = fixture("");
    std::fs::remove_file(d.0.join("config/rsh/motd")).unwrap();
    let mut normal = Vec::new();
    let mut suppressed = Vec::new();
    let mut script = Vec::new();
    for _ in 0..50 {
        let start = std::time::Instant::now();
        let output = d
            .command(env!("CARGO_BIN_EXE_rsh"))
            .args(["-c", ":"])
            .output()
            .unwrap();
        assert!(output.status.success());
        script.push(start.elapsed());
        for (args, samples) in [
            (&["-i"][..], &mut normal),
            (&["--no-motd", "-i"][..], &mut suppressed),
        ] {
            let start = std::time::Instant::now();
            let mut s = launch(&d, args, false);
            prompt(&mut s);
            samples.push(start.elapsed());
            close(s);
        }
    }
    normal.sort();
    suppressed.sort();
    script.sort();
    eprintln!(
        "median (50 samples): -c : {:?}; no-file interactive {:?}; --no-motd interactive {:?}; motd delta {:?}",
        script[25],
        normal[25],
        suppressed[25],
        normal[25].saturating_sub(suppressed[25])
    );
}
