mod support;
use std::process::Output;
use support::{TempDir, capture};
fn setup() -> TempDir {
    let d = TempDir::new();
    d.fixtures();
    std::fs::create_dir_all(d.0.join("search/nested")).unwrap();
    std::os::unix::fs::symlink("child", d.0.join("link")).unwrap();
    std::fs::write(
        d.0.join("source-file"),
        b"printf 'source:%s:%s\\n' \"$#\" \"$1\"; set -- modified; return 4; echo BAD\n",
    )
    .unwrap();
    d
}
fn run(d: &TempDir, program: &str, source: &str) -> Output {
    let mut c = d.command(program);
    if program == "bash" {
        c.arg("--posix");
    }
    c.args(["-c", source, "fixture"]);
    capture(c).0
}
fn available(shell: &str) -> bool {
    if std::process::Command::new(shell)
        .args(["-c", ":"])
        .status()
        .is_err()
    {
        eprintln!("skip missing {shell}");
        false
    } else {
        true
    }
}
fn normalize(d: &TempDir, name: &str, out: &[u8]) -> String {
    if out.is_empty() {
        return String::new();
    }
    let s = d.normalize(out).replace("/private<TMP>", "<TMP>");
    if name == "times" {
        assert_eq!(s.lines().count(), 2);
        for line in s.lines() {
            assert_eq!(line.split_whitespace().count(), 2);
            assert!(line.contains('m') && line.contains('s'));
        }
        return "<TIMES>\n".to_string();
    }
    if name == "alias" {
        return s.strip_prefix("alias ").unwrap_or(&s).to_string();
    }
    s
}
#[test]
fn builtin_goldens() {
    for e in std::fs::read_dir("tests/golden/builtins").unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|e| e != "sh") {
            continue;
        }
        let name = p.file_stem().unwrap().to_str().unwrap();
        let d = setup();
        let o = run(
            &d,
            env!("CARGO_BIN_EXE_rsh"),
            &std::fs::read_to_string(&p).unwrap(),
        );
        let status = std::fs::read_to_string(p.with_extension("status"))
            .unwrap_or("0\n".into())
            .trim()
            .parse::<i32>()
            .unwrap();
        assert_eq!(o.status.code(), Some(status), "{name}: {:?}", o.stderr);
        assert!(o.stderr.is_empty(), "{name}: {:?}", o.stderr);
        assert_eq!(
            normalize(&d, name, &o.stdout),
            std::fs::read_to_string(p.with_extension("out")).unwrap(),
            "{name}"
        );
    }
}
#[test]
fn builtin_differential() {
    for sh in ["dash", "bash"] {
        if !available(sh) {
            continue;
        }
        for e in std::fs::read_dir("tests/diff/builtins").unwrap() {
            let p = e.unwrap().path();
            let name = p.file_stem().unwrap().to_str().unwrap();
            let source = std::fs::read_to_string(&p).unwrap();
            let a = setup();
            let b = setup();
            let ours = run(&a, env!("CARGO_BIN_EXE_rsh"), &source);
            let reference = run(&b, sh, &source);
            assert_eq!(
                ours.status.code(),
                reference.status.code(),
                "{sh} {name}: {:?} {:?}",
                ours.stderr,
                reference.stderr
            );
            assert_eq!(
                normalize(&a, name, &ours.stdout),
                normalize(&b, name, &reference.stdout),
                "{sh} {name}"
            );
            assert_eq!(
                normalize(&a, name, &ours.stderr),
                normalize(&b, name, &reference.stderr),
                "{sh} {name}: stderr"
            );
        }
    }
}

#[test]
fn errexit_contexts_and_pipefail() {
    let d = setup();
    let cases = [
        ("set -e; false; echo BAD", 1, ""),
        ("set -e; false && echo BAD; echo ok", 0, "ok\n"),
        ("set -e; true && false; echo BAD", 1, ""),
        ("set -e; false || false; echo BAD", 1, ""),
        ("set -e; ! true; echo ok", 0, "ok\n"),
        (
            "set -e; f(){ false; echo continued; }; if f; then echo yes; fi",
            0,
            "continued\nyes\n",
        ),
        ("set -e; f(){ false; echo BAD; }; f", 1, ""),
        ("set -e; { false; echo BAD; } | cat; echo ok", 0, "ok\n"),
        (
            "set -e; ! { false; echo continued; }; echo ok",
            0,
            "continued\nok\n",
        ),
        (
            "set -e; while false; do echo BAD; done; until true; do echo BAD; done; echo ok",
            0,
            "ok\n",
        ),
        (
            "set -e; f(){ false; echo continued; }; f || echo BAD; echo ok",
            0,
            "continued\nok\n",
        ),
    ];
    for (source, status, stdout) in cases {
        let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert_eq!(o.status.code(), Some(status), "{source}: {:?}", o.stderr);
        assert_eq!(o.stdout, stdout.as_bytes(), "{source}");
        for sh in ["dash", "bash"] {
            if available(sh) {
                let r = run(&d, sh, source);
                assert_eq!(r.status.code(), Some(status), "{sh} {source}");
                assert_eq!(r.stdout, o.stdout, "{sh} {source}");
            }
        }
    }
    for source in [
        "set -o pipefail; false | true; echo $?",
        "set -o pipefail; (exit 3) | (exit 5) | true; echo $?",
    ] {
        let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert_eq!(
            o.stdout,
            if source.contains("exit 3") {
                b"5\n".as_slice()
            } else {
                b"1\n"
            }
        );
        for sh in ["dash", "bash"] {
            if available(sh) && run(&d, sh, "set -o pipefail").status.success() {
                assert_eq!(run(&d, sh, source).stdout, o.stdout);
            }
        }
    }
}

#[test]
fn readonly_and_special_errors_are_fatal() {
    let d = setup();
    for source in [
        "readonly v=one; v=two; echo BAD",
        "readonly v; : ${v:=two}; echo BAD",
        "readonly v=one; export v=two; echo BAD",
        "readonly v=one; unset v; echo BAD",
        "readonly v=one; read v <<EOF\ntwo\nEOF\n",
    ] {
        let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert_eq!(o.status.code(), Some(1), "{source}: {:?}", o.stderr);
        assert!(o.stdout.is_empty());
        assert!(String::from_utf8_lossy(&o.stderr).contains("readonly"));
    }
    for source in [
        ". missing-file; echo BAD",
        "eval 'if true; then'; echo BAD",
        "export 'bad-name=x'; echo BAD",
        "readonly -z; echo BAD",
        "unset -z; echo BAD",
        "set -Z; echo BAD",
        "times unexpected; echo BAD",
        "shift 999; echo BAD",
        "trap action BAD_SIGNAL; echo BAD",
    ] {
        let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert!(!o.status.success(), "{source}");
        assert!(o.stdout.is_empty(), "{source}");
        assert!(o.stderr.starts_with(b"rsh:"), "{source}: {:?}", o.stderr);
    }
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "A=kept :; echo \"$A\"; command export bad-name=x; echo survived",
    );
    assert_eq!(o.stdout, b"kept\nsurvived\n");
}

#[test]
fn nounset_noexec_noclobber_and_xtrace() {
    let d = setup();
    for source in [
        "set -u; echo $missing; echo BAD",
        "set -u; echo ${#missing}; echo BAD",
        "set -u; echo $1; echo BAD",
    ] {
        let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert!(!o.status.success());
        assert!(o.stdout.is_empty());
        assert!(String::from_utf8_lossy(&o.stderr).contains("not set"));
    }
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "set -u; set --; printf '%s:<%s>\\n' \"$#\" \"$*\"; printf '%s\\n' \"${missing-default}\"",
    );
    assert_eq!(o.stdout, b"0:<>\ndefault\n");
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "set -n; echo BAD >should-not-exist",
    );
    assert!(o.status.success());
    assert!(!d.0.join("should-not-exist").exists());
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "echo first >f; set -C; echo second >f; echo status:$?; echo forced >|f; cat f; echo ignored >/dev/null",
    );
    assert_eq!(o.stdout, b"status:1\nforced\n");
    assert!(String::from_utf8_lossy(&o.stderr).contains("f:"));
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "PS4='trace:$LINENO '; set -x\necho \"a b\"\nset +x\n",
    );
    assert_eq!(o.stdout, b"a b\n");
    assert_eq!(o.stderr, b"trace:2 echo a b\ntrace:3 set +x\n");
}

#[test]
fn read_pipeline_streaming_and_eof() {
    use std::io::Write;
    use std::process::Stdio;
    let d = setup();
    let mut c = d.command(env!("CARGO_BIN_EXE_rsh"));
    c.args([
        "-c",
        "read -r a; read b; printf '<%s><%s>\\n' \"$a\" \"$b\"; cat",
    ]);
    let mut child = c
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"first\\value\nsecond\\ value\nrest\n")
        .unwrap();
    let o = child.wait_with_output().unwrap();
    assert_eq!(o.stdout, b"<first\\value><second value>\nrest\n");
    let mut child = d
        .command(env!("CARGO_BIN_EXE_rsh"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"read x\ninline data\nprintf '<%s>:%s\\n' \"$x\" \"$LINENO\"\n")
        .unwrap();
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success());
    assert_eq!(o.stdout, b"<inline data>:3\n");
}

#[test]
fn exec_replaces_process_and_persistent_descriptors() {
    let d = setup();
    let mut c = d.command(env!("CARGO_BIN_EXE_rsh"));
    c.args([
        "-c",
        "trap 'echo BAD' EXIT; exec /bin/sh -c 'printf \"%s\\n\" \"$$\"' sh",
    ]);
    let child = c.stdout(std::process::Stdio::piped()).spawn().unwrap();
    let pid = child.id();
    let o = child.wait_with_output().unwrap();
    assert_eq!(o.stdout, format!("{pid}\n").as_bytes());
    assert!(o.status.success());
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "exec rsh_no_such_command; echo BAD",
    );
    assert_eq!(o.status.code(), Some(127));
    assert!(o.stdout.is_empty());
    assert!(String::from_utf8_lossy(&o.stderr).contains("not found"));
}

#[test]
fn aliases_are_eligible_only_when_enabled_and_cycle_safe() {
    let d = setup();
    let source = "alias rsh_m7_say='echo hello'\nrsh_m7_say world\n";
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert_eq!(o.stdout, b"hello world\n");
    assert!(o.status.success());
    let disabled = format!("set +o expand_aliases\n{source}");
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), &disabled);
    assert_eq!(o.status.code(), Some(127));
    let source = "set -o expand_aliases\nalias rsh_m7_say='echo hello'\nrsh_m7_say world\nalias one='echo ' word=expanded\none word\nalias rsh_m7_self='rsh_m7_self'\nrsh_m7_self\n";
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert_eq!(o.stdout, b"hello world\nexpanded\n");
    assert_eq!(o.status.code(), Some(127));
    for sh in ["dash", "bash"] {
        if !available(sh) {
            continue;
        }
        let source = if sh == "bash" {
            source.replace("set -o expand_aliases", "shopt -s expand_aliases")
        } else {
            source.replace("set -o expand_aliases", ":")
        };
        let r = run(&d, sh, &source);
        assert_eq!(r.stdout, o.stdout);
        assert_eq!(r.status.code(), Some(127));
    }
}

#[test]
fn trap_context_inheritance_listing_and_status() {
    let d = setup();
    let source = "trap 'echo EXIT:$?' EXIT; trap 'v=caught; echo signal:$?' USR1; false; kill -s SIGUSR1 $$; echo \"$v\"; (kill -0 $$; echo child); printf 'capture:<%s>\\n' \"$(echo sub)\"; trap - USR1; trap -p";
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert_eq!(o.status.code(), Some(0), "{:?}", o.stderr);
    assert_eq!(
        o.stdout,
        b"signal:0\ncaught\nchild\ncapture:<sub>\ntrap -- 'echo EXIT:$?' EXIT\nEXIT:0\n"
    );
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "trap 'echo BAD' EXIT; trap '' USR1; (kill -s USR1 $$; echo ignored); trap - EXIT",
    );
    assert!(o.status.success());
    assert_eq!(o.stdout, b"ignored\n");
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), "trap 'exit 7' EXIT; exit 3");
    assert_eq!(o.status.code(), Some(7));
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "trap 'echo listing' EXIT; saved=$(trap); trap - EXIT; printf '%s\\n' \"$saved\"",
    );
    assert_eq!(o.stdout, b"trap -- 'echo listing' EXIT\n");
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "trap '' CHLD; /usr/bin/true; echo $?; (exit 4) & wait $!; echo $?",
    );
    assert_eq!(o.stdout, b"0\n4\n");
    assert!(o.status.success(), "{:?}", o.stderr);
}

#[test]
fn getopts_clusters_errors_reset_and_explicit_arguments() {
    let d = setup();
    let source = "set -- -ab value; getopts 'ab:' x; printf '%s:%s\\n' \"$x\" \"$OPTIND\"; getopts 'ab:' x; printf '%s:%s:%s\\n' \"$x\" \"$OPTARG\" \"$OPTIND\"; OPTIND=1; getopts 'ab:' x; echo \"$x\"; OPTIND=1; OPTERR=0; getopts a x -z; printf '%s:%s:<%s>\\n' \"$?\" \"$x\" \"${OPTARG-}\"; getopts a x operand; echo $?";
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert_eq!(o.stdout, b"a:1\nb:value:3\na\n0:?:<>\n1\n");
    assert!(o.stderr.is_empty());
    for source in ["getopts a x -z; echo $?", "getopts 'a:' x -a; echo $?"] {
        let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert_eq!(o.stdout, b"0\n");
        assert!(String::from_utf8_lossy(&o.stderr).contains("getopts"));
    }
}

#[test]
fn test_argument_count_rules_and_errors() {
    let d = setup();
    let cases = [
        ("test", 1),
        ("test ''", 1),
        ("test !", 0),
        ("test ! ''", 0),
        ("test -n ''", 1),
        ("test -z ''", 0),
        ("test a = a", 0),
        ("test -n = -n", 0),
        ("test ! a b", 2),
        ("test ! a -a ''", 0),
        ("test '(' -n '' ')'", 1),
        ("test '(' a -o '' ')' -a ! ''", 0),
        ("[ a = a ]", 0),
        ("[ a", 2),
        ("test 1 -eq bad", 2),
        ("test '(' a", 2),
        ("test -e source-file", 0),
        ("test -f source-file", 0),
        ("test -d child", 0),
        ("test -L link", 0),
        ("test -s source-file", 0),
        ("test 2 -ge 1", 0),
        ("test 2 -le 1", 1),
    ];
    for (source, status) in cases {
        let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert_eq!(o.status.code(), Some(status), "{source}: {:?}", o.stderr);
        if status == 2 {
            assert!(!o.stderr.is_empty());
        }
        for sh in ["dash", "bash"] {
            if available(sh) {
                let r = run(&d, sh, source);
                assert_eq!(
                    r.status.code(),
                    Some(status),
                    "{sh} {source}: {:?}",
                    r.stderr
                );
            }
        }
    }
}

#[test]
fn numeric_printf_errors_continue_and_bytes_survive() {
    let d = setup();
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "printf '%d:%s\\n' bad after 4 next",
    );
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(o.stdout, b"0:after\n4:next\n");
    assert!(String::from_utf8_lossy(&o.stderr).contains("numeric"));
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        r#"printf '%d:%c:%b:%s' "'A" Z 'a\000b' done"#,
    );
    assert!(o.status.success(), "{:?}", o.stderr);
    assert!(o.stdout.ends_with(b"a\0b:done"));
}

#[test]
fn startup_options_source_arguments_and_command_default_path() {
    let d = setup();
    for (options, source, status) in [
        (vec!["-e"], "false; echo BAD", 1),
        (vec!["-u"], "echo $missing", 2),
        (vec!["-n"], "echo BAD >never", 0),
        (vec!["-o", "pipefail"], "false | true", 1),
    ] {
        let mut c = d.command(env!("CARGO_BIN_EXE_rsh"));
        c.args(options).args(["-c", source]);
        let o = capture(c).0;
        assert_eq!(o.status.code(), Some(status));
        assert!(o.stdout.is_empty());
    }
    assert!(!d.0.join("never").exists());
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "PATH=/bad; command -p printenv PATH; A=prefix command -p printenv A; echo ${A-unset}",
    );
    assert_eq!(o.stdout, b"/bad\nprefix\nunset\n");
    let o = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "set -- outer rest; . ./source-file inner value; printf '%s:%s:%s\\n' \"$?\" \"$#\" \"$1\"",
    );
    assert_eq!(o.stdout, b"source:2:inner\n4:2:outer\n");
}

#[test]
fn audited_reference_differences_are_explicit() {
    let d = setup();
    let ours = run(&d, env!("CARGO_BIN_EXE_rsh"), "echo -n no; echo 'a\\nb'");
    assert_eq!(ours.stdout, b"noa\\nb\n");
    for sh in ["dash", "bash"] {
        if !available(sh) {
            continue;
        }
        let r = run(&d, sh, "echo -n no; echo 'a\\nb'");
        assert!(r.status.success());
        assert!(
            r.stdout == ours.stdout
                || r.stdout == b"noa\nb\n"
                || r.stdout == b"-n no\na\nb\n"
                || r.stdout == b"-n no\na\\nb\n"
        );
    }
    let source = r#"set -- -ab value; getopts 'ab:' x; printf '%s:%s\n' "$x" "$OPTIND"; getopts 'ab:' x; printf '%s:%s:%s\n' "$x" "$OPTARG" "$OPTIND""#;
    assert_eq!(
        run(&d, env!("CARGO_BIN_EXE_rsh"), source).stdout,
        b"a:1\nb:value:3\n"
    );
    for sh in ["dash", "bash"] {
        if available(sh) {
            let r = run(&d, sh, source);
            assert!(r.stdout == b"a:1\nb:value:3\n" || r.stdout == b"a:2\nb:value:3\n");
        }
    }
    let source =
        r#"set -- outer rest; . ./source-file inner value; printf '%s:%s:%s\n' "$?" "$#" "$1""#;
    for sh in ["dash", "bash"] {
        if available(sh) {
            let r = run(&d, sh, source);
            assert!(
                r.stdout == b"source:2:inner\n4:2:outer\n"
                    || r.stdout == b"source:2:outer\n4:1:modified\n"
                    || r.stdout == b"source:2:inner\n4:1:modified\n"
            );
        }
    }
    let source = "trap 'echo caught' USR1; kill -s SIGUSR1 $$";
    assert_eq!(
        run(&d, env!("CARGO_BIN_EXE_rsh"), source).stdout,
        b"caught\n"
    );
    for sh in ["dash", "bash"] {
        if available(sh) {
            let r = run(&d, sh, source);
            assert!(r.stdout == b"caught\n" || !r.status.success() && !r.stderr.is_empty());
        }
    }
}

#[test]
fn builtin_error_paths_have_diagnostics_and_status() {
    let d = setup();
    let cases = [
        ("cd no-such-directory", 1),
        ("pwd -Z", 2),
        ("printf", 2),
        ("printf '%d' invalid", 1),
        ("[ a", 2),
        ("read bad-name", 2),
        ("alias rsh_no_alias", 1),
        ("unalias rsh_no_alias", 1),
        ("unalias", 2),
        ("getopts", 2),
        ("umask not-octal", 1),
        ("wait invalid-pid", 127),
        ("kill", 2),
        ("kill -s BAD_SIGNAL 999999", 2),
        ("command -Z", 2),
        ("type rsh_no_type", 1),
    ];
    for (source, status) in cases {
        let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
        assert_eq!(o.status.code(), Some(status), "{source}: {:?}", o.stderr);
        assert!(!o.stderr.is_empty(), "{source}");
        for sh in ["dash", "bash"] {
            if !available(sh) {
                continue;
            }
            let r = run(&d, sh, source);
            // Local dash treats operand-less unalias as a successful no-op.
            if sh == "dash" && source == "unalias" {
                assert!(r.status.success());
                continue;
            }
            assert!(!r.status.success(), "{sh} {source}");
            if !matches!(source, "getopts" | "unalias") {
                assert!(
                    !r.stderr.is_empty()
                        || source.starts_with("wait")
                        || (source.starts_with("type") && !r.stdout.is_empty()),
                    "{sh} {source}"
                );
            }
        }
    }
}

#[test]
fn read_ifs_empty_and_escaped_delimiters() {
    let d = setup();
    let source = r#"printf ' a b \n' | { IFS= read -r x; printf '<%s>\n' "$x"; }
printf '%s\n' 'a\:b:c' | { IFS=: read a b; printf '<%s><%s>\n' "$a" "$b"; }
printf '\n' | { read a b; printf '%s:<%s><%s>\n' "$?" "$a" "$b"; }
"#;
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert_eq!(o.stdout, b"< a b >\n<a:b><c>\n0:<><>\n");
    for sh in ["dash", "bash"] {
        if available(sh) {
            let r = run(&d, sh, source);
            assert_eq!(r.stdout, o.stdout);
            assert_eq!(r.status.code(), o.status.code());
        }
    }
}

#[test]
fn wait_interrupts_for_a_trap_and_keeps_child_status() {
    let d = setup();
    let source = "trap 'echo signal' USR1; parent=$$; (sleep 0.02; kill -s USR1 \"$parent\") & sender=$!; sleep 0.15 & child=$!; wait \"$child\"; status=$?; echo interrupted:$status; wait \"$child\"; echo child:$?; wait \"$sender\"";
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert!(o.status.success(), "{:?}", o.stderr);
    let number = rsh::sys::signal_number(b"USR1").unwrap();
    assert_eq!(
        o.stdout,
        format!("signal\ninterrupted:{}\nchild:0\n", 128 + number).as_bytes()
    );
}

#[test]
fn lineno_and_worker_metadata_are_preserved() {
    let d = setup();
    let source = "# comment\nprintf '%s\\n' \"$LINENO\"\nf(){\nprintf 'function:%s\\n' \"$LINENO\"\n}\nf | cat\nprintf 'capture:%s\\n' \"$(f)\"\n";
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert!(o.status.success());
    assert_eq!(o.stdout, b"2\nfunction:4\ncapture:function:4\n");
}

#[test]
fn listing_quotes_roundtrip_and_empty_alias() {
    let d = setup();
    let source = "v=\"space ' quote\"; export v; export -p >vars; unset v; . ./vars; printf '<%s>\\n' \"$v\"; set -o expand_aliases\nalias empty=''\nempty\necho after\n";
    let o = run(&d, env!("CARGO_BIN_EXE_rsh"), source);
    assert!(o.status.success(), "{:?}", o.stderr);
    assert_eq!(o.stdout, b"<space ' quote>\nafter\n");
    let exported = std::fs::read(d.0.join("vars")).unwrap();
    assert!(rsh::parser::parse(&exported).is_ok());
}

#[test]
fn empty_file_test_operands_do_not_resolve_to_current_directory() {
    let d = setup();
    let source = "for op in -e -f -d -r -w -x -s -L -h; do test \"$op\" ''; printf '%s:%s\\n' \"$op\" \"$?\"; done; INCLUDE=; if [ -e \"$INCLUDE\" ]; then echo WRONG; else echo no-include; fi";
    let expected = b"-e:1\n-f:1\n-d:1\n-r:1\n-w:1\n-x:1\n-s:1\n-L:1\n-h:1\nno-include\n";
    for sh in [env!("CARGO_BIN_EXE_rsh"), "dash", "bash"] {
        if (sh == "dash" || sh == "bash") && !available(sh) {
            continue;
        }
        let output = run(&d, sh, source);
        assert!(output.status.success(), "{sh}: {output:?}");
        assert_eq!(output.stdout, expected, "{sh}");
        assert!(output.stderr.is_empty(), "{sh}: {output:?}");
    }
}

#[test]
fn portable_fedora_locale_filter_uses_a_pipe_instead_of_here_string() {
    let d = setup();
    let source = "LANG=ja_JP.UTF-8; if printf '%s\\n' \"$LANG\" | grep -q -E -i -e '^.+\\.utf-?8$'; then LANG=en_US.UTF-8; fi; printf '%s\\n' \"$LANG\"";
    for sh in [env!("CARGO_BIN_EXE_rsh"), "dash", "bash"] {
        if (sh == "dash" || sh == "bash") && !available(sh) {
            continue;
        }
        let out = run(&d, sh, source);
        assert!(out.status.success(), "{sh}: {out:?}");
        assert_eq!(out.stdout, b"en_US.UTF-8\n", "{sh}");
        assert!(out.stderr.is_empty(), "{sh}: {out:?}");
    }
}
