mod support;
use support::{TempDir, capture};
fn setup() -> TempDir {
    let d = TempDir::new();
    d.fixtures();
    d
}
fn available(shell: &str) -> bool {
    match std::process::Command::new(shell).args(["-c", ":"]).status() {
        Ok(s) => s.success(),
        Err(_) => {
            eprintln!("skip missing reference {shell}");
            false
        }
    }
}
fn run(d: &TempDir, shell: &str, source: &str) -> std::process::Output {
    let mut cmd = d.command(shell);
    if shell == "bash" {
        cmd.arg("--posix");
    }
    cmd.args(["-c", source, "fixture"]);
    capture(cmd).0
}
#[test]
fn control_goldens() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/control");
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "sh") {
            continue;
        }
        let dir = setup();
        let out = run(
            &dir,
            env!("CARGO_BIN_EXE_rsh"),
            &std::fs::read_to_string(&path).unwrap(),
        );
        assert_eq!(out.status.code(), Some(0), "{path:?}: {:?}", out.stderr);
        assert!(out.stderr.is_empty(), "{path:?}: {:?}", out.stderr);
        assert_eq!(
            dir.normalize(&out.stdout),
            std::fs::read_to_string(path.with_extension("out")).unwrap(),
            "{path:?}"
        );
    }
}
#[test]
fn control_differential() {
    for shell in ["dash", "bash"] {
        if !available(shell) {
            continue;
        }
        for entry in std::fs::read_dir("tests/diff/control").unwrap() {
            let path = entry.unwrap().path();
            let source = std::fs::read_to_string(&path).unwrap();
            let a = setup();
            let b = setup();
            let ours = run(&a, env!("CARGO_BIN_EXE_rsh"), &source);
            let reference = run(&b, shell, &source);
            assert_eq!(
                ours.status.code(),
                reference.status.code(),
                "{shell} {path:?}: {:?} {:?}",
                ours.stderr,
                reference.stderr
            );
            assert_eq!(
                a.normalize(&ours.stdout),
                b.normalize(&reference.stdout),
                "{shell} {path:?}"
            );
            assert_eq!(
                a.normalize(&ours.stderr),
                b.normalize(&reference.stderr),
                "{shell} {path:?}"
            );
        }
    }
}
#[test]
fn invalid_control_and_recursion_are_bounded() {
    let d = setup();
    for source in [
        "f(){ f; }; f",
        "f(){ true | f; }; f",
        "for x in a; do break 0; done",
        "continue invalid",
        "return 3",
        "set -- a; shift 2",
    ] {
        let out = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert!(!out.status.success(), "{source}");
        assert!(
            out.stderr.starts_with(b"rsh:"),
            "{source}: {:?}",
            out.stderr
        );
        assert!(!out.stderr.windows(5).any(|w| w == b"panic"));
    }
    let out = run(&d, env!("CARGO_BIN_EXE_rsh"), "f(){ f; }; f");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("recursion depth limit"));
}
#[test]
fn shift_and_return_error_policy() {
    let d = setup();
    let out = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "set -- a; shift 2; printf '%s:%s:%s\\n' \"$?\" \"$#\" \"$1\"; return 7",
    );
    assert!(out.stdout.is_empty());
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("shift"));
    for n in ["-1", "x", "999999999999999999999999"] {
        let o = run(
            &d,
            env!("CARGO_BIN_EXE_rsh"),
            &format!("set -- a; shift {n}"),
        );
        assert_eq!(o.status.code(), Some(1));
    }
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "set -- a b; shift 0; shift; printf '%s:%s\\n' \"$#\" \"$1\"",
    );
    assert_eq!(o.stdout, b"1:b\n");
}
#[test]
fn pipeline_while_read() {
    let d = setup();
    let out = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "printf 'a\\nb\\n' | while read x; do printf '%s\\n' \"$x\"; done",
    );
    assert_eq!(out.stdout, b"a\nb\n");
}

#[test]
fn stress_probe_isolated() {
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args(["--ignored", "--exact", "stress_probe", "--nocapture"]);
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
#[ignore = "isolated: descriptor and memory measurements must not race other tests"]
fn stress_probe() {
    let d = setup();
    let mut e = rsh::env::ShellEnv::inherited().unwrap();
    e.cwd = d.0.clone();
    let mut shell = rsh::exec::Shell::new(e, env!("CARGO_BIN_EXE_rsh").into());
    let ast =
        rsh::parser::parse(b"i=0; while :; do i=$((i+1)); case $i in 100000) break;; esac; done")
            .unwrap();
    let count = || std::fs::read_dir("/dev/fd").unwrap().count();
    let before = count();
    assert_eq!(shell.run(&ast), 0);
    let first = rsh::sys::peak_resident_bytes().unwrap();
    assert_eq!(shell.env.get(b"i"), Some(b"100000".as_slice()));
    assert_eq!(shell.run(&ast), 0);
    let second = rsh::sys::peak_resident_bytes().unwrap();
    assert!(
        second.saturating_sub(first) < 2 * 1024 * 1024,
        "resident growth after a second 100k loop: {} bytes",
        second.saturating_sub(first)
    );
    assert_eq!(count(), before);
    assert!(!rsh::sys::has_children().unwrap());
}

#[test]
fn function_errors_restore_parameters_and_descriptors() {
    let d = setup();
    let out = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "set -- outer; f(){ printf BAD; } <missing; f inner; printf '%s:%s:%s\\n' \"$?\" \"$#\" \"$1\"; f(){ return 255; } >out; f; printf '%s:%s\\n' \"$?\" \"$1\"",
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, b"1:1:outer\n255:outer\n");
    assert!(String::from_utf8_lossy(&out.stderr).contains("missing"));
    assert_eq!(std::fs::read(d.0.join("out")).unwrap(), b"");
}

#[test]
fn deep_compounds_with_recursive_functions_are_bounded() {
    let d = setup();
    let source = format!("f(){{ {} f; {} }}; f", "{ ".repeat(30), "}; ".repeat(30));
    let out = run(&d, env!("CARGO_BIN_EXE_rsh"), &source);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nesting limit"));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("stack overflow"));
}

#[test]
fn reference_policy_differences() {
    let d = setup();
    for (source, want) in [("return 7", 1), ("set -- a; shift 2", 1)] {
        let ours = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert_eq!(ours.status.code(), Some(want));
        assert!(!ours.stderr.is_empty());
        if available("dash") {
            let out = run(&d, "dash", source);
            assert_eq!(
                out.status.code(),
                Some(if source.starts_with("return") { 7 } else { 2 })
            );
        }
        if available("bash") {
            assert_eq!(run(&d, "bash", source).status.code(), Some(1));
        }
    }
    let prefix = "A=outer; f(){ printf 'inside:%s\\n' \"$A\"; A=changed; }; A=temp f; printf 'after:%s\\n' \"$A\"";
    let ours = run(&d, env!("CARGO_BIN_EXE_rsh"), prefix);
    assert_eq!(ours.stdout, b"inside:temp\nafter:outer\n");
    if available("dash") {
        assert_eq!(run(&d, "dash", prefix).stdout, ours.stdout);
    }
    if available("bash") {
        let o = run(&d, "bash", prefix);
        assert!(
            o.stdout == ours.stdout
                || o.stdout == b"inside:temp\nafter:changed\n"
                || o.stdout == b"inside:temp\nafter:temp\n"
        );
    }
    let source = "for x in a b; do f(){ break; }; f; printf '%s' \"$x\"; done";
    assert_eq!(run(&d, env!("CARGO_BIN_EXE_rsh"), source).stdout, b"ab");
    if available("dash") {
        assert_eq!(run(&d, "dash", source).stdout, b"ab");
    }
    if available("bash") {
        let o = run(&d, "bash", source);
        assert!(o.stdout == b"ab" || o.stdout.is_empty());
    }
}
