use crate::support::TempDir;
use expectrl::session::OsSession;
use expectrl::{ControlCode, Eof, Expect, Regex, Session};
use std::{os::fd::AsRawFd, time::Duration};
struct Ui {
    s: OsSession,
    d: TempDir,
    submitted: bool,
    boundary: bool,
}
fn literal(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if "\\.^$|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
impl Ui {
    fn new() -> Self {
        Self::with_args(&[])
    }
    fn with_args(args: &[&str]) -> Self {
        let d = TempDir::new();
        d.fixtures();
        let mut cmd = d.command(env!("CARGO_BIN_EXE_rsh"));
        cmd.args(["-i"])
            .args(args)
            .env("TERM", "xterm-256color")
            .env("PS1", "UI> ")
            .env("PS2", "MORE> ");
        let mut s = Session::spawn(cmd).unwrap();
        s.set_expect_timeout(Some(Duration::from_secs(10)));
        let mut ui = Self {
            s,
            d,
            submitted: false,
            boundary: false,
        };
        ui.until("UI> ");
        ui.until("\x1b[?25h");
        ui
    }
    fn until(&mut self, text: &str) -> Vec<u8> {
        let pattern = format!("(?s)\\A.*?({}|\\x1b\\[6n)", literal(text));
        let mut output = Vec::new();
        loop {
            let got = match self.s.expect(Regex(&pattern)) {
                Ok(g) => g,
                Err(e) => {
                    let rest = self
                        .s
                        .check(Regex("(?s).+"))
                        .ok()
                        .map(|c| String::from_utf8_lossy(c.as_bytes()).into_owned())
                        .unwrap_or_default();
                    panic!("waiting for {text:?}: {e}; buffered={rest:?}");
                }
            };
            output.extend(got.as_bytes());
            if got.as_bytes().windows(8).any(|s| s == b"\x1b[?2004l") {
                self.boundary = true;
            }
            if got.get(1) == Some(b"\x1b[6n".as_slice()) {
                self.s.send("\x1b[1;1R").unwrap();
            } else {
                return output;
            }
        }
    }
    fn line(&mut self, s: &str) {
        self.submitted = true;
        self.boundary = false;
        self.s.send_line(s).unwrap();
    }
    fn prompt(&mut self) -> Vec<u8> {
        if self.submitted && !self.boundary {
            self.until("\x1b[?2004l");
        }
        self.submitted = false;
        let mut out = self.until("UI> ");
        out.extend(self.until("\x1b[?25h"));
        out
    }
    fn send<B: AsRef<[u8]>>(&mut self, b: B) -> Result<(), expectrl::Error> {
        let b = b.as_ref();
        if b.contains(&b'\r') || ((b.contains(&3) || b.contains(&4)) && !self.submitted) {
            self.boundary = false;
        }
        if b.contains(&b'\r') || b.contains(&3) || b.contains(&4) {
            self.submitted = true;
        }
        self.s.send(b)
    }
    fn foreground(&mut self) {
        self.until("\x1b[?2004l");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let group = rsh::sys::current_foreground(self.s.as_raw_fd()).unwrap();
            if group > 0 && group != self.s.get_process().pid().as_raw() {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "foreground job was not launched"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn close(&mut self) {
        self.line("exit");
        self.s.expect(Eof).unwrap();
    }
}
impl Drop for Ui {
    fn drop(&mut self) {
        if let Ok(g) = rsh::sys::current_foreground(self.s.as_raw_fd())
            && g > 0
            && g != self.s.get_process().pid().as_raw()
        {
            let _ = rsh::sys::send_signal(-g, 9);
        }
        let _ = self.s.send_line("kill -KILL %+; exit; exit");
    }
}
#[test]
fn prompt_expansion_status_multiline_ctrl_c_and_eof() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ui = Ui::new();
    ui.line("false");
    let out = ui.prompt();
    assert!(
        out.windows(4).any(|w| w == b"\x1b[31") || out.windows(7).any(|w| w == b"38;5;9m"),
        "{out:?}"
    );
    ui.line("PS1='$(printf UI)> '; PS2='${HOME}> '");
    ui.prompt();
    ui.line("if true; then");
    ui.until(&format!("{}> ", ui.d.0.join("home").display()));
    ui.line(":; fi");
    ui.prompt();
    ui.line("PS2='MORE> '");
    ui.prompt();
    ui.line("if true; then");
    ui.until("MORE> ");
    ui.line("printf 'MULTI-DONE\\n'");
    ui.until("MORE> ");
    ui.line("fi");
    ui.until("MULTI-DONE\r\n");
    ui.prompt();
    ui.send("echo MUST-NOT-RUN").unwrap();
    ui.send(ControlCode::ETX).unwrap();
    ui.prompt();
    ui.line("printf 'status=%s\\n' $?");
    ui.until("status=130\r\n");
    ui.prompt();
    ui.line("echo )");
    ui.until("syntax error");
    ui.prompt();
    ui.line("printf 'parse=%s\\n' $?");
    ui.until("parse=2\r\n");
    ui.prompt();
    ui.send(ControlCode::EOT).unwrap();
    ui.s.expect(Eof).unwrap();
}
#[test]
fn history_arrows_reverse_search_hints_and_space_exclusion() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ui = Ui::new();
    ui.line("printf 'HISTORY-ONE\\n'");
    ui.until("HISTORY-ONE\r\n");
    ui.prompt();
    ui.send("\x1b[A\r").unwrap();
    ui.until("HISTORY-ONE\r\n");
    ui.prompt();
    ui.send("printf").unwrap();
    ui.until("HISTORY-ONE");
    ui.until("\x1b[?25h");
    ui.send("\x1b[C\r").unwrap();
    ui.until("HISTORY-ONE\r\n");
    ui.prompt();
    ui.send(ControlCode::DC2).unwrap();
    ui.send("HISTORY-ONE").unwrap();
    ui.until("\x1b[?25h");
    ui.send("\r").unwrap();
    ui.until("\x1b[?25h");
    ui.send("\r").unwrap();
    ui.until("HISTORY-ONE\r\n");
    ui.prompt();
    ui.line(" echo SPACE-HIDDEN");
    ui.until("SPACE-HIDDEN\r\n");
    ui.prompt();
    ui.line("HISTFILE=; history | cat >history-pipe");
    ui.prompt();
    assert!(
        std::fs::read_to_string(ui.d.0.join("history-pipe"))
            .unwrap()
            .contains("printf 'HISTORY-ONE")
    );
    ui.line("(history -c); history | cat >history-parent");
    ui.prompt();
    assert!(
        std::fs::read_to_string(ui.d.0.join("history-parent"))
            .unwrap()
            .contains("printf 'HISTORY-ONE")
    );
    ui.close();
    let entries = rsh::interactive::history::decode(
        &std::fs::read(ui.d.0.join("home/.local/share/rsh/history")).unwrap(),
    );
    assert!(!entries.iter().any(|e| e.command.contains("SPACE-HIDDEN")));
    assert!(entries.iter().any(|e| e.command.contains("HISTORY-ONE")));
}
#[test]
fn completions_variables_files_directories_and_menu() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ui = Ui::new();
    std::fs::write(ui.d.0.join("uniquefile"), b"FILE-CONTENT\n").unwrap();
    std::fs::write(ui.d.0.join("alpha-one"), b"a").unwrap();
    std::fs::write(ui.d.0.join("alpha-two"), b"b").unwrap();
    std::fs::create_dir(ui.d.0.join("only-dir")).unwrap();
    std::fs::write(ui.d.0.join("only-file"), b"x").unwrap();
    ui.send("cat uniq\t\r").unwrap();
    ui.until("FILE-CONTENT\r\n");
    ui.prompt();
    ui.send("printf '%s\\n' $HO\t\r").unwrap();
    let home = ui.d.0.join("home").display().to_string();
    ui.until(&format!("{home}\r\n"));
    ui.prompt();
    ui.send("cat alpha-\t").unwrap();
    ui.until("one");
    ui.until("\x1b[?25h");
    ui.send(ControlCode::ETX).unwrap();
    ui.prompt();
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir(ui.d.0.join("bin")).unwrap();
    let exe = ui.d.0.join("bin/zzrshm9unique");
    std::fs::write(&exe, b"#!/bin/sh\nprintf 'PATH-COMPLETE\\n'\n").unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    ui.line(&format!("PATH={}/bin:/usr/bin:/bin", ui.d.0.display()));
    ui.prompt();
    // PATH scans are asynchronous and bounded, independent of key latency.
    std::thread::sleep(Duration::from_millis(100));
    ui.send("zzrshm9u\t\r").unwrap();
    ui.until("PATH-COMPLETE\r\n");
    ui.prompt();
    ui.send("cd only-\t\r").unwrap();
    ui.prompt();
    ui.line("pwd");
    ui.until("only-dir\r\n");
    ui.prompt();
    ui.close();
}
#[test]
fn bracketed_paste_requires_enter_and_multiline_history() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ui = Ui::new();
    ui.send("\x1b[200~printf 'PASTE-A\\n'\nprintf 'PASTE-B\\n'\x1b[201~")
        .unwrap();
    // No execution before Enter: output is redirected in the source to inspect state.
    ui.send(ControlCode::ETX).unwrap();
    ui.prompt();
    assert!(!ui.d.0.join("pasted").exists());
    ui.send("\x1b[200~printf a >pasted\nprintf b >>pasted\x1b[201~")
        .unwrap();
    std::thread::sleep(Duration::from_millis(80));
    assert!(!ui.d.0.join("pasted").exists());
    ui.send("\r").unwrap();
    ui.prompt();
    assert_eq!(std::fs::read(ui.d.0.join("pasted")).unwrap(), b"ab");
    ui.send("\x1b[A\r").unwrap();
    ui.prompt();
    assert_eq!(std::fs::read(ui.d.0.join("pasted")).unwrap(), b"ab");
    ui.close();
    let entries = rsh::interactive::history::decode(
        &std::fs::read(ui.d.0.join("home/.local/share/rsh/history")).unwrap(),
    );
    assert!(entries.iter().any(|e| e.command.contains('\n')));
}

#[test]
fn resize_jobs_and_editor_survive_foreground_interrupt() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ui = Ui::new();
    rsh::sys::resize_terminal(ui.s.as_raw_fd(), 100, 32).unwrap();
    ui.until("\x1b[?25h");
    std::thread::sleep(Duration::from_millis(150));
    ui.line("printf 'SIZE=%s,%s\\n' $COLUMNS $LINES");
    ui.until("SIZE=100,32\r\n");
    ui.prompt();
    ui.line("sleep 100");
    ui.foreground();
    ui.send(ControlCode::SUB).unwrap();
    ui.until("Stopped");
    ui.prompt();
    ui.line("jobs");
    ui.until("Stopped");
    ui.prompt();
    ui.send(ControlCode::EOT).unwrap();
    ui.until("There are stopped jobs.");
    ui.prompt();
    ui.line("fg");
    ui.foreground();
    ui.send(ControlCode::ETX).unwrap();
    ui.prompt();
    ui.line("exec 3>editor-fd; printf FD-OK >&3; exec 3>&-; exec 10>editor-fd10; printf TEN >&10; exec 10>&-; cat editor-fd; echo");
    ui.until("FD-OK\r\n");
    ui.prompt();
    #[cfg(target_os = "linux")]
    {
        ui.line("ls /proc/self/fd >child-fds");
        ui.prompt();
        let fds = std::fs::read_to_string(ui.d.0.join("child-fds")).unwrap();
        assert!(
            fds.lines().all(|s| s.parse::<i32>().unwrap() <= 3),
            "editor fd leaked: {fds}"
        );
    }
    ui.line("printf 'EDITOR-ALIVE\\n'");
    ui.until("EDITOR-ALIVE\r\n");
    ui.prompt();
    ui.close();
}

#[test]
fn two_shells_merge_history_and_history_builtin_clears_it() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Dumb terminals exercise the same store with two real shell processes,
    // without needing two concurrent ANSI terminal emulators.
    let d = TempDir::new();
    d.fixtures();
    let path = d.0.join("shared-history");
    let spawn = || {
        let mut cmd = d.command(env!("CARGO_BIN_EXE_rsh"));
        cmd.args(["-i"])
            .env("TERM", "dumb")
            .env("PS1", "PLAIN> ")
            .env("HISTFILE", &path);
        let mut s = Session::spawn(cmd).unwrap();
        s.set_expect_timeout(Some(Duration::from_secs(10)));
        s.expect("PLAIN> ").unwrap();
        s
    };
    let mut a = spawn();
    let mut b = spawn();
    for i in 0..30 {
        a.send_line(format!("echo A-{i}")).unwrap();
        b.send_line(format!("echo B-{i}")).unwrap();
        a.expect(format!("A-{i}\r\n")).unwrap();
        a.expect("PLAIN> ").unwrap();
        b.expect(format!("B-{i}\r\n")).unwrap();
        b.expect("PLAIN> ").unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    let entries = rsh::interactive::history::decode(&bytes);
    assert_eq!(bytes.iter().filter(|b| **b == b'\n').count(), entries.len());
    assert_eq!(entries.len(), 60);
    for i in 0..30 {
        assert!(entries.iter().any(|e| e.command == format!("echo A-{i}")));
        assert!(entries.iter().any(|e| e.command == format!("echo B-{i}")));
    }
    a.send_line("printf '%s\\n' 'é'").unwrap();
    a.expect("é\r\n").unwrap();
    a.expect("PLAIN> ").unwrap();
    a.send_line("history").unwrap();
    a.expect("echo A-29").unwrap();
    a.expect("PLAIN> ").unwrap();
    a.send_line("history -c").unwrap();
    a.expect("PLAIN> ").unwrap();
    assert!(std::fs::read(&path).unwrap().is_empty());
    a.send_line("exit").unwrap();
    b.send_line("exit").unwrap();
    a.expect(Eof).unwrap();
    b.expect(Eof).unwrap();
}

#[test]
fn panic_hook_restores_terminal_modes() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let d = TempDir::new();
    d.fixtures();
    let mut command = d.command(std::env::current_exe().unwrap().to_str().unwrap());
    command
        .args([
            "--ignored",
            "--exact",
            "interactive::editor_panic_helper",
            "--nocapture",
        ])
        .env("TERM", "xterm-256color");
    let mut session = Session::spawn(command).unwrap();
    session.set_expect_timeout(Some(Duration::from_secs(10)));
    loop {
        let m = session
            .expect(Regex("(?s)\\A.*?(\\x1b\\[6n|EDITOR-PANIC-PROBE)"))
            .unwrap();
        if m.get(1) == Some(b"\x1b[6n".as_slice()) {
            session.send("\x1b[1;1R").unwrap();
        } else {
            break;
        }
    }
    session.expect(Eof).unwrap();
    assert_eq!(
        rsh::sys::terminal_modes(session.as_raw_fd())
            .unwrap()
            .echo_and_canonical(),
        (false, true)
    );
}

#[test]
#[ignore = "isolated panic-recovery probe, invoked by panic_hook_restores_terminal_modes"]
fn editor_panic_helper() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    struct Broken;
    impl reedline::Highlighter for Broken {
        fn highlight(&self, _: &str, _: usize) -> reedline::StyledText {
            panic!("EDITOR-PANIC-PROBE");
        }
    }
    let _recovery = rsh::sys::terminal_recovery(true).unwrap();
    let mut editor = reedline::Reedline::create().with_highlighter(Box::new(Broken));
    let _ = editor.read_line(&reedline::DefaultPrompt::default());
}

#[test]
fn vi_option_noexec_exception_and_interactive_lineno() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ui = Ui::with_args(&["--vi"]);
    ui.line("echo VI-INSERT");
    ui.until("VI-INSERT\r\n");
    ui.prompt();
    ui.line("set +o vi");
    ui.prompt();
    ui.line("printf 'LINE=%s\\n' $LINENO");
    ui.until("LINE=3\r\n");
    ui.prompt();
    ui.close();
    let mut ui = Ui::with_args(&["-n"]);
    ui.line("echo INTERACTIVE-NOEXEC-IGNORED");
    ui.until("INTERACTIVE-NOEXEC-IGNORED\r\n");
    ui.prompt();
    ui.close();
}

#[test]
fn idle_background_notification_redraws_prompt_without_input() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ui = Ui::new();
    ui.line("sleep 0.3 &");
    ui.until("[1]");
    ui.prompt();
    // No Enter is sent: the idle callback must reap and notify through the
    // external printer without corrupting the editable prompt.
    ui.until("Done");
    ui.until("UI> ");
    ui.until("\x1b[?25h");
    ui.line("echo AFTER-NOTICE");
    ui.until("AFTER-NOTICE\r\n");
    ui.prompt();
    ui.close();
}
