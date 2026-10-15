use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rsh"))
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .output()
        .unwrap()
}

#[test]
fn executes_external_command_with_arguments() {
    let output = run(&["-c", " \t /usr/bin/printf %s hello\t "]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"hello");
    assert!(output.stderr.is_empty());
}

#[test]
fn propagates_nonzero_status() {
    let output = run(&["-c", "/usr/bin/awk BEGIN{exit(42)}"]);
    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn empty_command_succeeds() {
    for source in ["", " \t "] {
        let output = run(&["-c", source]);
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn invalid_cli_reports_usage() {
    for args in [
        vec![],
        vec!["-c"],
        vec!["-x", "cmd"],
        vec!["-c", "cmd", "extra"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.starts_with(b"rsh: arguments: usage:"));
    }
}

#[test]
fn spawn_failure_is_reported() {
    let output = run(&["-c", "/nonexistent/rsh-m0-command"]);
    assert!(!output.status.success());
    assert!(
        output
            .stderr
            .starts_with(b"rsh: /nonexistent/rsh-m0-command:")
    );
}

#[test]
fn preserves_non_utf8_arguments() {
    let mut source = b"/usr/bin/printf %s ".to_vec();
    source.push(0xff);
    let output = Command::new(env!("CARGO_BIN_EXE_rsh"))
        .arg("-c")
        .arg(OsString::from_vec(source))
        .env_clear()
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, [0xff]);
}
