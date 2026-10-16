//! Regular builtin; startup invokes this name through ordinary shell dispatch.
use super::Outcome;
use crate::{env::ShellEnv, interactive::sanitize::sanitize, sys};
use std::{
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};
fn user_path(env: &ShellEnv) -> Option<PathBuf> {
    let base = env
        .get(b"XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(|v| PathBuf::from(OsStr::from_bytes(v)))
        .or_else(|| {
            env.get(b"HOME")
                .map(|v| PathBuf::from(OsStr::from_bytes(v)).join(".config"))
        });
    base.map(|p| p.join("rsh/motd"))
}
pub(super) fn run(args: &[Vec<u8>], env: &ShellEnv) -> Outcome {
    if !args.is_empty() {
        return Outcome::error(b"motd", b"unexpected operand", 1);
    }
    let system = PathBuf::from("/etc/motd");
    // This override is absent from every non-test build, including debug binaries.
    #[cfg(test)]
    let system = env
        .get(b"RSH_TEST_SYSTEM_MOTD")
        .map(|v| PathBuf::from(OsStr::from_bytes(v)))
        .unwrap_or(system);
    let paths = user_path(env)
        .into_iter()
        .chain(env.options.login.then_some(system));
    for path in paths {
        match sys::motd_file(&absolute(env, &path)) {
            Ok(None) => continue,
            Ok(Some((bytes, truncated))) => {
                let mut stdout = sanitize(&bytes, sys::terminal(1));
                if truncated {
                    stdout.push(b'\n');
                }
                return Outcome {
                    stdout,
                    ..Outcome::default()
                };
            }
            Err(error) => {
                let mut context = b"motd: ".to_vec();
                context.extend(path.as_os_str().as_bytes());
                return Outcome::error(&context, error.to_string().as_bytes(), 1);
            }
        }
    }
    Outcome::default()
}
fn absolute(env: &ShellEnv, path: &Path) -> PathBuf {
    env.cwd.join(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Fixture, ShellEnv) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rsh-motd-unit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(path.join("config/rsh")).unwrap();
        let mut env = ShellEnv::inherited().unwrap();
        env.cwd = path.clone();
        env.set(b"HOME".to_vec(), path.as_os_str().as_bytes().to_vec());
        env.set(
            b"XDG_CONFIG_HOME".to_vec(),
            path.join("config").as_os_str().as_bytes().to_vec(),
        );
        env.set(
            b"RSH_TEST_SYSTEM_MOTD".to_vec(),
            path.join("system").as_os_str().as_bytes().to_vec(),
        );
        (Fixture(path), env)
    }
    #[test]
    fn system_file_requires_login_and_user_file_has_priority() {
        let (f, mut env) = fixture();
        std::fs::write(f.0.join("system"), b"system\n").unwrap();
        assert!(run(&[], &env).stdout.is_empty());
        env.options.login = true;
        assert_eq!(run(&[], &env).stdout, b"system\n");
        std::fs::write(f.0.join("config/rsh/motd"), b"user\n").unwrap();
        assert_eq!(run(&[], &env).stdout, b"user\n");
        std::fs::remove_file(f.0.join("config/rsh/motd")).unwrap();
        std::fs::remove_file(f.0.join("system")).unwrap();
        let out = run(&[], &env);
        assert_eq!(out.status, 0);
        assert!(out.stdout.is_empty());
        assert!(crate::interactive::builtin_names().contains(&"motd"));
        assert!(matches!(
            super::super::regular(b"motd"),
            Some(super::super::Builtin::Motd)
        ));
        assert!(super::super::special(b"motd").is_none());
    }
    #[test]
    fn empty_user_file_suppresses_system_and_operands_fail() {
        let (f, mut env) = fixture();
        env.options.login = true;
        std::fs::write(f.0.join("system"), b"system\n").unwrap();
        std::fs::write(f.0.join("config/rsh/motd"), b"").unwrap();
        assert!(run(&[], &env).stdout.is_empty());
        assert_eq!(run(&[b"unexpected".to_vec()], &env).status, 1);
    }
    #[test]
    #[ignore = "manual timing reference; no CI timing assertion"]
    fn absent_motd_startup_overhead() {
        use expectrl::{Eof, Expect, Session};
        if std::env::var_os("RSH_MOTD_BENCH_CHILD").is_none() {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--ignored",
                    "--exact",
                    "builtins::motd::tests::absent_motd_startup_overhead",
                    "--nocapture",
                ])
                .env("RSH_MOTD_BENCH_CHILD", "1");
            let mut session = Session::spawn(command).unwrap();
            session.set_expect_timeout(Some(std::time::Duration::from_secs(15)));
            let output = session.expect(Eof).unwrap();
            let text = String::from_utf8_lossy(output.as_bytes());
            assert!(text.contains("complete motd startup hook:"), "{text}");
            eprintln!("{text}");
            return;
        }
        assert!(sys::terminal(0));
        let (_fixture, mut env) = fixture();
        env.options.interactive = true;
        env.options.monitor = true;
        // No inherited user suppression may accidentally turn this into a no-op.
        env.set(b"RSH_NO_MOTD".to_vec(), Vec::new());
        let mut shell = crate::exec::Shell::new(env, std::env::current_exe().unwrap());
        shell.interactive_start();
        let start = std::time::Instant::now();
        for _ in 0..10000 {
            crate::interactive::startup_motd(std::hint::black_box(&mut shell), false);
        }
        eprintln!(
            "complete motd startup hook: {:?} per invocation (10000 iterations, terminal stdin, absent files)",
            start.elapsed() / 10000
        );
    }
}
