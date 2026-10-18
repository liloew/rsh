mod support;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::process::Output;
use support::TempDir;

fn run(args: &[&str]) -> Output {
    // Startup also applies to -c; never fall back to the host user's config.
    let fixture = TempDir::new();
    fixture.fixtures();
    fixture
        .command(env!("CARGO_BIN_EXE_rsh"))
        .args(args)
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
    let output = run(&["-c", "/usr/bin/awk 'BEGIN{exit(42)}'"]);
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
    for args in [vec!["-c"], vec!["-Z", "cmd"]] {
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
    let fixture = TempDir::new();
    fixture.fixtures();
    let output = fixture
        .command(env!("CARGO_BIN_EXE_rsh"))
        .arg("-c")
        .arg(OsString::from_vec(source))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, [0xff]);
}

#[test]
fn command_mode_positional_arguments() {
    let out = run(&["-c", "printf '%s\\n' \"$0\" \"$1\" \"$#\"", "name", "value"]);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"name\nvalue\n1\n");
}

#[test]
fn cli_fixture_isolates_home_and_working_directory() {
    let output = run(&[
        "-c",
        "test -d \"$HOME\" && test \"$HOME\" = \"$PWD/home\"; printf '%s' $?",
    ]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"0");
    assert!(output.stderr.is_empty());
}
