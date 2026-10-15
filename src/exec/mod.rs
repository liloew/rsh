//! M3/M4 execution: ordered redirections and grouping, without M5 expansions or M6 control flow.
use crate::ast::*;
use crate::builtins::{self, Builtin, Outcome};
use crate::env::ShellEnv;
use crate::sys::{self, Child, PreparedProcess, SpawnOptions};
use std::os::fd::AsRawFd;
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
    fd_floor: i32,
    pub background_signals: bool,
}
struct Simple {
    words: Vec<Vec<u8>>,
    assignments: Vec<(Vec<u8>, Vec<u8>)>,
    builtin: Option<Builtin>,
    special: bool,
    redirs: Vec<Redirection>,
}
impl Shell {
    pub fn new(env: ShellEnv, executable: PathBuf) -> Self {
        Self {
            env,
            executable,
            background: Vec::new(),
            fd_floor: 20,
            background_signals: false,
        }
    }
    pub fn run(&mut self, script: &Script) -> u8 {
        self.fd_floor = self
            .fd_floor
            .max(crate::redirect::source_floor(script, &self.env));
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
                let diagnostic = if let ExecError::Io(error) = &error {
                    format!("rsh: execution: {error}\n")
                } else {
                    format!("{error}\n")
                };
                let _ = sys::write_fd(2, diagnostic.as_bytes());
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
        let mut redirs = Vec::new();
        for part in &simple.parts {
            match part {
                SimplePart::Word(word) => words.push(crate::expand::basic_word(word, &self.env)?),
                SimplePart::Assignment(assignment) => {
                    let value = crate::expand::basic_word(&assignment.value, &assigned)?;
                    assigned.set(assignment.name.clone(), value.clone());
                    assignments.push((assignment.name.clone(), value));
                }
                SimplePart::Redirection(redirection) => redirs.push(redirection.clone()),
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
            redirs,
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
            )?
            .with_redirects(
                crate::redirect::prepare(&simple.redirs, &self.env),
                if simple.special { 2 } else { 1 },
            ));
        }
        for (name, _) in &simple.assignments {
            local.mark_exported(name.clone());
        }
        Ok(PreparedProcess::external(
            &simple.words,
            &local.exported(),
            &local.cwd,
            local.get(b"PATH"),
        )?
        .with_redirects(crate::redirect::prepare(&simple.redirs, &self.env), 1))
    }
    fn pipeline(&mut self, pipeline: &Pipeline) -> Result<u8, ExecError> {
        self.reap_background();
        let status = if let [Command::Compound(compound, redirs)] = pipeline.commands.as_slice()
            && matches!(compound.as_ref(), CompoundCommand::Brace(_))
        {
            let CompoundCommand::Brace(body) = compound.as_ref() else {
                return Err(ExecError::Unsupported("compound command"));
            };
            let plan = crate::redirect::prepare(redirs, &self.env);
            let guard = sys::FdGuard::save(&plan, self.fd_floor)?;
            let status = if let Err(error) = plan.apply() {
                plan.report(error);
                1
            } else {
                self.run(body)
            };
            drop(guard);
            status
        } else if let [Command::Simple(command)] = pipeline.commands.as_slice() {
            let simple = self.simple(&Command::Simple(command.clone()))?;
            if simple.words.is_empty() || simple.builtin.is_some() {
                self.redirected_builtin(&simple)?
            } else {
                self.wait_processes(vec![self.prepare(&simple)?])?
            }
        } else {
            let prepared = pipeline
                .commands
                .iter()
                .enumerate()
                .map(|(index, command)| {
                    self.prepare_command(command, index > 0, index + 1 < pipeline.commands.len())
                })
                .collect::<Result<Vec<_>, _>>()?;
            self.wait_processes(prepared)?
        };
        self.reap_background();
        Ok(if pipeline.negated {
            u8::from(status == 0)
        } else {
            status
        })
    }
    fn redirected_builtin(&mut self, simple: &Simple) -> Result<u8, ExecError> {
        let plan = crate::redirect::prepare(&simple.redirs, &self.env);
        let guard = match sys::FdGuard::save(&plan, self.fd_floor) {
            Ok(guard) => guard,
            Err(error) => {
                let _ = sys::write_fd(2, format!("rsh: redirection: {error}\n").as_bytes());
                if simple.special {
                    self.env.exit_requested = Some(2);
                }
                return Ok(if simple.special { 2 } else { 1 });
            }
        };
        if let Err(failure) = plan.apply() {
            plan.report(failure);
            drop(guard);
            if simple.special {
                self.env.exit_requested = Some(2);
            }
            return Ok(if simple.special { 2 } else { 1 });
        }
        let outcome = self.parent_builtin(simple);
        let output =
            sys::write_fd(1, &outcome.stdout).and_then(|()| sys::write_fd(2, &outcome.stderr));
        if matches!(simple.builtin, Some(Builtin::Exec)) && simple.words.len() == 1 {
            guard.commit();
        } else {
            drop(guard);
        }
        Ok(if output.is_err() { 1 } else { outcome.status })
    }
    fn wait_processes(&mut self, prepared: Vec<PreparedProcess>) -> Result<u8, ExecError> {
        let children = sys::spawn_pipeline(
            &prepared,
            SpawnOptions {
                ignore_int_quit: self.background_signals,
                ..SpawnOptions::default()
            },
        )?;
        let mut last = 0;
        let mut error = None;
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
        Ok(last)
    }
    fn prepare_command(
        &self,
        command: &Command,
        pipe_in: bool,
        pipe_out: bool,
    ) -> Result<PreparedProcess, ExecError> {
        match command {
            Command::Simple(_) => self.prepare(&self.simple(command)?),
            Command::Compound(compound, redirs) => {
                let script = match compound.as_ref() {
                    CompoundCommand::Brace(body) | CompoundCommand::Subshell(body) => body,
                    _ => return Err(ExecError::Unsupported("control flow is TODO(M6)")),
                };
                let plan = crate::redirect::prepare(redirs, &self.env);
                let mut closed = sys::closed_stdio();
                if pipe_in {
                    closed &= !1;
                }
                if pipe_out {
                    closed &= !2;
                }
                let closed = plan.closed_stdio(closed);
                Ok(self
                    .worker_process(script, false, closed, plan.floor())?
                    .with_redirects(plan, 1))
            }
        }
    }
    fn launch_background(&mut self, list: &AndOr) -> Result<u8, ExecError> {
        // Re-exec is essential: a post-fork child must never run the Rust interpreter.
        let script = Script(vec![ListItem {
            and_or: list.clone(),
            separator: Separator::Sequential,
        }]);
        let process = self.worker_process(&script, true, sys::closed_stdio() & !1, 0)?;
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
    fn worker_process(
        &self,
        script: &Script,
        background: bool,
        closed: u8,
        reserved_floor: i32,
    ) -> Result<PreparedProcess, ExecError> {
        let source = sys::here_input(
            &script.to_source_bytes(),
            crate::redirect::source_floor(script, &self.env)
                .max(self.fd_floor)
                .max(reserved_floor),
        )?;
        let source_arg = format!("@{}", source.as_raw_fd()).into_bytes();
        let mut words = vec![
            self.executable.as_os_str().as_bytes().to_vec(),
            if background {
                b"--m3-worker".to_vec()
            } else {
                b"--subshell-worker".to_vec()
            },
            source_arg,
            self.env.last_status.to_string().into_bytes(),
            if self.env.noclobber {
                b"n=1".to_vec()
            } else {
                b"n=0".to_vec()
            },
        ];
        words.push(format!("c={closed}").into_bytes());
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
        Ok(
            PreparedProcess::external(&words, &self.env.exported(), &self.env.cwd, None)?
                .with_redirects(
                    sys::FdPlan {
                        ops: vec![sys::FdOp::Inherit { source }],
                    },
                    1,
                ),
        )
    }
    pub fn reap_background(&mut self) {
        self.background
            .retain(|child| matches!(child.try_wait(), Ok(None)));
    }
    pub fn background_count(&self) -> usize {
        self.background.len()
    }
}
