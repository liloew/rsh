pub mod options;
// Byte-preserving shell state, without global variable/cwd mutation.
use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub value: Option<Vec<u8>>,
    pub exported: bool,
    pub readonly: bool,
}

#[derive(Debug, Clone)]
pub struct ShellEnv {
    pub trap_listing_only: bool,
    pub stdin_source: bool,
    pub stdin_lines: usize,
    pub options: options::Options,
    pub aliases: BTreeMap<Vec<u8>, Vec<u8>>,
    pub traps: BTreeMap<i32, Vec<u8>>,
    pub source_depth: usize,
    pub errexit_ignored: bool,
    pub getopts_offset: usize,
    pub functions: BTreeMap<Vec<u8>, std::rc::Rc<crate::ast::Function>>,
    pub function_depth: usize,
    pub variables: BTreeMap<Vec<u8>, Variable>,
    /// Absolute logical cwd; the OS cwd is changed only in a forked child.
    pub cwd: PathBuf,
    pub last_status: u8,
    pub exit_requested: Option<u8>,
    /// Whether set -C prevents overwriting existing regular files.
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
            trap_listing_only: false,
            stdin_source: false,
            stdin_lines: 0,
            options: options::Options::default(),
            aliases: BTreeMap::new(),
            traps: BTreeMap::new(),
            source_depth: 0,
            errexit_ignored: false,
            getopts_offset: 1,
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
                    readonly: false,
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
        for (name, value) in [
            (b"IFS".as_slice(), b" \t\n".as_slice()),
            (b"PATH", b"/usr/bin:/bin"),
            (b"PS2", b"> "),
            (b"PS4", b"+ "),
            (b"OPTIND", b"1"),
            (b"OPTERR", b"1"),
            (b"LINENO", b"1"),
        ] {
            if env.get(name).is_none() {
                env.set(name.to_vec(), value.to_vec());
            }
        }
        if env.get(b"HOME").is_none()
            && let Some(home) = crate::sys::user_home(None)
        {
            env.set(b"HOME".to_vec(), home);
        }
        env.set(
            b"PPID".to_vec(),
            crate::sys::parent_id().to_string().into_bytes(),
        );
        Ok(env)
    }
    pub fn get(&self, name: &[u8]) -> Option<&[u8]> {
        self.variables
            .get(name)
            .and_then(|var| var.value.as_deref())
    }
    pub fn set(&mut self, name: Vec<u8>, value: Vec<u8>) {
        if name == b"OPTIND" {
            self.getopts_offset = 1;
        }
        self.variables
            .entry(name)
            .and_modify(|var| var.value = Some(value.clone()))
            .or_insert(Variable {
                value: Some(value),
                exported: false,
                readonly: false,
            });
    }
    pub fn is_readonly(&self, name: &[u8]) -> bool {
        self.variables.get(name).is_some_and(|v| v.readonly)
    }
    pub fn assign(&mut self, name: Vec<u8>, value: Vec<u8>) -> Result<(), String> {
        if self.is_readonly(&name) {
            return Err(format!(
                "{}: readonly variable",
                String::from_utf8_lossy(&name)
            ));
        }
        self.set(name, value);
        Ok(())
    }
    pub fn mark_exported(&mut self, name: Vec<u8>) {
        self.variables
            .entry(name)
            .or_insert(Variable {
                value: None,
                exported: false,
                readonly: false,
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
            trap_listing_only: false,
            stdin_source: false,
            stdin_lines: 0,
            options: options::Options::default(),
            aliases: BTreeMap::new(),
            traps: BTreeMap::new(),
            source_depth: 0,
            errexit_ignored: false,
            getopts_offset: 1,
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
