//! M3 AST execution. No redirections, compound commands or full expansions yet.
use crate::ast::*;
use crate::builtins::{self, Builtin, Outcome};
use crate::env::ShellEnv;
use crate::sys::{self, Child, PreparedProcess, SpawnOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ExecError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Prepare(#[from] sys::PrepareError),
    #[error(transparent)]
    Expand(#[from] crate::expand::ExpandError),
    #[error("rsh: execution: {0}")]
    Unsupported(&'static str),
}

pub struct Shell {
    pub env: ShellEnv,
    executable: PathBuf,
    background: Vec<Child>,
    pub background_signals: bool,
}
struct Simple {
    words: Vec<Vec<u8>>,
    assignments: Vec<(Vec<u8>, Vec<u8>)>,
    builtin: Option<Builtin>,
    special: bool,
}
impl Shell {
    pub fn new(env: ShellEnv, executable: PathBuf) -> Self {
        Self {
            env,
            executable,
            background: Vec::new(),
            background_signals: false,
        }
    }
    pub fn run(&mut self, script: &Script) -> u8 {
        for item in &script.0 {
            self.reap_background();
            if item.separator == Separator::Background {
                let result = self.launch_background(&item.and_or);
                self.env.last_status = self.report(result);
            } else {
                self.and_or(&item.and_or);
            }
            if self.env.exit_requested.is_some() {
                break;
            }
        }
        self.reap_background();
        self.env.exit_requested.unwrap_or(self.env.last_status)
    }
    fn report(&self, result: Result<u8, ExecError>) -> u8 {
        match result {
            Ok(status) => status,
            Err(error) => {
                if let ExecError::Io(error) = &error {
                    eprintln!("rsh: execution: {error}");
                } else {
                    eprintln!("{error}");
                }
                if matches!(error, ExecError::Unsupported(_)) {
                    2
                } else {
                    1
                }
            }
        }
    }
    fn and_or(&mut self, list: &AndOr) {
        let result = self.pipeline(&list.first);
        self.env.last_status = self.report(result);
        for (op, pipeline) in &list.rest {
            if self.env.exit_requested.is_some() {
                break;
            }
            if matches!(op, AndOrOp::And) == (self.env.last_status == 0) {
                let result = self.pipeline(pipeline);
                self.env.last_status = self.report(result);
            }
        }
    }
    fn simple(&self, command: &Command) -> Result<Simple, ExecError> {
        let Command::Simple(simple) = command else {
            return Err(ExecError::Unsupported("compound execution is TODO(M6)"));
        };
        let mut words = Vec::new();
        let mut assignments = Vec::new();
        let mut assigned = self.env.clone();
        for part in &simple.parts {
            match part {
                SimplePart::Word(word) => words.push(crate::expand::basic_word(word, &self.env)?),
                SimplePart::Assignment(assignment) => {
                    let value = crate::expand::basic_word(&assignment.value, &assigned)?;
                    assigned.set(assignment.name.clone(), value.clone());
                    assignments.push((assignment.name.clone(), value));
                }
                SimplePart::Redirection(_) => {
                    return Err(ExecError::Unsupported("redirections are TODO(M4)"));
                }
            }
        }
        let name = words.first().map(Vec::as_slice).unwrap_or_default();
        let special = builtins::special(name);
        // Dispatch order: special builtin -> TODO(M6) function -> regular -> external.
        let builtin = special.or_else(|| builtins::regular(name));
        Ok(Simple {
            words,
            assignments,
            builtin,
            special: special.is_some(),
        })
    }
    fn parent_builtin(&mut self, simple: &Simple) -> Outcome {
        let saved: Vec<_> = simple
            .assignments
            .iter()
            .map(|(name, _)| (name.clone(), self.env.variables.get(name).cloned()))
            .collect();
        for (name, value) in &simple.assignments {
            self.env.set(name.clone(), value.clone());
        }
        let outcome = if let Some(builtin) = simple.builtin {
            builtins::run(
                builtin,
                simple.words.get(1..).unwrap_or_default(),
                &mut self.env,
            )
        } else {
            Outcome::default()
        };
        if !simple.special && !simple.words.is_empty() {
            for (name, value) in saved {
                match value {
                    Some(var) => {
                        self.env.variables.insert(name, var);
                    }
                    None => {
                        self.env.variables.remove(&name);
                    }
                }
            }
        }
        outcome
    }
    fn prepare(&self, simple: &Simple) -> Result<PreparedProcess, ExecError> {
        let mut local = self.env.clone();
        for (name, value) in &simple.assignments {
            local.set(name.clone(), value.clone());
        }
        if simple.words.is_empty() || simple.builtin.is_some() {
            let outcome = if let Some(builtin) = simple.builtin {
                builtins::run(
                    builtin,
                    simple.words.get(1..).unwrap_or_default(),
                    &mut local,
                )
            } else {
                Outcome::default()
            };
            return Ok(PreparedProcess::finished(
                outcome.status,
                outcome.stdout,
                outcome.stderr,
                &self.env.cwd,
            )?);
        }
        for (name, _) in &simple.assignments {
            local.mark_exported(name.clone());
        }
        Ok(PreparedProcess::external(
            &simple.words,
            &local.exported(),
            &local.cwd,
            local.get(b"PATH"),
        )?)
    }
    fn pipeline(&mut self, pipeline: &Pipeline) -> Result<u8, ExecError> {
        self.reap_background();
        let commands = pipeline
            .commands
            .iter()
            .map(|command| self.simple(command))
            .collect::<Result<Vec<_>, _>>()?;
        let status = if let [simple] = commands.as_slice()
            && (simple.words.is_empty() || simple.builtin.is_some())
        {
            let outcome = self.parent_builtin(simple);
            std::io::stdout().lock().write_all(&outcome.stdout)?;
            std::io::stderr().lock().write_all(&outcome.stderr)?;
            outcome.status
        } else {
            let prepared = commands
                .iter()
                .map(|simple| self.prepare(simple))
                .collect::<Result<Vec<_>, _>>()?;
            let children = sys::spawn_pipeline(
                &prepared,
                SpawnOptions {
                    ignore_int_quit: self.background_signals,
                    ..SpawnOptions::default()
                },
            )?;
            let mut last = 0;
            let mut error = None;
            // Wait for EVERY child, even after a wait error; last command defines status.
            for child in children {
                match child.wait() {
                    Ok(status) => last = status,
                    Err(e) => {
                        error.get_or_insert(e);
                    }
                }
            }
            if let Some(error) = error {
                return Err(error.into());
            }
            last
        };
        self.reap_background();
        Ok(if pipeline.negated {
            u8::from(status == 0)
        } else {
            status
        })
    }
    fn launch_background(&mut self, list: &AndOr) -> Result<u8, ExecError> {
        // Re-exec is essential: a post-fork child must never run the Rust interpreter.
        let source = Script(vec![ListItem {
            and_or: list.clone(),
            separator: Separator::Sequential,
        }])
        .to_source_bytes();
        let mut words = vec![
            self.executable.as_os_str().as_bytes().to_vec(),
            b"--m3-worker".to_vec(),
            source,
            self.env.last_status.to_string().into_bytes(),
        ];
        for (name, var) in &self.env.variables {
            let mut encoded = vec![if var.value.is_none() {
                b'u'
            } else if var.exported {
                b'e'
            } else {
                b'v'
            }];
            encoded.extend_from_slice(name);
            encoded.push(b'=');
            if let Some(value) = &var.value {
                encoded.extend_from_slice(value);
            }
            words.push(encoded);
        }
        let process = PreparedProcess::external(&words, &self.env.exported(), &self.env.cwd, None)?;
        self.background.extend(sys::spawn_pipeline(
            &[process],
            SpawnOptions {
                detached: true,
                null_stdin: true,
                ignore_int_quit: true,
            },
        )?);
        Ok(0)
    }
    pub fn reap_background(&mut self) {
        self.background
            .retain(|child| matches!(child.try_wait(), Ok(None)));
    }
    pub fn background_count(&self) -> usize {
        self.background.len()
    }
}
