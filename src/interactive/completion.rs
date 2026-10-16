//! Pure candidate selection over a bounded filesystem listing, with a pluggable registry.
use crate::env::ShellEnv;
use std::os::unix::ffi::OsStrExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
// Directory iteration owns process-global descriptors. Redirections must never
// run while an iterator can still close a descriptor they have just replaced.
static SCANNERS: std::sync::RwLock<()> = std::sync::RwLock::new(());
pub(crate) fn execution_guard() -> Result<std::sync::RwLockWriteGuard<'static, ()>, &'static str> {
    let deadline = Instant::now() + Duration::from_millis(75);
    loop {
        match SCANNERS.try_write() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(e)) => return Ok(e.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    return Err("completion filesystem is busy; press Enter to retry");
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}
pub(crate) fn scan_guard() -> std::sync::RwLockReadGuard<'static, ()> {
    SCANNERS.read().unwrap_or_else(|e| e.into_inner())
}
#[derive(Clone, Default)]
pub struct Context {
    pub cwd: PathBuf,
    pub home: String,
    pub variables: BTreeMap<String, String>,
    pub commands: BTreeSet<String>,
    pub jobs: Vec<String>,
}
impl Context {
    pub fn from_env(env: &ShellEnv) -> Self {
        Self {
            cwd: env.cwd.clone(),
            home: String::from_utf8_lossy(env.get(b"HOME").unwrap_or_default()).into_owned(),
            variables: env
                .variables
                .iter()
                .filter_map(|(k, v)| {
                    Some((
                        String::from_utf8(k.clone()).ok()?,
                        String::from_utf8(v.value.clone().unwrap_or_default()).ok()?,
                    ))
                })
                .collect(),
            commands: super::builtin_names()
                .iter()
                .map(|s| s.to_string())
                .chain(
                    env.functions
                        .keys()
                        .chain(env.aliases.keys())
                        .filter_map(|s| String::from_utf8(s.clone()).ok()),
                )
                .collect(),
            jobs: Vec::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub directory: bool,
}
pub type Provider = Arc<dyn Fn(&[String], &Context) -> Vec<String> + Send + Sync>;
#[derive(Clone, Default)]
pub struct Registry {
    providers: BTreeMap<String, Provider>,
}
impl Registry {
    pub fn register(&mut self, name: impl Into<String>, provider: Provider) {
        self.providers.insert(name.into(), provider);
    }
    pub fn standard() -> Self {
        let mut r = Self::default();
        for (cmd, values) in [
            (
                "git",
                vec![
                    "add", "branch", "checkout", "commit", "diff", "fetch", "log", "merge", "pull",
                    "push", "restore", "status", "switch",
                ],
            ),
            (
                "cargo",
                vec![
                    "add", "build", "check", "clippy", "doc", "fmt", "run", "test", "update",
                ],
            ),
        ] {
            r.register(
                cmd,
                Arc::new(move |_, _| values.iter().map(|s| s.to_string()).collect()),
            );
        }
        for cmd in ["fg", "bg", "kill"] {
            r.register(cmd, Arc::new(|_, ctx| ctx.jobs.clone()));
        }
        r
    }
    pub fn candidates(&self, args: &[String], ctx: &Context) -> Vec<String> {
        args.first()
            .and_then(|s| self.providers.get(s))
            .map(|p| p(args, ctx))
            .unwrap_or_default()
    }
}
fn start(line: &str, pos: usize) -> usize {
    let mut quoted = None;
    let mut escaped = false;
    let mut start = 0;
    for (i, c) in line.get(..pos).unwrap_or("").char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && quoted != Some('\'') {
            escaped = true;
            continue;
        }
        if matches!(c, '\'' | '"') {
            if quoted == Some(c) {
                quoted = None;
            } else if quoted.is_none() {
                quoted = Some(c);
            }
            continue;
        }
        if quoted.is_none() && (c.is_whitespace() || ";|&()".contains(c)) {
            start = i + c.len_utf8();
        }
    }
    start
}
pub fn escape(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        if !c.is_alphanumeric() && !"/_-.~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
pub fn expanded_path(prefix: &str, ctx: &Context) -> String {
    if prefix == "~" {
        return ctx.home.clone();
    }
    if let Some(p) = prefix.strip_prefix("~/") {
        return format!("{}/{p}", ctx.home);
    }
    if let Some(p) = prefix.strip_prefix('$') {
        let (name, rest) = if let Some(p) = p.strip_prefix('{') {
            if let Some(i) = p.find('}') {
                (p.get(..i).unwrap_or(""), p.get(i + 1..).unwrap_or(""))
            } else {
                return prefix.to_string();
            }
        } else {
            let i = p.find('/').unwrap_or(p.len());
            (p.get(..i).unwrap_or(""), p.get(i..).unwrap_or(""))
        };
        if let Some(value) = ctx.variables.get(name) {
            return format!("{value}{rest}");
        }
    }
    prefix.replace("\\ ", " ")
}
fn command_context(before: &str) -> (bool, Vec<String>) {
    use crate::lexer::{Operator, TokenKind};
    let mut command = true;
    let mut target = false;
    let mut args = Vec::new();
    for token in crate::lexer::lex(before.as_bytes()).unwrap_or_default() {
        match token.kind {
            TokenKind::Word(ref word) => {
                if target {
                    target = false;
                    continue;
                }
                if command && crate::parser::assignment(word).is_some() {
                    continue;
                }
                let raw = before
                    .get(token.span.start.offset..token.span.end.offset)
                    .unwrap_or("");
                if command
                    && matches!(
                        raw,
                        "if" | "then" | "elif" | "else" | "while" | "until" | "do" | "!" | "{"
                    )
                {
                    continue;
                }
                args.push(raw.to_string());
                command = false;
            }
            TokenKind::Operator(
                Operator::Less
                | Operator::Greater
                | Operator::Append
                | Operator::Clobber
                | Operator::DupInput
                | Operator::DupOutput
                | Operator::ReadWrite
                | Operator::HereDoc
                | Operator::HereDocStrip,
            ) => {
                target = true;
            }
            TokenKind::Newline | TokenKind::Operator(_) => {
                command = true;
                target = false;
                args.clear();
            }
            _ => {}
        }
    }
    (command && !target, args)
}
fn path_prefix(prefix: &str) -> &str {
    if let Some(quote) = prefix.chars().next().filter(|c| matches!(c, '\'' | '"')) {
        let prefix = prefix.get(1..).unwrap_or("");
        prefix.strip_suffix(quote).unwrap_or(prefix)
    } else {
        prefix
    }
}
fn expand_prefix(prefix: &str, ctx: &Context, quote: Option<char>) -> String {
    if quote == Some('\'') || (quote == Some('"') && prefix.starts_with('~')) {
        prefix.replace("\\ ", " ")
    } else {
        expanded_path(prefix, ctx)
    }
}
pub fn select(
    line: &str,
    pos: usize,
    ctx: &Context,
    files: &[FileEntry],
    registry: &Registry,
) -> Vec<reedline::Suggestion> {
    let pos = pos.min(line.len());
    if !line.is_char_boundary(pos) {
        return Vec::new();
    }
    let begin = start(line, pos);
    let prefix = line.get(begin..pos).unwrap_or("");
    let before = line.get(..begin).unwrap_or("");
    let (command, args) = command_context(before);
    let quoted = prefix.chars().next().filter(|c| matches!(c, '\'' | '"'));
    let span_end = if quoted.is_some_and(|q| line.get(pos..).is_some_and(|s| s.starts_with(q))) {
        pos + 1
    } else {
        pos
    };
    let prefix = path_prefix(prefix);
    let mut values = BTreeSet::new();
    if let Some(name) = prefix
        .strip_prefix("${")
        .or_else(|| prefix.strip_prefix('$'))
        && quoted != Some('\'')
        && !name.contains('/')
        && !name.contains('}')
    {
        for v in ctx.variables.keys().filter(|v| v.starts_with(name)) {
            let value = if prefix.starts_with("${") {
                format!("${{{v}}}")
            } else {
                format!("${v}")
            };
            values.insert(if quoted == Some('"') {
                format!("\"{value}\"")
            } else {
                value
            });
        }
    } else {
        if command && !prefix.contains('/') {
            values.extend(
                ctx.commands
                    .iter()
                    .filter(|s| s.starts_with(prefix))
                    .cloned(),
            );
        }
        if !command {
            values.extend(
                registry
                    .candidates(&args, ctx)
                    .into_iter()
                    .filter(|s| s.starts_with(prefix))
                    .map(|s| {
                        if args
                            .first()
                            .is_some_and(|s| matches!(s.as_str(), "kill" | "fg" | "bg"))
                        {
                            escape(&s)
                        } else {
                            s
                        }
                    }),
            );
        }
        let path = prefix
            .rsplit_once('/')
            .map(|(p, _)| format!("{p}/"))
            .unwrap_or_default();
        let leaf = prefix
            .rsplit('/')
            .next()
            .unwrap_or(prefix)
            .trim_start_matches(['\'', '"'])
            .replace("\\ ", " ");
        let allow_files = !command || prefix.contains('/') || prefix.starts_with(['~', '$']);
        for e in files.iter().filter(|_| allow_files).filter(|e| {
            e.name.starts_with(&leaf)
                && (!e.name.starts_with('.') || leaf.starts_with('.'))
                && (args.first().is_none_or(|s| s != "cd") || e.directory)
        }) {
            let candidate = format!("{path}{}{}", e.name, if e.directory { "/" } else { "" });
            // Expand prefix paths to escaped absolute paths rather than quoting away $ or ~.
            values.insert(escape(&expand_prefix(&candidate, ctx, quoted)));
        }
    }
    values
        .into_iter()
        .take(256)
        .map(|value| reedline::Suggestion {
            append_whitespace: !value.ends_with('/') && !value.starts_with('$'),
            value,
            span: reedline::Span {
                start: begin,
                end: span_end,
            },
            ..Default::default()
        })
        .collect()
}
pub fn listing(path: PathBuf) -> Vec<FileEntry> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    static ACTIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    if ACTIVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 2 {
        ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        return Vec::new();
    }
    let spawned = std::thread::Builder::new()
        .name("rsh-completion".into())
        .spawn(move || {
            let _scan = scan_guard();
            let start = Instant::now();
            let mut out = Vec::new();
            if let Ok(read) = std::fs::read_dir(path) {
                for e in read.take(2048).flatten() {
                    if start.elapsed() > Duration::from_millis(20) {
                        break;
                    }
                    if let Some(name) = e.file_name().to_str() {
                        out.push(FileEntry {
                            name: name.to_string(),
                            directory: e.path().is_dir(),
                        });
                    }
                }
            }
            let _ = tx.send(out);
            ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        });
    if spawned.is_err() {
        ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        return Vec::new();
    }
    rx.recv_timeout(Duration::from_millis(25))
        .unwrap_or_default()
}
#[derive(Default)]
struct CacheState {
    key: Vec<u8>,
    cwd: PathBuf,
    updated: Option<Instant>,
    commands: BTreeSet<String>,
    generation: u64,
    paths: BTreeMap<PathBuf, (Instant, Option<bool>)>,
}
#[derive(Clone, Default)]
pub struct PathCache(Arc<Mutex<CacheState>>);
impl PathCache {
    pub fn refresh(&self, path: &[u8], cwd: &Path) {
        let Ok(mut s) = self.0.lock() else {
            return;
        };
        if s.key == path
            && s.cwd == cwd
            && s.updated
                .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
        {
            return;
        }
        if s.key != path || s.cwd != cwd {
            s.commands.clear();
        }
        s.key = path.to_vec();
        s.cwd = cwd.to_path_buf();
        s.updated = Some(Instant::now());
        s.generation = s.generation.wrapping_add(1);
        let generation = s.generation;
        let path = path.to_vec();
        let cwd = cwd.to_path_buf();
        let shared = self.0.clone();
        drop(s);
        static ACTIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        if ACTIVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 2 {
            ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
        let spawned = std::thread::Builder::new()
            .name("rsh-path-cache".into())
            .spawn(move || {
                let _scan = scan_guard();
                let start = Instant::now();
                let mut found = BTreeSet::new();
                let mut count = 0;
                for dir in path.split(|b| *b == b':').take(2000) {
                    if start.elapsed() > Duration::from_millis(50) || count >= 20_000 {
                        break;
                    }
                    if let Ok(entries) =
                        std::fs::read_dir(cwd.join(std::ffi::OsStr::from_bytes(dir)))
                    {
                        for e in entries.take(4096).flatten() {
                            count += 1;
                            if count >= 20_000 || start.elapsed() > Duration::from_millis(50) {
                                break;
                            }
                            if crate::sys::accessible(&e.path(), 1)
                                && e.path().is_file()
                                && let Some(name) = e.file_name().to_str()
                            {
                                found.insert(name.to_string());
                            }
                        }
                    }
                }
                if let Ok(mut s) = shared.lock()
                    && s.generation == generation
                {
                    s.commands = found;
                }
                ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            });
        if spawned.is_err() {
            ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    /// Explicit paths use the same nonblocking TTL policy as PATH commands.
    pub fn known_path(&self, name: &str, cwd: &Path) -> bool {
        if !name.contains('/') {
            return false;
        }
        let path = cwd.join(name);
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        if let Some((when, value)) = state.paths.get(&path)
            && when.elapsed() < Duration::from_secs(2)
        {
            return value.unwrap_or(false);
        }
        static ACTIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        if ACTIVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 2 {
            ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            return false;
        }
        if state.paths.len() >= 256 {
            state.paths.clear();
        }
        let when = Instant::now();
        state.paths.insert(path.clone(), (when, None));
        drop(state);
        let shared = self.0.clone();
        let spawned = std::thread::Builder::new()
            .name("rsh-path-check".into())
            .spawn(move || {
                let _scan = scan_guard();
                let known = crate::sys::accessible(&path, 1) && path.is_file();
                if let Ok(mut state) = shared.lock()
                    && let Some((stamp, value)) = state.paths.get_mut(&path)
                    && *stamp == when
                {
                    *value = Some(known);
                }
                ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            });
        if spawned.is_err() {
            ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
        false
    }
    pub fn commands(&self) -> BTreeSet<String> {
        self.0
            .lock()
            .map(|s| s.commands.clone())
            .unwrap_or_default()
    }
}
pub struct Completion {
    pub context: Arc<Mutex<Context>>,
    pub registry: Registry,
    pub cache: PathCache,
}
impl reedline::Completer for Completion {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<reedline::Suggestion> {
        let Ok(mut ctx) = self.context.lock().map(|s| s.clone()) else {
            return Vec::new();
        };
        ctx.commands.extend(self.cache.commands());
        let begin = start(line, pos.min(line.len()));
        let prefix = line.get(begin..pos).unwrap_or("");
        let expanded = expand_prefix(
            path_prefix(prefix),
            &ctx,
            prefix.chars().next().filter(|c| matches!(c, '\'' | '"')),
        );
        let parent = expanded
            .rsplit_once('/')
            .map(|(p, _)| if p.is_empty() { "/" } else { p })
            .unwrap_or(".");
        select(
            line,
            pos,
            &ctx,
            &listing(ctx.cwd.join(parent)),
            &self.registry,
        )
    }
}
