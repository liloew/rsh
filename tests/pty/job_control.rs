use crate::support::TempDir;
use expectrl::session::OsSession;
use expectrl::{ControlCode, Eof, Expect, Regex, Session};

use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};
struct Shell {
    session: OsSession,
    _dir: TempDir,
}
impl Shell {
    fn new() -> Self {
        Self::start(true)
    }
    fn start(explicit: bool) -> Self {
        let d = TempDir::new();
        d.fixtures();
        let mut c = d.command(env!("CARGO_BIN_EXE_rsh"));
        if explicit {
            c.arg("-i");
        }
        c.env("PS1", "rsh-test> ");
        let mut session = Session::spawn(c).unwrap();
        session.set_expect_timeout(Some(Duration::from_secs(8)));
        session.expect("rsh-test> ").unwrap();
        Self { session, _dir: d }
    }
    fn line(&mut self, s: &str) {
        self.session.send_line(s).unwrap();
    }
    fn prompt(&mut self) {
        self.session.expect("rsh-test> ").unwrap();
    }
    fn expect(&mut self, s: &str) {
        self.session.expect(s).unwrap();
    }
    fn foreground(&self) -> i32 {
        let start = Instant::now();
        loop {
            let group = rsh::sys::current_foreground(self.session.as_raw_fd()).unwrap();
            if group != self.session.get_process().pid().as_raw() {
                return group;
            }
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "job did not get terminal"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn processes(group: i32) -> Vec<Vec<String>> {
        let output = std::process::Command::new("ps")
            .args(["-axo", "pid=,pgid=,stat=,comm="])
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|l| l.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
            .filter(|f| f.get(1).and_then(|s| s.parse::<i32>().ok()) == Some(group))
            .collect()
    }
    fn sleepers(&self, group: i32, count: usize) {
        let start = Instant::now();
        loop {
            let processes = Self::processes(group);
            if processes.len() == count
                && processes.iter().all(|f| {
                    f.last()
                        .is_some_and(|s| s.rsplit('/').next() == Some("sleep"))
                })
            {
                return;
            }
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "sleep pipeline did not exec: {processes:?}"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn control(&mut self, c: ControlCode) {
        self.session.send(c).unwrap();
    }
}
impl Drop for Shell {
    fn drop(&mut self) {
        if let Ok(group) = rsh::sys::current_foreground(self.session.as_raw_fd())
            && group > 0
            && group != self.session.get_process().pid().as_raw()
        {
            let _ = rsh::sys::send_signal(-group, 9);
        }
        let _ = self
            .session
            .send_line("kill -KILL %+; kill -KILL %+; exit; exit");
    }
}
// Observed bash interaction: Ctrl-Z retains a stopped job, bg resumes without
// owning the terminal, fg transfers it back, Ctrl-C returns to the prompt.
// Poll terminal ownership before control characters rather than relying on a
// scheduler delay. exec retains the shell PID/group and uses a short settling delay.
#[test]
fn stop_bg_fg_interrupt_and_terminal_restore() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut sh = Shell::new();
    sh.line("exec 3>fd3; true; exec 3>&-; exec 10>fd10; true; exec 10>&-");
    sh.prompt();
    sh.line("sleep 100");
    let group = sh.foreground();
    sh.sleepers(group, 1);
    sh.control(ControlCode::SUB);
    sh.expect("Stopped");
    sh.prompt();
    sh.line("jobs -l");
    sh.session
        .expect(Regex(r"\[1\]\+.*Stopped.*sleep 100"))
        .unwrap();
    sh.prompt();
    sh.line("wait %1; jobs");
    sh.expect("Stopped");
    sh.prompt();
    sh.line("bg %1");
    sh.expect("Running");
    sh.prompt();
    sh.line("fg %+ ");
    sh.expect("sleep 100");
    sh.foreground();
    sh.control(ControlCode::ETX);
    sh.prompt();
    sh.line("printf 'alive:%s\\n' $?");
    sh.expect("alive:130");
    sh.prompt();
    sh.line("stty -a");
    let baseline = sh.session.expect("rsh-test> ").unwrap();
    let baseline = String::from_utf8_lossy(baseline.before()).contains(" -echo ");
    sh.line("/bin/sh -c 'stty -echo; kill -STOP $$; sleep 100'");
    sh.expect("Stopped");
    sh.prompt();
    sh.line("stty -a");
    let capture = sh.session.expect("rsh-test> ").unwrap();
    let text = String::from_utf8_lossy(capture.before());
    assert_eq!(text.contains(" -echo "), baseline, "{text}");
    sh.line("bg %1; kill -KILL %1; wait; exit");
    if let Err(e) = sh.session.expect(Eof) {
        let remaining = sh.session.expect(Regex(r"(?s).+")).unwrap();
        panic!(
            "EOF failed: {e}: {}",
            String::from_utf8_lossy(remaining.as_bytes())
        );
    }
}
#[test]
fn plain_prompt_ignores_empty_chld_but_reports_completion() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut sh = Shell::new();
    rsh::sys::send_signal(
        sh.session.get_process().pid().as_raw(),
        rsh::sys::signal_number(b"CHLD").unwrap(),
    )
    .unwrap();
    sh.session
        .set_expect_timeout(Some(Duration::from_millis(300)));
    assert!(matches!(
        sh.session.expect("rsh-test> "),
        Err(expectrl::Error::ExpectTimeout)
    ));
    sh.session.set_expect_timeout(Some(Duration::from_secs(8)));
    sh.line("sleep 0.3 &");
    sh.session.expect(Regex(r"\[1\] [0-9]+")).unwrap();
    sh.prompt();
    sh.expect("Done");
    sh.prompt();
    sh.line("exit");
    sh.session.expect(Eof).unwrap();
}

#[test]
fn background_markers_specs_wait_and_disown() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut sh = Shell::new();
    sh.line("sleep 100 &");
    sh.session.expect(Regex(r"\[1\] [0-9]+")).unwrap();
    sh.prompt();
    sh.line("sleep 101 &");
    sh.session.expect(Regex(r"\[2\] [0-9]+")).unwrap();
    sh.prompt();
    sh.line("jobs");
    // expectrl can read both rows at once; never let the first match consume
    // the second row's Running marker (notably with macOS PTY buffering).
    sh.session.expect(Regex(r"\[1\]-[^\r\n]*Running")).unwrap();
    sh.session.expect(Regex(r"\[2\]\+[^\r\n]*Running")).unwrap();
    sh.prompt();
    sh.line("kill %sleep");
    sh.expect("no such job");
    sh.prompt();
    sh.line("kill %-; wait %1; echo waited:$?");
    sh.expect("waited:143");
    sh.prompt();
    sh.line("kill %?101; wait %+; echo waited:$?");
    sh.expect("waited:143");
    sh.prompt();
    sh.line("sleep 100 &");
    sh.prompt();
    sh.line("kill %1");
    sh.expect("Terminated");
    sh.prompt();
    sh.line("sleep 100 &");
    sh.prompt();
    sh.line("disown -h %sleep; jobs -p");
    sh.session.expect(Regex(r"[0-9]+\r\n")).unwrap();
    sh.prompt();
    sh.line("kill %sleep; wait; exit");
    sh.session.expect(Eof).unwrap();
}
#[test]
fn pipeline_group_and_stopped_exit_warning() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut sh = Shell::new();
    sh.line("sleep 100 | sleep 100");
    let group = sh.foreground();
    sh.sleepers(group, 2);
    sh.control(ControlCode::SUB);
    sh.expect("Stopped");
    sh.prompt();
    let processes = Shell::processes(group);
    assert_eq!(processes.len(), 2, "{processes:?}");
    assert!(
        processes
            .iter()
            .all(|p| p.get(2).is_some_and(|s| s.starts_with('T'))),
        "{processes:?}"
    );
    sh.line("x=$(printf value); jobs -p >pgid; echo CHECK-DONE");
    sh.expect("CHECK-DONE");
    sh.prompt();
    assert_eq!(
        std::fs::read_to_string(sh._dir.0.join("pgid"))
            .unwrap()
            .trim(),
        group.to_string()
    );
    let processes = Shell::processes(group);
    assert!(
        processes
            .iter()
            .all(|p| p.get(2).is_some_and(|s| s.starts_with('T'))),
        "substitution resumed a job: {processes:?}"
    );
    sh.line("exit");
    sh.expect("There are stopped jobs.");
    sh.prompt();
    sh.line("exit");
    sh.session.expect(Eof).unwrap();
}
#[test]
fn builtin_function_pipelines_and_exec_signals() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut sh = Shell::new();
    sh.line("f() { sleep 100; }; f | cat");
    sh.foreground();
    sh.control(ControlCode::ETX);
    sh.prompt();
    sh.line("yes | head -1");
    sh.expect("y\r\n");
    sh.prompt();
    sh.line("printf x | cat");
    sh.expect("x");
    sh.prompt();
    sh.line("sleep 100 | read x");
    sh.foreground();
    sh.control(ControlCode::ETX);
    sh.prompt();
    sh.line("printf x | f");
    sh.foreground();
    sh.control(ControlCode::ETX);
    sh.prompt();
    sh.line("/bin/sh -c 'kill -TERM $$'");
    sh.expect("Terminated");
    sh.prompt();
    sh.line("exec sleep 100");
    std::thread::sleep(Duration::from_millis(80));
    sh.control(ControlCode::ETX);
    sh.session.expect(Eof).unwrap();
}

#[test]
fn automatic_mode_subshells_substitutions_and_ttin() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut sh = Shell::start(false);
    sh.line("case $- in *im*|*mi*) echo monitored;; esac");
    sh.expect("monitored");
    sh.prompt();
    sh.line("cat &");
    sh.expect("Stopped");
    sh.prompt();
    sh.line("(jobs; case $- in *m*) echo BAD;; *) echo isolated;; esac)");
    sh.expect("isolated");
    sh.prompt();
    sh.line("x=$(jobs); printf 'sub:<%s>\\n' \"$x\"");
    sh.expect("sub:<>");
    sh.prompt();
    sh.line("kill %1; bg %1; wait %1");
    sh.prompt();
    sh.line("set +m; case $- in *m*) echo BAD;; *) echo disabled;; esac");
    sh.expect("disabled");
    sh.prompt();
    sh.line("set -m; case $- in *m*) echo enabled;; esac");
    sh.expect("enabled");
    sh.prompt();
    sh.line("exit");
    sh.session.expect(Eof).unwrap();
}
#[test]
fn pipeline_interrupt_leaves_no_group_members() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut sh = Shell::new();
    sh.line("sleep 100 | sleep 100");
    let group = sh.foreground();
    sh.control(ControlCode::ETX);
    sh.prompt();
    let start = Instant::now();
    while rsh::sys::send_signal(-group, 0).is_ok() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "pipeline group survived Ctrl-C"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    sh.line("exit");
    sh.session.expect(Eof).unwrap();
}

#[test]
fn disown_exempts_hangup_and_hangup_continues_stopped_jobs() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut sh = Shell::new();
    sh.line("sleep 100 &");
    sh.prompt();
    sh.line("jobs -p > group");
    sh.prompt();
    let group = std::fs::read_to_string(sh._dir.0.join("group"))
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    struct Group(i32);
    impl Drop for Group {
        fn drop(&mut self) {
            let _ = rsh::sys::send_signal(-self.0, 9);
        }
    }
    let _cleanup = Group(group);
    sh.line("disown -a; jobs; echo cleared");
    sh.expect("cleared");
    sh.prompt();
    sh.line("exit");
    sh.session.expect(Eof).unwrap();
    assert!(rsh::sys::send_signal(-group, 0).is_ok());
    let mut sh = Shell::new();
    let helper = std::env::current_exe().unwrap();
    sh.line(&format!(
        "'{}' --ignored --exact job_control::hangup_probe --test-threads=1",
        helper.display()
    ));
    let group = sh.foreground();
    // Do not let the helper stop and return the terminal before foreground()
    // has observed the handoff, even if the test process is descheduled.
    std::fs::write(sh._dir.0.join("stop-ready"), b"stop\n").unwrap();
    sh.expect("Stopped");
    sh.prompt();
    sh.line("exit");
    sh.expect("There are stopped jobs.");
    sh.prompt();
    sh.line("exit");
    sh.session.expect(Eof).unwrap();
    // The HUP action requires CONT to run after a group stop.
    let _cleanup = Group(group);
    let start = Instant::now();
    while !sh._dir.0.join("hup").exists() {
        assert!(start.elapsed() < Duration::from_secs(4));
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
#[ignore = "isolated helper executed as the foreground PTY job by the HUP test"]
fn hangup_probe() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    rsh::sys::trap_signal(rsh::sys::HUP, Some(b"record")).unwrap();
    let start = Instant::now();
    while !std::path::Path::new("stop-ready").exists() {
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "parent did not acknowledge foreground handoff"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    rsh::sys::send_signal(
        std::process::id() as i32,
        rsh::sys::signal_number(b"STOP").unwrap(),
    )
    .unwrap();
    let start = Instant::now();
    while rsh::sys::pending_signal_mask() & (1u64 << rsh::sys::HUP) == 0 {
        assert!(start.elapsed() < Duration::from_secs(4));
        std::thread::sleep(Duration::from_millis(2));
    }
    std::fs::write("hup", b"received after resume\n").unwrap();
}
#[test]
fn monitored_race_200_jobs_retains_every_status() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut sh = Shell::new();
    sh.session.set_expect_timeout(Some(Duration::from_secs(20)));
    sh.line("pids=; i=0; while test $i -lt 200; do (exit $((i%20))) & pids=\"$pids $!\"; i=$((i+1)); done; for p in $pids; do wait $p; printf 'status:%s\\n' $?; done; echo RACE-DONE");
    let captured = sh.session.expect("RACE-DONE").unwrap();
    let text = String::from_utf8_lossy(captured.before());
    let statuses: Vec<_> = text
        .lines()
        .filter_map(|l| l.strip_prefix("status:"))
        .map(|s| s.trim().parse::<u8>().unwrap())
        .collect();
    assert_eq!(
        statuses,
        (0..200).map(|i| (i % 20) as u8).collect::<Vec<_>>()
    );
    sh.prompt();
    sh.line("exit");
    sh.session.expect(Eof).unwrap();
}
