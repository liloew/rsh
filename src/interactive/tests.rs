use super::*;
#[path = "../../tests/support/mod.rs"]
mod support;
use support::TempDir;
fn env(d: &TempDir) -> crate::env::ShellEnv {
    let mut e = crate::env::ShellEnv::inherited().unwrap();
    e.variables.clear();
    e.cwd = d.0.clone();
    e.set(b"HOME".to_vec(), d.0.as_os_str().as_bytes().to_vec());
    e
}
#[test]
fn continuation_and_error_classification() {
    assert_eq!(classify("if true; then"), InputState::Incomplete);
    assert_eq!(classify("echo 'open"), InputState::Incomplete);
    assert_eq!(classify("if true; then :; fi"), InputState::Complete);
    assert_eq!(classify("echo )"), InputState::Syntax);
}
#[test]
fn lexer_highlight_mappings_and_arbitrary_incomplete_inputs() {
    use highlight::Kind::*;
    let line = "echo 'hi' $HOME >file # comment";
    let k = highlight::classify(line, |s| s == "echo");
    assert_eq!(k.first(), Some(&Command));
    assert_eq!(k.get(5), Some(&String));
    assert_eq!(k.get(10), Some(&Expansion));
    assert!(
        highlight::classify("echo 'a\\' $HOME", |_| true)
            .get(10..15)
            .unwrap()
            .iter()
            .all(|k| *k == Expansion)
    );
    assert!(k.contains(&Redirect));
    assert!(
        highlight::classify("echo `printf hi`", |_| true)
            .get(5..16)
            .unwrap()
            .iter()
            .all(|k| *k == Expansion)
    );
    assert_eq!(k.last(), Some(&Comment));
    assert_eq!(
        highlight::classify("unknown", |_| false).first(),
        Some(&Unknown)
    );
    assert!(highlight::classify("echo 'open", |_| true).contains(&Incomplete));
    for s in [
        "",
        "$(",
        "${",
        "`",
        "if",
        "\"a$(echo \")\")\"",
        "écho $变量",
        "a#b",
    ] {
        assert_eq!(highlight::classify(s, |_| false).len(), s.len());
    }
}
#[test]
fn suggestions_prefer_cwd_and_recent_prefix() {
    let e = |command: &str, cwd: &[u8]| history::Entry {
        timestamp: 1,
        cwd: cwd.to_vec(),
        command: command.to_string(),
    };
    let v = vec![
        e("git status", b"a"),
        e("git log", b"b"),
        e("git diff", b"a"),
    ];
    assert_eq!(history::suggest(&v, "git ", b"a"), "diff");
    assert_eq!(history::suggest(&v, "git l", b"a"), "og");
    assert_eq!(history::suggest(&v, "", b"a"), "");
}
#[test]
fn pure_completion_registry_paths_variables_and_directories() {
    let mut ctx = completion::Context {
        home: "/home/user".into(),
        ..Default::default()
    };
    ctx.commands.insert("echo".into());
    ctx.variables.insert("HOME".into(), "/home/user".into());
    ctx.variables.insert("DATA".into(), "/data".into());
    let files = vec![
        completion::FileEntry {
            name: "my file".into(),
            directory: false,
        },
        completion::FileEntry {
            name: "my dir".into(),
            directory: true,
        },
    ];
    let mut registry = completion::Registry::standard();
    registry.register("custom", Arc::new(|_, _| vec!["choice".into()]));
    let values = |line: &str| {
        completion::select(line, line.len(), &ctx, &files, &registry)
            .into_iter()
            .map(|s| s.value)
            .collect::<Vec<_>>()
    };
    assert_eq!(values("ec"), vec!["echo"]);
    assert_eq!(values("A=1 ec"), vec!["echo"]);
    assert_eq!(values("if ec"), vec!["echo"]);
    assert_eq!(values("cat | ec"), vec!["echo"]);
    assert_eq!(values("cat | cd my"), vec!["my\\ dir/"]);
    assert!(values("cat 'my").contains(&"my\\ file".into()));
    let quoted = completion::select("cat \"my\"", 7, &ctx, &files, &registry);
    assert!(quoted.iter().all(|s| s.span.end == 8));
    assert_eq!(values("echo $HO"), vec!["$HOME"]);
    assert_eq!(values("echo ${HO"), vec!["${HOME}"]);
    assert_eq!(values("cd my"), vec!["my\\ dir/"]);
    assert!(values("cat my").contains(&"my\\ file".into()));
    assert_eq!(values("custom ch"), vec!["choice"]);
    assert_eq!(completion::expanded_path("~/a", &ctx), "/home/user/a");
    assert_eq!(completion::expanded_path("$DATA/a", &ctx), "/data/a");
}
#[test]
fn prompt_defaults_git_head_and_parameter_expansion() {
    let d = TempDir::new();
    let mut e = env(&d);
    e.set(b"VALUE".to_vec(), b"hello".to_vec());
    e.set(b"PS1".to_vec(), b"$VALUE:$? ".to_vec());
    e.last_status = 7;
    let mut s = Shell::new(e, std::path::PathBuf::from("/unused"));
    assert_eq!(s.interactive_prompt(b"PS1").unwrap(), b"hello:7 ");
    assert_eq!(s.env.last_status, 7);
    assert_eq!(
        prompt::default_prompt(&d.0, Some(d.0.as_os_str().as_bytes()), false, false),
        "~$ "
    );
    assert_eq!(
        prompt::default_prompt(&d.0, Some(d.0.as_os_str().as_bytes()), true, false),
        "~# "
    );
    std::fs::create_dir(d.0.join(".git")).unwrap();
    std::fs::write(d.0.join(".git/HEAD"), b"ref: refs/heads/topic\n").unwrap();
    assert_eq!(prompt::branch(&d.0), Some("topic".into()));
}
#[test]
fn history_roundtrip_limits_dedup_space_and_clear() {
    let d = TempDir::new();
    let mut e = env(&d);
    e.set(b"HISTSIZE".to_vec(), b"2".to_vec());
    e.set(b"HISTFILESIZE".to_vec(), b"3".to_vec());
    let mut h = history::Store::open(&e).unwrap();
    h.record("one", b"/a").unwrap();
    h.record("one", b"/a").unwrap();
    h.record(" hidden", b"/a").unwrap();
    h.record("if true; then\n:; fi", b"/a").unwrap();
    h.record("three", b"/a").unwrap();
    assert_eq!(h.entries.len(), 2);
    let entries = history::decode(&std::fs::read(h.path.as_ref().unwrap()).unwrap());
    assert_eq!(entries.len(), 3);
    assert!(entries.iter().all(|e| e.timestamp > 0));
    assert_eq!(entries.get(1).unwrap().command, "if true; then\n:; fi");
    let loaded = history::Store::open(&e).unwrap();
    assert_eq!(loaded.entries, h.entries);
    h.clear().unwrap();
    assert!(std::fs::read(h.path.as_ref().unwrap()).unwrap().is_empty());
    assert!(history::decode(b"broken\nrsh1\tnan\t00\t0x\n").is_empty());
}
#[test]
#[ignore = "reference benchmark; run manually with --ignored --nocapture"]
fn highlight_200_chars_benchmark() {
    let line = format!("echo {}", "a".repeat(195));
    use reedline::Highlighter;
    let mut ctx = completion::Context::default();
    ctx.commands.insert("echo".into());
    ctx.commands
        .extend((0..2000).map(|i| format!("command{i}")));
    let renderer = highlight::Highlight {
        context: Arc::new(Mutex::new(ctx)),
        cache: completion::PathCache::default(),
    };
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = renderer.highlight(&line, line.len());
    }
    eprintln!(
        "200-byte full highlighter average: {:?}",
        start.elapsed() / 1000
    );
}
#[test]
#[ignore = "reference benchmark; run manually with --ignored --nocapture"]
fn path_2000_completion_benchmark() {
    let ctx = completion::Context {
        commands: (0..2000).map(|i| format!("command{i}")).collect(),
        ..Default::default()
    };
    let registry = completion::Registry::standard();
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = completion::select("command1", 8, &ctx, &[], &registry);
    }
    eprintln!("2000-command lookup average: {:?}", start.elapsed() / 1000);
}

#[test]
fn path_cache_invalidates_and_scans_only_executable_files() {
    use std::os::unix::fs::PermissionsExt;
    let d = TempDir::new();
    std::fs::create_dir(d.0.join("bin")).unwrap();
    std::fs::write(d.0.join("bin/unique-command"), b"#!/bin/sh\n").unwrap();
    std::fs::set_permissions(
        d.0.join("bin/unique-command"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    std::fs::write(d.0.join("bin/plain-file"), b"data").unwrap();
    let cache = completion::PathCache::default();
    cache.refresh(b"bin", &d.0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !cache.commands().contains("unique-command") {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(!cache.commands().contains("plain-file"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !cache.known_path("bin/unique-command", &d.0) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(!cache.known_path("bin/plain-file", &d.0));
    cache.refresh(b"missing-directory", &d.0);
    assert!(!cache.commands().contains("unique-command"));
}

#[test]
fn history_xdg_overrides_and_malformed_records() {
    let d = TempDir::new();
    let mut e = env(&d);
    e.set(
        b"XDG_DATA_HOME".to_vec(),
        d.0.join("data").as_os_str().as_bytes().to_vec(),
    );
    assert_eq!(history::path(&e), Some(d.0.join("data/rsh/history")));
    e.set(b"HISTFILE".to_vec(), b"custom".to_vec());
    assert_eq!(history::path(&e), Some(d.0.join("custom")));
    e.set(b"HISTFILE".to_vec(), Vec::new());
    assert_eq!(history::path(&e), None);
    for bytes in [
        b"rsh1\t0\t\tzz\n".as_slice(),
        b"\xff\n",
        b"rsh1\t0\t1\t00\n",
    ] {
        assert!(history::decode(bytes).is_empty());
    }
    let kinds = highlight::classify("echo '$HOME' \"$HOME\" ${HOME:-here}", |_| true);
    assert!(
        kinds
            .get(6..11)
            .unwrap()
            .iter()
            .all(|k| *k == highlight::Kind::String)
    );
    assert!(
        kinds
            .get(14..19)
            .unwrap()
            .iter()
            .all(|k| *k == highlight::Kind::Expansion)
    );
}

#[test]
#[ignore = "reference benchmark; run manually with --ignored --nocapture"]
fn path_2000_entries_benchmark() {
    let d = TempDir::new();
    let path = (0..2000)
        .map(|i| format!("missing-{i}"))
        .collect::<Vec<_>>()
        .join(":");
    let cache = completion::PathCache::default();
    cache.refresh(path.as_bytes(), &d.0);
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        cache.refresh(path.as_bytes(), &d.0);
    }
    eprintln!(
        "2000-entry PATH cache-hit average: {:?}",
        start.elapsed() / 1000
    );
}

#[test]
fn blocked_scan_does_not_block_command_handoff_indefinitely() {
    // A blocked filesystem cannot allow fd-owning scans and redirections to
    // overlap, but must also not hang the input loop waiting for that scan.
    let _scan = completion::scan_guard();
    let start = std::time::Instant::now();
    assert!(completion::execution_guard().is_err());
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}
