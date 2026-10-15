#![allow(dead_code)]
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub struct TempDir(pub PathBuf);
impl TempDir {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "rsh-m3-case-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("temp dir: {e}"),
            }
        }
    }
    pub fn fixtures(&self) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir(self.0.join("home")).unwrap();
        std::fs::create_dir(self.0.join("child")).unwrap();
        std::fs::write(self.0.join("blocked"), b"not executable\n").unwrap();
        std::fs::set_permissions(
            self.0.join("blocked"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        // Only tests use another shell: a real external program terminated by SIGTERM.
        std::fs::write(self.0.join("term"), b"#!/bin/sh\nkill -TERM $$\n").unwrap();
        std::fs::set_permissions(self.0.join("term"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
    }
    pub fn command(&self, program: &str) -> Command {
        let mut cmd = Command::new(program);
        cmd.current_dir(&self.0)
            .env_clear()
            .env("LC_ALL", "C")
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", self.0.join("home"))
            .env("PWD", &self.0);
        cmd
    }
    pub fn normalize(&self, bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).replace(self.0.to_string_lossy().as_ref(), "<TMP>")
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn capture(mut command: Command) -> (Output, Duration) {
    let start = Instant::now();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(5) {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("command did not terminate within five seconds");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let elapsed = start.elapsed();
    (
        Output {
            status,
            stdout: out.join().unwrap(),
            stderr: err.join().unwrap(),
        },
        elapsed,
    )
}

pub fn normalize_stdout(dir: &TempDir, name: &str, bytes: &[u8]) -> String {
    let output = dir.normalize(bytes);
    if name == "assignment-pipeline" {
        // env variable enumeration order is unspecified; preserve all lines/values.
        let mut lines: Vec<_> = output.lines().collect();
        lines.sort();
        format!("{}\n", lines.join("\n"))
    } else {
        output
    }
}
pub fn normalize_stderr(dir: &TempDir, name: &str, bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    // Shell-specific prefixes, line numbers and signal messages are presentation.
    match name {
        "missing" => {
            assert!(String::from_utf8_lossy(bytes).contains("not found"));
            "not found\n".into()
        }
        "nonexecutable" => {
            assert!(
                String::from_utf8_lossy(bytes)
                    .to_lowercase()
                    .contains("permission denied")
            );
            "permission denied\n".into()
        }
        "signal" => {
            let text = String::from_utf8_lossy(bytes);
            assert!(text.contains("Terminated"));
            String::new()
        }
        _ => dir.normalize(bytes),
    }
}
