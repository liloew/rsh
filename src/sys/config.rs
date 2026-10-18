//! Deterministic user configuration discovery without recursive Rust calls.
use std::{
    io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};
/// Collect startup candidates before execution, preserving non-UTF-8 path ordering.
/// Directory symlinks are deliberately not traversed; regular-file links are valid.
/// A missing root is silent; other discovery errors are returned beside candidates.
pub fn rsh_config_files(root: &Path) -> (Vec<PathBuf>, Vec<(PathBuf, io::Error)>) {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    let mut errors = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if directory == root && error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                errors.push((directory, error));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    errors.push((directory.clone(), error));
                    continue;
                }
            };
            let path = entry.path();
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(error) => {
                    errors.push((path, error));
                    continue;
                }
            };
            if kind.is_dir() {
                pending.push(path);
            } else if entry.file_name().as_bytes().ends_with(b"rsh") {
                if kind.is_file() {
                    files.push(path);
                } else if kind.is_symlink() {
                    match std::fs::metadata(&path) {
                        Ok(meta) if meta.is_file() => files.push(path),
                        Ok(_) => {}
                        Err(error) => errors.push((path, error)),
                    }
                }
            }
        }
    }
    files.sort_by(|a, b| a.as_os_str().as_bytes().cmp(b.as_os_str().as_bytes()));
    errors.sort_by(|a, b| a.0.as_os_str().as_bytes().cmp(b.0.as_os_str().as_bytes()));
    (files, errors)
}
