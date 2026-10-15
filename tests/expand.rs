mod support;
use support::{TempDir, capture};
fn setup() -> TempDir {
    let d = TempDir::new();
    std::fs::create_dir(d.0.join("home")).unwrap();
    for f in ["a.txt", "b.txt", "1.txt", ".hidden.txt"] {
        std::fs::write(d.0.join(f), b"").unwrap();
    }
    d
}
fn run(d: &TempDir, program: &str, source: &[u8]) -> std::process::Output {
    use std::os::unix::ffi::OsStrExt;
    let mut c = d.command(program);
    if program == "bash" {
        c.arg("--posix");
    }
    c.args([
        std::ffi::OsStr::new("-c"),
        std::ffi::OsStr::from_bytes(source),
        std::ffi::OsStr::new("fixture"),
    ]);
    capture(c).0
}
#[test]
fn expansion_goldens() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/expand");
    for entry in std::fs::read_dir(root).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().is_none_or(|e| e != "sh") {
            continue;
        }
        let d = setup();
        let out = run(&d, env!("CARGO_BIN_EXE_rsh"), &std::fs::read(&p).unwrap());
        assert_eq!(
            out.status.code(),
            Some(0),
            "{p:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.stderr.is_empty(), "{p:?}: {:?}", out.stderr);
        assert_eq!(
            d.normalize(&out.stdout),
            std::fs::read_to_string(p.with_extension("out")).unwrap(),
            "{p:?}"
        );
    }
}
#[test]
fn expansion_differential() {
    for shell in ["dash", "bash"] {
        if std::process::Command::new(shell)
            .arg("-c")
            .arg(":")
            .status()
            .is_err()
        {
            eprintln!("{shell} missing; skip differential");
            continue;
        }
        for entry in std::fs::read_dir("tests/diff/expand").unwrap() {
            let p = entry.unwrap().path();
            let source = std::fs::read(&p).unwrap();
            let a = setup();
            let b = setup();
            let ours = run(&a, env!("CARGO_BIN_EXE_rsh"), &source);
            let reference = run(&b, shell, &source);
            assert_eq!(
                ours.status.code(),
                reference.status.code(),
                "{shell} {p:?}: {:?} {:?}",
                ours.stderr,
                reference.stderr
            );
            assert_eq!(
                a.normalize(&ours.stdout),
                b.normalize(&reference.stdout),
                "{shell} {p:?}"
            );
            assert_eq!(
                a.normalize(&ours.stderr),
                b.normalize(&reference.stderr),
                "{shell} {p:?}"
            );
        }
    }
}
#[test]
fn expansion_errors_abort_without_panic() {
    for source in [
        b"printf '%s' ${missing:?required}; printf bad".as_slice(),
        b"x=; : ${x?required}; : ${x:?required}; printf bad",
        b": $((1/0)); printf bad",
        b": $((08)); printf bad",
        b": $((1+)); printf bad",
    ] {
        let d = setup();
        let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert!(!o.status.success());
        assert!(o.stdout.is_empty());
        assert!(o.stderr.starts_with(b"rsh: expansion:"));
        assert!(!o.stderr.windows(5).any(|w| w == b"panic"));
    }
}
#[test]
fn substitution_large_output_and_no_fd_growth() {
    let d = setup();
    let source=b"x=$(yes x | head -c 1100000); printf '%s\\n' ${#x}; x=$(false); printf '%s\\n' $?; x=$(true); x=$(true); x=$(true)";
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert!(o.status.success(), "{:?}", o.stderr);
    assert_eq!(o.stdout, b"1099999\n1\n");
    assert!(o.stderr.is_empty());
}
#[test]
fn reference_disagreements_are_explicit() {
    if ["dash", "bash"].iter().any(|s| {
        std::process::Command::new(s)
            .args(["-c", ":"])
            .status()
            .is_err()
    }) {
        eprintln!("reference shell missing; skip disagreements");
        return;
    }
    let d = setup();
    let ours = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        b"x='3+4'; printf '%s\\n' $((x*2))",
    );
    assert_eq!(ours.stdout, b"14\n");
    let dash = run(&d, "dash", b"x='3+4'; printf '%s\\n' $((x*2))");
    assert!(!dash.status.success());
    let bash = run(&d, "bash", b"x='3+4'; printf '%s\\n' $((x*2))");
    assert_eq!(bash.stdout, b"14\n");
    let source = b"set -- a '' b; IFS=:; printf '<%s>\\n' $@ $*";
    let ours = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert_eq!(ours.stdout, b"<a>\n<b>\n<a>\n<b>\n");
    let dash = run(&d, "dash", source);
    assert_eq!(ours.stdout, dash.stdout);
    let bash = run(&d, "bash", source);
    assert_eq!(bash.stdout, b"<a>\n<>\n<b>\n<a>\n<>\n<b>\n");
    let comma = b"printf '%s\n' $((1,2))";
    assert_eq!(run(&d, env!("CARGO_BIN_EXE_rsh"), comma).stdout, b"2\n");
    assert!(!run(&d, "dash", comma).status.success());
    assert_eq!(run(&d, "bash", comma).stdout, b"2\n");
    let scalar_at = b"set -- a b; IFS=:; x=$@; printf '%s\n' \"$x\"";
    assert_eq!(
        run(&d, env!("CARGO_BIN_EXE_rsh"), scalar_at).stdout,
        b"a:b\n"
    );
    assert_eq!(run(&d, "dash", scalar_at).stdout, b"a:b\n");
    assert_eq!(run(&d, "bash", scalar_at).stdout, b"a b\n");
}
#[cfg(target_os = "linux")]
#[test]
fn substitution_internal_fds_do_not_leak() {
    let d = setup();
    let out=run(&d,env!("CARGO_BIN_EXE_rsh"),b"x=$(ls /proc/self/fd); printf '%s\\n' \"$x\"; x=$(ls /proc/self/fd); printf '%s\\n' \"$x\"");
    assert!(out.status.success());
    for line in out.stdout.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        let fd = std::str::from_utf8(line).unwrap().parse::<u32>().unwrap();
        assert!(fd <= 3, "leaked fd {fd}");
    }
}

#[test]
fn substitution_fd_and_reaping_probe_isolated() {
    let mut c = std::process::Command::new(std::env::current_exe().unwrap());
    c.args([
        "--ignored",
        "--exact",
        "substitution_fd_probe",
        "--nocapture",
    ]);
    let (o, _) = capture(c);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
#[test]
#[ignore = "isolated: measures the process descriptor table and child set"]
fn substitution_fd_probe() {
    let d = setup();
    let mut e = rsh::env::ShellEnv::inherited().unwrap();
    e.cwd = d.0.clone();
    let mut s = rsh::exec::Shell::new(e, env!("CARGO_BIN_EXE_rsh").into());
    let count = || std::fs::read_dir("/dev/fd").unwrap().count();
    let before = count();
    let script = rsh::parser::parse(b"x=$(true); y=$(false); z=$(printf hi)").unwrap();
    for _ in 0..40 {
        assert_eq!(s.run(&script), 0);
    }
    assert_eq!(count(), before);
    assert!(!rsh::sys::has_children().unwrap());
}

#[test]
fn byte_values_globs_and_special_parameters() {
    #[cfg(target_os = "linux")]
    use std::os::unix::ffi::OsStrExt;
    let d = setup();
    #[cfg(target_os = "linux")]
    std::fs::write(d.0.join(std::ffi::OsStr::from_bytes(b"\xff.txt")), b"").unwrap();
    let out=run(&d,env!("CARGO_BIN_EXE_rsh"),b"v='\xff'; printf '%s\\n' \"$v\" ${#v} *.txt; printf '%s\\n' \"$$\" \"$(printf '%s' \"$$\")\"; sleep 0.01 & printf '%s\\n' \"$!\"");
    assert!(out.status.success(), "{:?}", out.stderr);
    let lines = out.stdout.split(|b| *b == b'\n').collect::<Vec<_>>();
    let mut expected = vec![b"\xff".as_slice(), b"1", b"1.txt", b"a.txt", b"b.txt"];
    if cfg!(target_os = "linux") {
        expected.push(b"\xff.txt");
    }
    let n = expected.len();
    assert_eq!(lines.get(..n).unwrap(), expected);
    assert_eq!(lines.get(n), lines.get(n + 1));
    assert!(!lines.get(n + 2).unwrap().is_empty());
    assert!(lines.get(n + 2).unwrap().iter().all(u8::is_ascii_digit));
}
#[test]
fn tilde_passwd_fallback_and_named_user() {
    let d = setup();
    let mut c = d.command(env!("CARGO_BIN_EXE_rsh"));
    c.env_remove("HOME")
        .args(["-c", "printf '%s\\n' ~ ~root ~rsh_nonexistent_user_12345"]);
    let (o, _) = capture(c);
    assert!(o.status.success());
    let lines = o.stdout.split(|b| *b == b'\n').collect::<Vec<_>>();
    assert_eq!(lines.first().copied(), rsh::sys::user_home(None).as_deref());
    assert_eq!(
        lines.get(1).copied(),
        rsh::sys::user_home(Some(b"root")).as_deref()
    );
    assert_eq!(
        lines.get(2).copied(),
        Some(b"~rsh_nonexistent_user_12345".as_slice())
    );
}
#[test]
fn quote_boundaries_and_subprocess_expansion_scope() {
    let source=b"x=outer; printf '%s\\n' \"$(x=inner; printf '%s' \"$x\")\" \"$x\"; printf %s ${q:=inner} | cat; printf '<%s>\\n' \"${q-unset}\"; (printf '%s' ${z:=inner}); printf '<%s>\\n' \"${z-unset}\"; IFS=:; x=':a:'; printf '<%s>\\n' \"\"$x $x\"\"; HOME=/custom; x=~\"literal\"; printf '%s\\n' \"$x\"";
    let d = setup();
    let ours = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert_eq!(
        ours.stdout,
        b"inner\nouter\ninner<unset>\ninner<unset>\n<>\n<a>\n<>\n<a>\n<>\n~literal\n"
    );
    for shell in ["dash", "bash"] {
        if let Ok(status) = std::process::Command::new(shell)
            .arg("-c")
            .arg(":")
            .status()
        {
            assert!(status.success());
            let reference = run(&d, shell, source);
            assert_eq!(ours.stdout, reference.stdout);
            assert_eq!(ours.status.code(), reference.status.code());
        }
    }
}

#[test]
fn capture_failure_and_stderr_are_preserved() {
    let d = setup();
    let o=run(&d,env!("CARGO_BIN_EXE_rsh"),b"x=$(rsh_m5_no_such_command); printf '%s\\n' \"$?\"; x=$(printf out; printf err >&2); printf '%s\\n' \"$x\"");
    assert!(o.status.success());
    assert_eq!(o.stdout, b"127\nout\n");
    let text = String::from_utf8_lossy(&o.stderr);
    assert!(text.contains("not found"));
    assert!(text.ends_with("err"));
}

#[test]
fn arithmetic_priority_and_lazy_evaluation() {
    let d = setup();
    let source=b"printf '%s\\n' \"$( (printf nested) )\" \"$((0 && (v=1)))\" \"${v-unset}\" \"$((1?2:1/0))\"";
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert!(o.status.success());
    assert_eq!(o.stdout, b"nested\n0\nunset\n2\n");
    for shell in ["dash", "bash"] {
        if std::process::Command::new(shell)
            .args(["-c", ":"])
            .status()
            .is_err()
        {
            eprintln!("skip missing {shell}");
            continue;
        }
        let out = run(&d, shell, source);
        if out.status.success() {
            assert_eq!(o.stdout, out.stdout);
        } else {
            assert_eq!(shell, "bash");
            assert!(String::from_utf8_lossy(&out.stderr).contains("division by 0"));
        }
    }
    let invalid = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        b"printf '%s' $((printf nested))",
    );
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("arithmetic"));
}
