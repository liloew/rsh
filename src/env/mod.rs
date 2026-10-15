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
    pub functions: BTreeMap<Vec<u8>, std::rc::Rc<crate::ast::Function>>,
    pub function_depth: usize,
    pub variables: BTreeMap<Vec<u8>, Variable>,
    /// Absolute logical cwd; the OS cwd is changed only in a forked child.
    pub cwd: PathBuf,
    pub last_status: u8,
    pub exit_requested: Option<u8>,
    /// Placeholder for set -C (the option builtin belongs to M7).
    pub noclobber: bool,
    pub noglob: bool,
    pub positional: Vec<Vec<u8>>,
    pub arg0: Vec<u8>,
    pub shell_pid: u32,
    pub last_background: Option<i32>,
    pub substitution_status: Option<u8>,
}

impl ShellEnv {
    pub fn inherited() -> std::io::Result<Self> {
        let cwd = std::env::current_dir()?;
        let mut env = Self {
            functions: BTreeMap::new(),
            function_depth: 0,
            variables: BTreeMap::new(),
            cwd,
            last_status: 0,
            exit_requested: None,
            noclobber: false,
            noglob: false,
            positional: Vec::new(),
            arg0: b"rsh".to_vec(),
            shell_pid: std::process::id(),
            last_background: None,
            substitution_status: None,
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

/// Length-delimited internal worker state, including arbitrary byte values.
pub fn encode_snapshot(fields: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for field in fields {
        out.extend((field.len() as u64).to_le_bytes());
        out.extend(field);
    }
    out
}
pub fn decode_snapshot(mut bytes: &[u8]) -> std::io::Result<Vec<Vec<u8>>> {
    let mut out = Vec::new();
    while !bytes.is_empty() {
        let Some((len, rest)) = bytes.split_at_checked(8) else {
            return Err(std::io::Error::other("truncated worker snapshot"));
        };
        let len = u64::from_le_bytes(
            len.try_into()
                .map_err(|_| std::io::Error::other("invalid snapshot length"))?,
        );
        let len =
            usize::try_from(len).map_err(|_| std::io::Error::other("snapshot length overflow"))?;
        let Some((field, rest)) = rest.split_at_checked(len) else {
            return Err(std::io::Error::other("truncated worker field"));
        };
        out.push(field.to_vec());
        bytes = rest;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_flags_survive_assignment_and_unset_export_is_not_empty() {
        let mut env = ShellEnv {
            functions: BTreeMap::new(),
            function_depth: 0,
            variables: BTreeMap::new(),
            cwd: PathBuf::from("/"),
            last_status: 0,
            exit_requested: None,
            noclobber: false,
            noglob: false,
            positional: Vec::new(),
            arg0: b"rsh".to_vec(),
            shell_pid: std::process::id(),
            last_background: None,
            substitution_status: None,
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
