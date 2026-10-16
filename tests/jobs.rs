mod support;
use support::{TempDir, capture};
fn run(shell: &str, source: &str) -> std::process::Output {
    let d = TempDir::new();
    d.fixtures();
    let mut c = d.command(shell);
    if shell == "bash" {
        c.arg("--posix");
    }
    c.args(["-c", source]);
    capture(c).0
}
#[test]
fn noninteractive_differential() {
    for entry in std::fs::read_dir("tests/diff/jobs").unwrap() {
        let p = entry.unwrap().path();
        let source = std::fs::read_to_string(&p).unwrap();
        let ours = run(env!("CARGO_BIN_EXE_rsh"), &source);
        for shell in ["dash", "bash"] {
            if std::process::Command::new(shell)
                .args(["-c", ":"])
                .status()
                .is_err()
            {
                eprintln!("skip missing {shell}");
                continue;
            }
            let other = run(shell, &source);
            assert_eq!(ours.stdout, other.stdout, "{shell} {p:?}");
            assert_eq!(ours.status.code(), other.status.code(), "{shell} {p:?}");
            assert!(ours.stderr.is_empty(), "{:?}", ours.stderr);
            assert!(other.stderr.is_empty(), "{:?}", other.stderr);
        }
    }
}
#[test]
fn race_200_jobs_preserves_each_status() {
    let source = "pids=; i=0; while test $i -lt 200; do (exit $((i%20))) & pids=\"$pids $!\"; i=$((i+1)); done; for p in $pids; do wait $p; printf '%s\\n' $?; done";
    let o = run(env!("CARGO_BIN_EXE_rsh"), source);
    assert!(o.status.success(), "{:?}", o.stderr);
    assert!(o.stderr.is_empty());
    let expected = (0..200)
        .map(|i| format!("{}\n", i % 20))
        .collect::<String>();
    assert_eq!(o.stdout, expected.as_bytes());
}
#[test]
fn background_stdin_is_null_and_kill_list() {
    let o = run(
        env!("CARGO_BIN_EXE_rsh"),
        "cat & p=$!; wait $p; echo status:$?; kill -l 15; kill -l TERM; wait 999999; echo unknown:$?",
    );
    assert_eq!(o.stdout, b"status:0\nTERM\n15\nunknown:127\n");
    assert!(o.stderr.is_empty());
}
#[test]
fn monitor_option_is_reported_but_noninteractive_does_not_control_tty() {
    let o = run(
        env!("CARGO_BIN_EXE_rsh"),
        "set -m; case $- in *m*) echo on;; esac; true | true; set +m; case $- in *m*) echo BAD;; *) echo off;; esac",
    );
    assert_eq!(o.stdout, b"on\noff\n");
    assert!(o.stderr.is_empty());
}
#[test]
fn mixed_children_reaped_in_isolated_process() {
    let mut c = std::process::Command::new(std::env::current_exe().unwrap());
    c.args(["--ignored", "--exact", "reaping_probe", "--nocapture"]);
    let out = c.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}
#[test]
#[ignore = "invoked by isolated wrapper to avoid racing process-wide child inspection"]
fn reaping_probe() {
    let d = TempDir::new();
    d.fixtures();
    let mut env = rsh::env::ShellEnv::inherited().unwrap();
    env.cwd = d.0.clone();
    let executable = std::path::PathBuf::from(env!("CARGO_BIN_EXE_rsh"));
    let mut shell = rsh::exec::Shell::new(env, executable);
    assert_eq!(shell.run_source(b"i=0; while test $i -lt 200; do (exit $((i%10))) & true | true; i=$((i+1)); done; wait"),0);
    assert_eq!(shell.background_count(), 0);
    assert!(!rsh::sys::has_children().unwrap());
    let ps = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,stat="])
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert!(ps.status.success());
    for line in String::from_utf8_lossy(&ps.stdout).lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.get(1).and_then(|s| s.parse::<u32>().ok()) == Some(std::process::id()) {
            assert!(
                !fields.get(2).is_some_and(|s| s.starts_with('Z')),
                "zombie child: {line}"
            );
        }
    }
}
#[test]
fn cooperative_interrupt_and_reference_difference() {
    use std::io::Read;
    use std::time::{Duration, Instant};
    for shell in [env!("CARGO_BIN_EXE_rsh"), "dash", "bash"] {
        if std::process::Command::new(shell)
            .args(["-c", ":"])
            .status()
            .is_err()
        {
            continue;
        }
        for interrupt_child in [false, true] {
            let d = TempDir::new();
            d.fixtures();
            let script = if interrupt_child {
                "/bin/sh -c 'echo $$ > child-pid; exec sleep 100'; echo BAD"
            } else {
                "/bin/sh -c 'trap \"\" INT; echo $$ > child-pid; sleep 0.2; exit 130'; echo survived"
            };
            let mut c = d.command(shell);
            if shell == "bash" {
                c.arg("--posix");
            }
            let mut child = c
                .args(["-c", script])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let start = Instant::now();
            let pid = loop {
                if let Ok(text) = std::fs::read_to_string(d.0.join("child-pid"))
                    && let Ok(pid) = text.trim().parse::<i32>()
                {
                    break pid;
                }
                assert!(start.elapsed() < Duration::from_secs(3));
                std::thread::sleep(Duration::from_millis(2));
            };
            rsh::sys::send_signal(child.id() as i32, rsh::sys::INT).unwrap();
            if interrupt_child {
                rsh::sys::send_signal(pid, rsh::sys::INT).unwrap();
            }
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if start.elapsed() > Duration::from_secs(4) {
                    let _ = rsh::sys::send_signal(pid, 9);
                    child.kill().unwrap();
                    panic!("cooperative wait timed out: {shell}");
                }
                std::thread::sleep(Duration::from_millis(2));
            };
            let mut out = Vec::new();
            child.stdout.take().unwrap().read_to_end(&mut out).unwrap();
            use std::os::unix::process::ExitStatusExt;
            if interrupt_child {
                assert!(out.is_empty(), "{shell}: {out:?}");
                assert_eq!(
                    status
                        .code()
                        .unwrap_or_else(|| 128 + status.signal().unwrap()),
                    130,
                    "{shell}"
                );
            } else if shell == "dash" && !status.success() {
                // dash versions with Debian #683671 use unconditional deferred
                // exit, unlike the explicitly requested cooperative policy.
                assert_eq!(
                    status
                        .code()
                        .unwrap_or_else(|| 128 + status.signal().unwrap()),
                    130
                );
                assert!(out.is_empty());
            } else {
                assert!(status.success(), "{shell}: {status}");
                assert_eq!(out, b"survived\n");
            }
        }
    }
}
#[test]
fn initially_ignored_signals_cannot_be_caught_in_scripts() {
    let d = TempDir::new();
    d.fixtures();
    let mut c = d.command("/bin/sh");
    c.args(["-c","trap '' INT CHLD TSTP; exec \"$@\"","fixture",env!("CARGO_BIN_EXE_rsh"),"-c","trap 'echo BAD' INT CHLD TSTP; kill -s INT $$; kill -s TSTP $$; true; /usr/bin/true; echo alive"]);
    let o = capture(c).0;
    assert_eq!(o.stdout, b"alive\n");
    assert!(o.status.success(), "{:?}", o.stderr);
}
#[test]
fn background_external_pid_matches_dollar_bang() {
    use std::os::unix::fs::PermissionsExt;
    for program in [env!("CARGO_BIN_EXE_rsh"), "dash", "bash"] {
        if std::process::Command::new(program)
            .args(["-c", ":"])
            .status()
            .is_err()
        {
            continue;
        }
        let d = TempDir::new();
        d.fixtures();
        std::fs::write(
            d.0.join("pid-report"),
            b"#!/bin/sh\necho $$ > child-pid\nexec sleep 100\n",
        )
        .unwrap();
        std::fs::set_permissions(
            d.0.join("pid-report"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let mut c = d.command(program);
        if program == "bash" {
            c.arg("--posix");
        }
        c.args(["-c","./pid-report & p=$!; while test ! -s child-pid; do :; done; read actual <child-pid; test \"$p\" -eq \"$actual\"; echo pid:$?; kill -TERM \"$p\"; wait \"$p\" 2>/dev/null; echo status:$?"]);
        let o = capture(c).0;
        assert_eq!(o.stdout, b"pid:0\nstatus:143\n", "{program}");
        assert!(o.stderr.is_empty(), "{program}: {:?}", o.stderr);
    }
}
