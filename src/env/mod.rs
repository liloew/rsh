//! Byte-preserving shell variables and exported environment, without global mutation.
use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub value: Option<Vec<u8>>,
    pub exported: bool,
}

#[derive(Debug, Clone)]
pub struct ShellEnv {
    pub variables: BTreeMap<Vec<u8>, Variable>,
    /// Absolute logical cwd; the OS cwd is changed only in a forked child.
    pub cwd: PathBuf,
    pub last_status: u8,
    pub exit_requested: Option<u8>,
    /// Placeholder for set -C (the option builtin belongs to M7).
    pub noclobber: bool,
}

impl ShellEnv {
    pub fn inherited() -> std::io::Result<Self> {
        let cwd = std::env::current_dir()?;
        let mut env = Self {
            variables: BTreeMap::new(),
            cwd,
            last_status: 0,
            exit_requested: None,
            noclobber: false,
        };
        for (key, value) in std::env::vars_os() {
            env.variables.insert(
                key.as_bytes().to_vec(),
                Variable {
                    value: Some(value.as_bytes().to_vec()),
                    exported: true,
                },
            );
        }
        if let Some(pwd) = env.get(b"PWD") {
            let path = PathBuf::from(std::ffi::OsStr::from_bytes(pwd));
            if path.is_absolute() && path.canonicalize().is_ok_and(|p| p == env.cwd) {
                env.cwd = path;
            }
        }
        let pwd = env.cwd.as_os_str().as_bytes().to_vec();
        env.set(b"PWD".to_vec(), pwd);
        env.mark_exported(b"PWD".to_vec());
        Ok(env)
    }
    pub fn get(&self, name: &[u8]) -> Option<&[u8]> {
        self.variables
            .get(name)
            .and_then(|var| var.value.as_deref())
    }
    pub fn set(&mut self, name: Vec<u8>, value: Vec<u8>) {
        self.variables
            .entry(name)
            .and_modify(|var| var.value = Some(value.clone()))
            .or_insert(Variable {
                value: Some(value),
                exported: false,
            });
    }
    pub fn mark_exported(&mut self, name: Vec<u8>) {
        self.variables
            .entry(name)
            .or_insert(Variable {
                value: None,
                exported: false,
            })
            .exported = true;
    }
    pub fn exported(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.variables
            .iter()
            .filter_map(|(name, var)| {
                if var.exported {
                    var.value.as_ref().map(|v| (name.clone(), v.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}

pub fn valid_name(name: &[u8]) -> bool {
    name.first()
        .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
        && name.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_flags_survive_assignment_and_unset_export_is_not_empty() {
        let mut env = ShellEnv {
            variables: BTreeMap::new(),
            cwd: PathBuf::from("/"),
            last_status: 0,
            exit_requested: None,
            noclobber: false,
        };
        env.mark_exported(b"X".to_vec());
        assert!(env.exported().is_empty());
        env.set(b"X".to_vec(), b"one".to_vec());
        env.set(b"X".to_vec(), vec![0xff]);
        assert_eq!(env.exported(), vec![(b"X".to_vec(), vec![0xff])]);
        env.set(b"Y".to_vec(), b"local".to_vec());
        assert_eq!(env.exported().len(), 1);
    }
}
