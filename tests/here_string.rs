mod support;
use support::{TempDir, capture};
fn run(d: &TempDir, shell: &str, source: &str) -> std::process::Output {
    let mut command = d.command(shell);
    if shell == "bash" {
        command.arg("--posix");
    }
    command.args(["-c", source]);
    capture(command).0
}
fn bash_available() -> bool {
    let available = std::process::Command::new("bash")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !available {
        eprintln!("skip bash comparison: bash is not installed");
    }
    available
}
#[test]
fn here_strings_match_bash_including_fedora_locale_filter() {
    if !bash_available() {
        return;
    }
    // <<< is an intentional extension: dash rejects this grammar and is not a reference.
    let d = TempDir::new();
    d.fixtures();
    std::fs::write(d.0.join("glob-file"), b"x").unwrap();
    let scripts = [
        include_str!("golden/redirect/here_string.sh"),
        "HOME=/some/home; cat <<<~; x=' a  b * '; IFS=:; cat <<<$x; set -- a 'b c'; cat <<<\"$@\"; cat <<<\"$*\"",
        "x=$(cat <<< nested); printf '<%s>\\n' \"$x\"; cat <<<left | cat; f(){ cat; }; f <<<function; (cat) <<<subshell",
        "printf file >f; cat <<<string <f; cat <f <<<string; read -r x <<<read; echo \"$x\"; echo normal",
        "LANG=ja_JP.UTF-8; if [ -n \"$LANG\" ]; then if grep -q -E -i -e '^.+\\.utf-?8$' <<< \"$LANG\"; then LANG=en_US.UTF-8; fi; fi; echo \"$LANG\"",
        "if false; then cat <<< never; fi; cat <<'END'\nheredoc\nEND\ncat <<< 'a\nb'",
    ];
    for script in scripts {
        let r = run(&d, env!("CARGO_BIN_EXE_rsh"), script);
        let b = run(&d, "bash", script);
        assert_eq!(r.status.code(), b.status.code(), "{script}: {:?}", r.stderr);
        assert_eq!(r.stdout, b.stdout, "{script}");
        assert_eq!(r.stderr, b.stderr, "{script}");
    }
}
#[test]
fn large_here_string_does_not_block_even_if_input_is_unused() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::write(d.0.join("large"), vec![b'x'; 1024 * 1024]).unwrap();
    let script = "x=$(cat large); : <<<\"$x\"; cat <<<\"$x\" | wc -c";
    let r = run(&d, env!("CARGO_BIN_EXE_rsh"), script);
    assert!(r.status.success(), "{r:?}");
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), "1048577");
    if bash_available() {
        let b = run(&d, "bash", script);
        assert_eq!(r.stdout, b.stdout);
    }
    assert!(r.stderr.is_empty());
}
#[test]
fn option_is_preserved_in_pipeline_and_subshell_workers() {
    let d = TempDir::new();
    d.fixtures();
    for script in [
        "set +o here_strings; cat <<<disabled",
        "set +o here_strings; (cat <<<disabled)",
        "set +o here_strings; true | cat <<<disabled",
    ] {
        let r = run(&d, env!("CARGO_BIN_EXE_rsh"), script);
        assert!(!r.status.success());
        assert!(r.stdout.is_empty());
        assert!(String::from_utf8_lossy(&r.stderr).contains("here-strings are disabled"));
    }
    let r = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "set +o here_strings; set -o here_strings; cat <<<enabled",
    );
    assert!(r.status.success());
    assert_eq!(r.stdout, b"enabled\n");
}
