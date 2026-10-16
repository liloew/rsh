//! Script fixtures run with -i and a PTY stdin. The line editor itself is
//! covered by interactive.rs; monitor is disabled for script-output parity.
use crate::support::{TempDir, capture, normalize_stdout};
use expectrl::{Eof, Expect, Session};
use std::path::Path;
fn fixture() -> TempDir {
    use std::os::unix::fs::PermissionsExt;
    let d = TempDir::new();
    d.fixtures();
    std::fs::create_dir_all(d.0.join("search/nested")).unwrap();
    std::os::unix::fs::symlink("child", d.0.join("link")).unwrap();
    std::fs::write(
        d.0.join("source-file"),
        b"printf 'source:%s:%s\\n' \"$#\" \"$1\"; set -- modified; return 4; echo BAD\n",
    )
    .unwrap();
    for f in ["a.txt", "b.txt", "1.txt", ".hidden.txt"] {
        std::fs::write(d.0.join(f), b"").unwrap();
    }
    std::fs::write(
        d.0.join("both"),
        b"#!/bin/sh\nprintf 'out\\n'\nprintf 'err\\n' >&2\n",
    )
    .unwrap();
    std::fs::set_permissions(d.0.join("both"), std::fs::Permissions::from_mode(0o755)).unwrap();
    d
}
fn normalize(d: &TempDir, name: &str, bytes: &[u8]) -> String {
    let text = normalize_stdout(d, name, bytes).replace("/private<TMP>", "<TMP>");
    if name == "times" {
        return format!("{} timing rows", text.lines().count());
    }
    // Interactive mode intentionally adds i to $-. Other option flags agree.
    text.lines()
        .map(|s| {
            if !s.is_empty() && s.bytes().all(|c| b"efunCxilm".contains(&c)) {
                s.replace('i', "")
            } else {
                s.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn m3_through_m8_script_fixtures_with_interactive_pty_stdin() {
    let _pty = crate::PTY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for tree in ["golden", "diff"] {
        for group in ["exec", "redirect", "expand", "control", "builtins", "jobs"] {
            let Ok(entries) = std::fs::read_dir(root.join("tests").join(tree).join(group)) else {
                continue;
            };
            for file in entries
                .map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|e| e == "sh"))
            {
                let source = format!(
                    "{}\n_M9_STATUS=$?; wait; exit \"$_M9_STATUS\"",
                    std::fs::read_to_string(&file).unwrap()
                );
                let name = file.file_stem().unwrap().to_str().unwrap();
                let a = fixture();
                let b = fixture();
                let mut baseline = a.command(env!("CARGO_BIN_EXE_rsh"));
                baseline.args(["-c", &source, "fixture"]);
                let expected = capture(baseline).0;
                let driver = "\"$@\" >pty.out 2>pty.err; printf '%s' $? >pty.status";
                let mut cmd = b.command(env!("CARGO_BIN_EXE_rsh"));
                cmd.args([
                    "-c",
                    driver,
                    "driver",
                    env!("CARGO_BIN_EXE_rsh"),
                    "-i",
                    "-c",
                    &format!("set +m; {source}"),
                    "fixture",
                ]);
                let mut session = Session::spawn(cmd).unwrap();
                session.set_expect_timeout(Some(std::time::Duration::from_secs(10)));
                session
                    .expect(Eof)
                    .unwrap_or_else(|e| panic!("{file:?}: {e}"));
                let actual = std::fs::read(b.0.join("pty.out")).unwrap();
                let err = std::fs::read(b.0.join("pty.err")).unwrap();
                let status = std::fs::read_to_string(b.0.join("pty.status")).unwrap();
                assert_eq!(
                    status.parse::<i32>().unwrap(),
                    expected.status.code().unwrap(),
                    "{file:?}: status {err:?}"
                );
                assert_eq!(
                    normalize(&b, name, &actual),
                    normalize(&a, name, &expected.stdout),
                    "{file:?}: stdout"
                );
                assert_eq!(
                    normalize(&b, name, &err),
                    normalize(&a, name, &expected.stderr),
                    "{file:?}: stderr"
                );
                count += 1;
            }
        }
    }
    assert!(count > 100, "fixture corpus unexpectedly small: {count}");
    eprintln!("{count} existing fixtures passed with -i and PTY stdin");
}
