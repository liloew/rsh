mod support;
use support::{TempDir, capture};
fn run(d: &TempDir, shell: &str, source: &str) -> std::process::Output {
    let mut cmd = d.command(shell);
    if shell == "bash" {
        cmd.arg("--posix");
    }
    cmd.args(["-c", source, "outer", "original"]);
    capture(cmd).0
}
#[test]
fn source_and_dot_share_state_arguments_return_path_and_redirections() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::create_dir(d.0.join("search")).unwrap();
    std::fs::write(
        d.0.join("search/script"),
        b"VALUE=loaded; printf '<%s:%s:%s>\\n' \"$0\" \"$#\" \"$1\"; return 7; echo WRONG\n",
    )
    .unwrap();
    std::fs::write(
        d.0.join("search/modify"),
        b"printf '<%s:%s:%s>\\n' \"$0\" \"$#\" \"$1\"; set -- modified; return 7\n",
    )
    .unwrap();
    for name in ["source", "."] {
        let script = format!(
            "PATH=search:/usr/bin:/bin; {name} search/script supplied >out; printf 'status:%s\\n' $?; cat out; printf '%s:%s:%s\\n' \"$VALUE\" \"$#\" \"$1\"; {name} search/modify >out; printf 'status:%s\\n' $?; cat out; printf '%s:%s\\n' \"$#\" \"$1\""
        );
        let result = run(&d, env!("CARGO_BIN_EXE_rsh"), &script);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(result.stdout,b"status:7\n<outer:1:supplied>\nloaded:1:original\nstatus:7\n<outer:1:original>\n1:modified\n");
        // Apple dash-16 ignores operands after the sourced filename. Compare
        // explicit source arguments to Bash, and argument-free dot below.
        for shell in ["bash"] {
            if std::process::Command::new(shell)
                .arg("--version")
                .output()
                .is_err()
            {
                eprintln!("skip missing {shell}");
                continue;
            }
            let reference = run(&d, shell, &script);
            assert_eq!(result.stdout, reference.stdout, "{shell}");
            assert_eq!(result.status.code(), reference.status.code());
        }
    }
    let script = "PATH=search:/usr/bin:/bin; . search/script; . search/modify; printf 'status:%s:%s:%s\\n' $? \"$VALUE\" \"$1\"";
    let actual = run(&d, env!("CARGO_BIN_EXE_rsh"), script);
    for shell in ["dash", "bash"] {
        if std::process::Command::new(shell)
            .arg("--version")
            .output()
            .is_err()
        {
            continue;
        }
        let expected = run(&d, shell, script);
        assert_eq!(actual.stdout, expected.stdout, "{shell}");
    }
}
#[test]
fn source_is_discoverable_and_errors_do_not_exit_like_special_dot() {
    let d = TempDir::new();
    d.fixtures();
    let result = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "type source; command -v source; source; echo status:$?; source ./missing; echo alive:$?",
    );
    assert!(result.status.success());
    assert_eq!(
        result.stdout,
        b"source is a shell builtin\nsource\nstatus:1\nalive:1\n"
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("rsh: source:"));
    assert!(rsh::interactive::builtin_names().contains(&"source"));
}

#[test]
fn explicit_source_arguments_follow_existing_rsh_restore_rule() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::write(d.0.join("change"), b"set -- changed; return 3\n").unwrap();
    for name in ["source", "."] {
        let output = run(
            &d,
            env!("CARGO_BIN_EXE_rsh"),
            &format!("{name} ./change supplied; echo $?:$1"),
        );
        assert_eq!(output.stdout, b"3:original\n");
    }
}

#[test]
fn empty_sources_succeed_and_first_command_observes_the_callers_status() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::write(d.0.join("empty"), b"# no commands\n\n").unwrap();
    std::fs::write(d.0.join("status"), b"printf 'entry:%s\\n' $?\n").unwrap();
    for name in ["source", "."] {
        let script =
            format!("false; {name} ./empty; echo empty:$?; false; {name} ./status; echo final:$?");
        let result = run(&d, env!("CARGO_BIN_EXE_rsh"), &script);
        assert_eq!(result.stdout, b"empty:0\nentry:1\nfinal:0\n");
        for reference in if name == "source" {
            vec!["bash"]
        } else {
            vec!["dash", "bash"]
        } {
            if std::process::Command::new(reference)
                .arg("--version")
                .output()
                .is_err()
            {
                continue;
            }
            let expected = run(&d, reference, &script);
            assert_eq!(result.stdout, expected.stdout);
        }
    }
}

#[test]
fn source_and_dot_resolve_paths_from_cwd_without_path_search() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::create_dir(d.0.join("search")).unwrap();
    std::fs::create_dir(d.0.join("elsewhere")).unwrap();
    std::fs::write(d.0.join("local.rsh"), b"VALUE=local; echo local\n").unwrap();
    std::fs::write(d.0.join("search/local.rsh"), b"echo WRONG\n").unwrap();
    std::fs::write(d.0.join("search/only-path"), b"echo WRONG\n").unwrap();
    std::fs::write(d.0.join("elsewhere/other"), b"echo other\n").unwrap();
    for name in ["source", "."] {
        let script = format!(
            "PATH=search:/usr/bin:/bin; {name} local.rsh; echo $VALUE; {name} ./local.rsh; {name} elsewhere/other; {name} '{}'; cd elsewhere; {name} ../local.rsh",
            d.0.join("elsewhere/other").display()
        );
        let actual = run(&d, env!("CARGO_BIN_EXE_rsh"), &script);
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stderr.is_empty());
        assert_eq!(actual.stdout, b"local\nlocal\nlocal\nother\nother\nlocal\n");
        let missing = run(
            &d,
            env!("CARGO_BIN_EXE_rsh"),
            &format!("PATH=search; {name} only-path"),
        );
        assert_eq!(missing.status.code(), Some(1));
        assert!(missing.stdout.is_empty());
    }
}

#[test]
fn startup_does_not_source_files_in_cwd() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::write(d.0.join("unexpected.rsh"), b"echo WRONG; LOADED=yes\n").unwrap();
    let actual = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "echo ${LOADED-unset}; source unexpected.rsh; echo $LOADED",
    );
    assert!(actual.status.success());
    assert_eq!(actual.stdout, b"unset\nWRONG\nyes\n");
}
