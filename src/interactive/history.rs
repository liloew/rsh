//! Versioned hex records retain multiline text and byte paths; a separate lock
//! inode serializes read/merge/rename even when history itself is replaced.
use crate::env::ShellEnv;
use std::os::unix::{ffi::OsStrExt, fs::OpenOptionsExt};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub timestamp: u64,
    pub cwd: Vec<u8>,
    pub command: String,
}
#[derive(Default)]
pub struct Store {
    pub entries: Vec<Entry>,
    pub path: Option<PathBuf>,
    pub size: usize,
    pub file_size: usize,
}
fn limit(env: &ShellEnv, name: &[u8], default: usize) -> usize {
    env.get(name)
        .and_then(|b| std::str::from_utf8(b).ok())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(default)
        .min(100_000)
}
pub fn path(env: &ShellEnv) -> Option<PathBuf> {
    if let Some(p) = env.get(b"HISTFILE") {
        return (!p.is_empty()).then(|| env.cwd.join(std::ffi::OsStr::from_bytes(p)));
    }
    let base = env
        .get(b"XDG_DATA_HOME")
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(std::ffi::OsStr::from_bytes(s)))
        .or_else(|| {
            env.get(b"HOME")
                .map(|s| PathBuf::from(std::ffi::OsStr::from_bytes(s)).join(".local/share"))
        })?;
    Some(base.join("rsh/history"))
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| {
            let a = (*p.first()? as char).to_digit(16)?;
            let b = (*p.get(1)? as char).to_digit(16)?;
            Some((a * 16 + b) as u8)
        })
        .collect()
}
pub fn decode(bytes: &[u8]) -> Vec<Entry> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            if p.next()? != "rsh1" {
                return None;
            }
            let timestamp = p.next()?.parse().ok()?;
            let cwd = unhex(p.next()?)?;
            let command = String::from_utf8(unhex(p.next()?)?).ok()?;
            (p.next().is_none()).then_some(Entry {
                timestamp,
                cwd,
                command,
            })
        })
        .collect()
}
pub(crate) fn encode(entries: &[Entry]) -> Vec<u8> {
    entries
        .iter()
        .map(|e| {
            format!(
                "rsh1\t{}\t{}\t{}\n",
                e.timestamp,
                hex(&e.cwd),
                hex(e.command.as_bytes())
            )
        })
        .collect::<String>()
        .into_bytes()
}
fn read(path: &std::path::Path) -> io::Result<Vec<Entry>> {
    match fs::File::open(path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(64 * 1024 * 1024).read_to_end(&mut bytes)?;
            Ok(decode(&bytes))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}
impl Store {
    pub fn open(env: &ShellEnv) -> io::Result<Self> {
        let path = path(env);
        let mut entries = if let Some(p) = &path {
            read(p)?
        } else {
            Vec::new()
        };
        let size = limit(env, b"HISTSIZE", 1000);
        if entries.len() > size {
            entries.drain(..entries.len() - size);
        }
        Ok(Self {
            entries,
            path,
            size,
            file_size: limit(env, b"HISTFILESIZE", 2000),
        })
    }
    pub fn configure(&mut self, env: &ShellEnv) {
        self.size = limit(env, b"HISTSIZE", 1000);
        self.file_size = limit(env, b"HISTFILESIZE", 2000);
        self.path = path(env);
        if self.entries.len() > self.size {
            self.entries.drain(..self.entries.len() - self.size);
        }
    }
    pub fn record(&mut self, command: &str, cwd: &[u8]) -> io::Result<()> {
        if command.trim().is_empty() || command.starts_with(' ') || command.len() > 1024 * 1024 {
            return Ok(());
        }
        let entry = Entry {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            cwd: cwd.to_vec(),
            command: command.to_string(),
        };
        if self.entries.last().is_none_or(|e| e.command != command) {
            self.entries.push(entry.clone());
        }
        if self.entries.len() > self.size {
            self.entries.drain(..self.entries.len() - self.size);
        }
        self.persist(Some(entry), false)
    }
    pub fn clear(&mut self) -> io::Result<()> {
        self.entries.clear();
        self.persist(None, true)
    }
    fn persist(&self, entry: Option<Entry>, clear: bool) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        fs::create_dir_all(parent)?;
        let lock_path = path.with_extension("lock");
        let file = OpenOptions::new()
            .create(true)
            .mode(0o600)
            .read(true)
            .write(true)
            .truncate(false)
            .open(lock_path)?;
        let _lock = crate::sys::HistoryLock::acquire(file)?;
        let mut entries = if clear { Vec::new() } else { read(path)? };
        if let Some(e) = entry
            && entries.last().is_none_or(|last| last.command != e.command)
        {
            entries.push(e);
        }
        if entries.len() > self.file_size {
            entries.drain(..entries.len() - self.file_size);
        }
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tmp = path.with_extension(format!(
            "tmp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let result = (|| {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(&encode(&entries))?;
            f.sync_all()?;
            fs::rename(&tmp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }
}
pub fn suggest(entries: &[Entry], prefix: &str, cwd: &[u8]) -> String {
    if prefix.is_empty() {
        return String::new();
    }
    entries
        .iter()
        .rev()
        .filter(|e| e.cwd == cwd)
        .chain(entries.iter().rev().filter(|e| e.cwd != cwd))
        .find_map(|e| {
            e.command
                .strip_prefix(prefix)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_default()
}
