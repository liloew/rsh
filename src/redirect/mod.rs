//! Ordered redirection planning. Raw descriptor operations remain in sys.
use crate::ast::{RedirOp, Redirection};
use crate::env::ShellEnv;
use crate::sys::{self, FdOp, FdPlan, OpenMode};
use std::ffi::{CString, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

fn descriptor(bytes: &[u8]) -> Option<i32> {
    if bytes.is_empty() {
        return None;
    }
    bytes.iter().try_fold(0i32, |value, byte| {
        if byte.is_ascii_digit() {
            value.checked_mul(10)?.checked_add(i32::from(byte - b'0'))
        } else {
            None
        }
    })
}
fn fail(context: Vec<u8>, reason: &[u8]) -> FdOp {
    FdOp::Fail {
        context,
        reason: reason.to_vec(),
    }
}

fn dup_source(redir: &Redirection, env: &ShellEnv) -> i32 {
    if !matches!(redir.op, RedirOp::DupInput | RedirOp::DupOutput) {
        return 9;
    }
    crate::expand::basic_word(&redir.target, env)
        .ok()
        .and_then(|v| descriptor(&v))
        .unwrap_or(9)
}

pub fn prepare(redirs: &[Redirection], env: &ShellEnv) -> FdPlan {
    let mut plan = FdPlan::default();
    // Reserve descriptors mentioned by the entire command, including later dup sources.
    let floor = redirs
        .iter()
        .flat_map(|r| {
            let target = r.fd.as_deref().and_then(descriptor).unwrap_or(9);
            let source = dup_source(r, env);
            [target, source]
        })
        .max()
        .unwrap_or(9)
        .saturating_add(1)
        .max(10);
    for redir in redirs {
        let default = if matches!(
            redir.op,
            RedirOp::Input | RedirOp::DupInput | RedirOp::ReadWrite | RedirOp::HereDoc(_)
        ) {
            0
        } else {
            1
        };
        let fd = if let Some(bytes) = &redir.fd {
            let Some(fd) = descriptor(bytes) else {
                plan.ops.push(fail(bytes.clone(), b"Bad file descriptor"));
                continue;
            };
            fd
        } else {
            default
        };
        if let RedirOp::HereDoc(document) = &redir.op {
            // TODO(M5): evaluate document.parts only when !document.quoted.
            match sys::here_input(&document.raw, floor) {
                Ok(source) => plan.ops.push(FdOp::Here { fd, source }),
                Err(error) => plan.ops.push(fail(
                    b"here-document".to_vec(),
                    error.to_string().as_bytes(),
                )),
            }
            continue;
        }
        let target = match crate::expand::basic_word(&redir.target, env) {
            Ok(target) => target,
            Err(error) => {
                plan.ops
                    .push(fail(b"redirection".to_vec(), error.to_string().as_bytes()));
                continue;
            }
        };
        if matches!(redir.op, RedirOp::DupInput | RedirOp::DupOutput) {
            plan.ops.push(if target == b"-" {
                FdOp::Close { fd }
            } else if let Some(source) = descriptor(&target) {
                FdOp::Dup {
                    fd,
                    source,
                    context: target,
                }
            } else {
                fail(target, b"Bad file descriptor")
            });
            continue;
        }
        let mode = match redir.op {
            RedirOp::Input => OpenMode::Input,
            RedirOp::Output if env.noclobber => OpenMode::NoClobber,
            RedirOp::Output | RedirOp::Clobber => OpenMode::Output,
            RedirOp::Append => OpenMode::Append,
            RedirOp::ReadWrite => OpenMode::ReadWrite,
            _ => continue,
        };
        let path = Path::new(OsStr::from_bytes(&target));
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            env.cwd.join(path)
        };
        plan.ops
            .push(match CString::new(absolute.as_os_str().as_bytes()) {
                Ok(path) => FdOp::Open {
                    fd,
                    path,
                    mode,
                    context: target,
                },
                Err(_) => fail(target, b"NUL byte in path"),
            });
    }
    plan
}

/// Keep the internal worker-source descriptor above every explicitly referenced fd.
pub fn source_floor(script: &crate::ast::Script, env: &ShellEnv) -> i32 {
    use crate::ast::{Command, CompoundCommand, SimplePart};
    fn redirs(redirs: &[Redirection], env: &ShellEnv) -> i32 {
        redirs
            .iter()
            .flat_map(|r| {
                [
                    r.fd.as_deref().and_then(descriptor).unwrap_or(9),
                    dup_source(r, env),
                ]
            })
            .max()
            .unwrap_or(9)
    }
    let mut max = 9;
    for item in &script.0 {
        for pipe in
            std::iter::once(&item.and_or.first).chain(item.and_or.rest.iter().map(|(_, p)| p))
        {
            for command in &pipe.commands {
                match command {
                    Command::Simple(s) => {
                        for p in &s.parts {
                            if let SimplePart::Redirection(r) = p {
                                max = max.max(redirs(std::slice::from_ref(r), env));
                            }
                        }
                    }
                    Command::Compound(compound, rs) => {
                        max = max.max(redirs(rs, env));
                        if let CompoundCommand::Brace(body) | CompoundCommand::Subshell(body) =
                            compound.as_ref()
                        {
                            max = max.max(source_floor(body, env));
                        }
                    }
                }
            }
        }
    }
    max.saturating_add(1).max(20)
}
