//! AST execution with expansion, redirections, control flow and functions.
mod builtins;
mod conditional;
mod control;
mod interactive;
mod jobs;
use crate::ast::*;
use crate::builtins as builtin_impl;
use crate::builtins::{Builtin, Outcome};
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

/// Structured non-local control is propagated through AST execution boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Status(u8),
    Break(usize),
    Continue(usize),
    Return(u8),
    Exit(u8),
}
impl Flow {
    pub fn status(self) -> u8 {
        match self {
            Self::Status(s) | Self::Return(s) | Self::Exit(s) => s,
            Self::Break(_) | Self::Continue(_) => 0,
        }
    }
}
pub const MAX_FUNCTION_DEPTH: usize = 32;

/// Serial interpreter state, including ownership of child statuses and terminal state.
/// Multiple values are not independent OS environments: fds and signals are global.
pub struct Shell {
    pub env: ShellEnv,
    executable: PathBuf,
    background: Vec<Child>,
    jobs: crate::jobs::Table,
    terminal: Option<sys::Terminal>,
    terminal_failed: bool,
    exit_warned: bool,
    command_text: Vec<u8>,
    completed_jobs: std::collections::BTreeMap<usize, u8>,
    fd_floor: i32,
    loop_depth: usize,
    eval_depth: usize,
    pub(super) ignored_errexit: usize,
    motd_startup: bool,
    abort_line: bool,
    command_depth: usize,
    completed: std::collections::BTreeMap<i32, u8>,
    active_assignments: Vec<(Vec<u8>, Vec<u8>)>,
    pub background_signals: bool,
    pub direct_worker: bool,
    pub(crate) startup_context: Option<PathBuf>,
    pub(crate) history:
        Option<std::sync::Arc<std::sync::Mutex<crate::interactive::history::Store>>>,
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
        let ignored_errexit = usize::from(env.errexit_ignored);
        Self {
            env,
            executable,
            background: Vec::new(),
            jobs: crate::jobs::Table::default(),
            terminal: None,
            terminal_failed: false,
            exit_warned: false,
            command_text: Vec::new(),
            completed_jobs: std::collections::BTreeMap::new(),
            fd_floor: 20,
            loop_depth: 0,
            eval_depth: 0,
            ignored_errexit,
            motd_startup: false,
            abort_line: false,
            command_depth: 0,
            completed: std::collections::BTreeMap::new(),
            active_assignments: Vec::new(),
            background_signals: false,
            direct_worker: false,
            history: None,
            startup_context: None,
        }
    }
    pub fn run(&mut self, script: &Script) -> u8 {
        self.direct_worker = self.direct_worker
            && matches!(script.0.as_slice(),[item]
            if item.and_or.rest.is_empty() && !item.and_or.first.negated
            && matches!(item.and_or.first.commands.as_slice(),[Command::Simple(_)]));
        let flow = self.eval_script(script);
        self.finish(flow)
    }
    // failglob aborts the current parsed input unit, not all subsequent input.
    // Nested sourced files must unwind first so positional parameters/fds restore.
    fn recover_line_abort(&mut self, flow: Flow) -> Flow {
        if self.abort_line && self.env.source_depth == 0 {
            self.abort_line = false;
            self.env.exit_requested = None;
            self.env.last_status = flow.status();
            Flow::Status(flow.status())
        } else {
            flow
        }
    }
    fn eval_script(&mut self, script: &Script) -> Flow {
        if self.eval_depth >= 128 {
            self.env.exit_requested = Some(2);
            let _ = sys::write_fd(2, b"rsh: execution: evaluation nesting limit exceeded\n");
            return Flow::Exit(2);
        }
        self.eval_depth += 1;
        self.fd_floor = self
            .fd_floor
            .max(crate::redirect::source_floor(script, &self.env));
        let mut flow = Flow::Status(self.env.last_status);
        for item in &script.0 {
            if self.env.options.noexec && !self.env.options.interactive {
                break;
            }
            if let Some(signal) = self.signals_between_commands() {
                flow = signal;
                break;
            }
            self.reap_background();
            flow = if item.separator == Separator::Background {
                let result = self.launch_background(&item.and_or).map(Flow::Status);
                self.report(result)
            } else {
                self.and_or(&item.and_or)
            };
            self.env.last_status = flow.status();
            if !matches!(flow, Flow::Status(_)) {
                break;
            }
        }
        self.reap_background();
        self.eval_depth -= 1;
        flow
    }
    fn report(&self, result: Result<Flow, ExecError>) -> Flow {
        if let Some(status) = self.env.exit_requested {
            if let Err(e) = result {
                let _ = sys::write_fd(2, format!("{e}\n").as_bytes());
            }
            return Flow::Exit(status);
        }
        match result {
            Ok(flow) => flow,
            Err(error) => {
                let diagnostic = if let ExecError::Io(e) = &error {
                    format!("rsh: execution: {e}\n")
                } else {
                    format!("{error}\n")
                };
                let _ = sys::write_fd(2, diagnostic.as_bytes());
                Flow::Status(if matches!(error, ExecError::Unsupported(_)) {
                    2
                } else {
                    1
                })
            }
        }
    }
    fn and_or(&mut self, list: &AndOr) -> Flow {
        let mut flow = Flow::Status(0);
        for (i, pipe) in std::iter::once(&list.first)
            .chain(list.rest.iter().map(|(_, p)| p))
            .enumerate()
        {
            if i > 0
                && let Some((op, _)) = list.rest.get(i - 1)
                && (matches!(op, AndOrOp::And) != (self.env.last_status == 0))
            {
                continue;
            }
            let exempt = i < list.rest.len() || pipe.negated;
            if exempt {
                self.ignored_errexit += 1;
            }
            let result = self.pipeline(pipe);
            flow = self.report(result);
            self.env.last_status = flow.status();
            if exempt {
                self.ignored_errexit -= 1;
            }
            if !matches!(flow, Flow::Status(_)) {
                break;
            }
            if !exempt
                && self.ignored_errexit == 0
                && self.env.options.errexit
                && flow.status() != 0
            {
                self.env.exit_requested = Some(flow.status());
                flow = Flow::Exit(flow.status());
                break;
            }
            if let Some(signal) = self.signals_between_commands() {
                flow = signal;
                break;
            }
        }
        flow
    }
    fn simple(&mut self, command: &Command) -> Result<Simple, ExecError> {
        let Command::Simple(simple) = command else {
            return Err(ExecError::Unsupported("expected a simple command"));
        };
        self.env
            .set(b"LINENO".to_vec(), simple.line.to_string().into_bytes());
        let mut words = Vec::new();
        let mut assignments = Vec::new();

        let mut redirs = Vec::new();
        for part in &simple.parts {
            match part {
                SimplePart::Word(word) => {
                    let declaration = words
                        .first()
                        .is_some_and(|w: &Vec<u8>| w == b"export" || w == b"readonly");
                    let assignment = matches!(word.0.first(),Some(WordPart::Literal(v)) if v.iter().position(|b|*b==b'=').is_some_and(|i|crate::env::valid_name(v.get(..i).unwrap_or_default())));
                    if declaration && assignment {
                        if let Some(a) = crate::parser::assignment(word) {
                            let value =
                                self.expand_scalar(&a.value, crate::expand::Mode::Assignment)?;
                            words.push([a.name.as_slice(), b"=", value.as_slice()].concat());
                        }
                    } else {
                        words.extend(self.expand(word, crate::expand::Mode::Argument)?);
                    }
                }
                SimplePart::Assignment(_) => {}
                SimplePart::Redirection(redirection) => {
                    redirs.push(self.expand_redirect(redirection)?)
                }
            }
        }
        let mut saved = Vec::new();
        let result = (|| -> Result<(), ExecError> {
            for part in &simple.parts {
                if let SimplePart::Assignment(a) = part {
                    if self.env.is_readonly(&a.name) {
                        self.env.exit_requested = Some(1);
                        return Err(crate::expand::ExpandError::new(format!(
                            "{}: readonly variable",
                            String::from_utf8_lossy(&a.name)
                        ))
                        .into());
                    }
                    let value = self.expand_scalar(&a.value, crate::expand::Mode::Assignment)?;
                    saved.push((a.name.clone(), self.env.variables.get(&a.name).cloned()));
                    self.env.set(a.name.clone(), value.clone());
                    assignments.push((a.name.clone(), value));
                }
            }
            Ok(())
        })();
        for (name, value) in saved.into_iter().rev() {
            if let Some(v) = value {
                self.env.variables.insert(name, v);
            } else {
                self.env.variables.remove(&name);
            }
        }
        result?;
        let name = words.first().map(Vec::as_slice).unwrap_or_default();
        if self.env.options.xtrace {
            let ps4 = self.env.get(b"PS4").unwrap_or(b"+ ").to_vec();
            let parts = crate::lexer::here_parts(&ps4, 0)
                .map_err(|e| crate::expand::ExpandError::new(e.to_string()))?;
            self.env.options.xtrace = false;
            let prefix = self.expand_scalar(&Word(parts), crate::expand::Mode::HereDoc);
            self.env.options.xtrace = true;
            let mut trace = prefix?;
            let mut items = assignments
                .iter()
                .map(|(k, v)| [k.as_slice(), b"=", v.as_slice()].concat())
                .collect::<Vec<_>>();
            items.extend(words.clone());
            trace.extend(items.join(b" ".as_slice()));
            trace.push(b'\n');
            let _ = sys::write_fd(2, &trace);
        }
        let special = builtin_impl::special(name);
        let builtin = special.or_else(|| {
            if self.env.functions.contains_key(name) {
                None
            } else {
                builtin_impl::regular(name)
            }
        });
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
            if !simple.special && !simple.words.is_empty() {
                self.env.mark_exported(name.clone());
            }
        }
        self.active_assignments = simple.assignments.clone();
        let outcome = if matches!(simple.builtin, Some(Builtin::Exit)) && self.stopped_exit() {
            Outcome::default()
        } else if let Some(builtin) = simple.builtin {
            if matches!(
                builtin,
                Builtin::Dot
                    | Builtin::Source
                    | Builtin::Eval
                    | Builtin::Exec
                    | Builtin::Type
                    | Builtin::Command
                    | Builtin::Wait
                    | Builtin::Jobs
                    | Builtin::Fg
                    | Builtin::Bg
                    | Builtin::Disown
                    | Builtin::Kill
                    | Builtin::History
            ) {
                self.context_builtin(builtin, simple.words.get(1..).unwrap_or_default())
            } else if matches!(
                builtin,
                Builtin::Break | Builtin::Continue | Builtin::Return
            ) {
                builtin_impl::control(
                    builtin,
                    simple.words.get(1..).unwrap_or_default(),
                    &mut self.env,
                    self.loop_depth,
                )
            } else {
                builtin_impl::run(
                    builtin,
                    simple.words.get(1..).unwrap_or_default(),
                    &mut self.env,
                )
            }
        } else {
            Outcome {
                status: self.env.substitution_status.unwrap_or(0),
                ..Outcome::default()
            }
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
                builtin_impl::run(
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
    fn pipeline(&mut self, pipeline: &Pipeline) -> Result<Flow, ExecError> {
        self.reap_background();
        let previous = std::mem::replace(
            &mut self.command_text,
            Script(vec![ListItem {
                and_or: AndOr {
                    first: pipeline.clone(),
                    rest: Vec::new(),
                },
                separator: Separator::Sequential,
            }])
            .to_source_bytes()
            .trim_ascii_end()
            .strip_suffix(b";")
            .unwrap_or_default()
            .to_vec(),
        );
        let flow = if let [command] = pipeline.commands.as_slice() {
            match command {
                Command::FunctionDef { name, definition } => {
                    self.env
                        .functions
                        .insert(name.clone(), std::rc::Rc::new(definition.clone()));
                    Flow::Status(0)
                }
                Command::Compound(compound, redirs)
                    if !matches!(compound.as_ref(), CompoundCommand::Subshell(_)) =>
                {
                    self.redirected_compound(compound, redirs)?
                }
                Command::Simple(_) => {
                    self.env.substitution_status = None;
                    let simple = self.simple(command)?;
                    if simple.builtin.is_some() || simple.words.is_empty() {
                        self.redirected_builtin(&simple)?
                    } else if let Some(function) = simple
                        .words
                        .first()
                        .and_then(|n| self.env.functions.get(n))
                        .cloned()
                    {
                        self.call_function(&simple, &function)?
                    } else {
                        let process = self.prepare(&simple)?;
                        Flow::Status(if self.direct_worker && self.eval_depth == 1 {
                            sys::replace_process(&process)
                        } else {
                            self.wait_processes(vec![process])?
                        })
                    }
                }
                _ => {
                    let process = self.prepare_command(command, false, false)?;
                    Flow::Status(self.wait_processes(vec![process])?)
                }
            }
        } else {
            let processes = pipeline
                .commands
                .iter()
                .enumerate()
                .map(|(i, c)| self.prepare_command(c, i > 0, i + 1 < pipeline.commands.len()))
                .collect::<Result<Vec<_>, _>>()?;
            Flow::Status(self.wait_processes(processes)?)
        };
        self.command_text = previous;
        self.reap_background();
        // Startup cancellation must propagate before a `!` can negate SIGINT.
        if self.motd_startup && flow.status() == (128 + sys::INT) as u8 {
            return Ok(Flow::Exit(flow.status()));
        }
        Ok(if let Flow::Status(s) = flow {
            Flow::Status(if pipeline.negated {
                u8::from(s == 0)
            } else {
                s
            })
        } else {
            flow
        })
    }
    fn redirected_builtin(&mut self, simple: &Simple) -> Result<Flow, ExecError> {
        let plan = crate::redirect::prepare(&simple.redirs, &self.env);
        let guard = match sys::FdGuard::save(&plan, self.fd_floor) {
            Ok(guard) => guard,
            Err(error) => {
                let _ = sys::write_fd(2, format!("rsh: redirection: {error}\n").as_bytes());
                if simple.special {
                    self.env.exit_requested = Some(2);
                }
                return Ok(if simple.special {
                    Flow::Exit(2)
                } else {
                    Flow::Status(1)
                });
            }
        };
        if let Err(failure) = plan.apply() {
            plan.report(failure);
            drop(guard);
            if simple.special {
                self.env.exit_requested = Some(2);
            }
            return Ok(if simple.special {
                Flow::Exit(2)
            } else {
                Flow::Status(1)
            });
        }
        let outcome = self.parent_builtin(simple);
        if simple.special && outcome.status != 0 && !outcome.stderr.is_empty() {
            self.env.exit_requested = Some(outcome.status);
        }

        let output =
            sys::write_fd(1, &outcome.stdout).and_then(|()| sys::write_fd(2, &outcome.stderr));
        if matches!(simple.builtin, Some(Builtin::Exec)) && simple.words.len() == 1 {
            guard.commit();
        } else {
            drop(guard);
        }
        Ok(if let Some(s) = self.env.exit_requested {
            Flow::Exit(s)
        } else if output.is_err() {
            Flow::Status(1)
        } else {
            outcome.flow.unwrap_or(Flow::Status(outcome.status))
        })
    }
    fn wait_processes(&mut self, prepared: Vec<PreparedProcess>) -> Result<u8, ExecError> {
        if self.monitor() {
            for process in &prepared {
                process.protect_resources()?;
            }
            let fd = self.terminal.as_ref().map(sys::Terminal::fd);
            let children = sys::spawn_pipeline(
                &prepared,
                SpawnOptions {
                    pgid: Some(0),
                    foreground_fd: fd,
                    job_signals: true,
                    ..SpawnOptions::default()
                },
            )?;
            let pids: Vec<_> = children.iter().map(Child::pid).collect();
            let pgid = pids.first().copied().unwrap_or(0);
            let id = self.jobs.insert(
                pids,
                pgid,
                self.command_text.clone(),
                false,
                self.env.options.pipefail,
            );
            return self.job_foreground(id, false);
        }
        if !self.env.options.interactive {
            sys::watch_interrupt()?;
        }
        let children = sys::spawn_pipeline(
            &prepared,
            SpawnOptions {
                ignore_int_quit: self.background_signals,
                ..SpawnOptions::default()
            },
        )?;
        let mut last = 0;
        let mut failure = 0;
        let mut interrupted_child = false;
        let mut error = None;
        for child in children {
            match child.wait_kind() {
                Ok(event) => {
                    let status = event.status();
                    last = status;
                    interrupted_child |= event == sys::ProcessEvent::Signaled(sys::INT);
                    if status != 0 {
                        failure = status;
                    }
                }
                Err(e) => {
                    error.get_or_insert(e);
                }
            }
        }
        if let Some(error) = error {
            return Err(error.into());
        }
        if !self.env.traps.contains_key(&sys::INT) && sys::take_interrupt() && interrupted_child {
            self.env.exit_requested = Some((128 + sys::INT) as u8);
        }
        Ok(if self.env.options.pipefail {
            failure
        } else {
            last
        })
    }
    fn prepare_command(
        &mut self,
        command: &Command,
        pipe_in: bool,
        pipe_out: bool,
    ) -> Result<PreparedProcess, ExecError> {
        let command = if let Command::Compound(compound, redirs) = command
            && let CompoundCommand::Subshell(body) = compound.as_ref()
        {
            Command::Compound(
                Box::new(CompoundCommand::Brace(body.clone())),
                redirs.clone(),
            )
        } else {
            command.clone()
        };
        // Expansions and their variable side effects belong to each pipeline/subshell
        // process. Re-exec before interpreting Rust, keeping the fork path signal-safe.
        let script = Script(vec![ListItem {
            separator: Separator::Sequential,
            and_or: AndOr {
                first: Pipeline {
                    negated: false,
                    commands: vec![command],
                },
                rest: Vec::new(),
            },
        }]);
        let mut closed = sys::closed_stdio();
        if pipe_in {
            closed &= !1;
        }
        if pipe_out {
            closed &= !2;
        }
        self.worker_process(&script, false, closed, 0)
    }
    fn launch_background(&mut self, list: &AndOr) -> Result<u8, ExecError> {
        // Re-exec is essential: a post-fork child must never run the Rust interpreter.
        let script = Script(vec![ListItem {
            and_or: list.clone(),
            separator: Separator::Sequential,
        }]);
        if self.monitor() {
            let processes = if list.rest.is_empty() && !list.first.negated {
                list.first
                    .commands
                    .iter()
                    .enumerate()
                    .map(|(i, c)| self.prepare_command(c, i > 0, i + 1 < list.first.commands.len()))
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                vec![self.worker_process(&script, false, sys::closed_stdio(), 0)?]
            };
            let children = sys::spawn_pipeline(
                &processes,
                SpawnOptions {
                    pgid: Some(0),
                    job_signals: true,
                    ..SpawnOptions::default()
                },
            )?;
            let pids: Vec<_> = children.iter().map(Child::pid).collect();
            let pgid = pids.first().copied().unwrap_or(0);
            self.env.last_background = pids.last().copied();
            let text = script.to_source_bytes();
            let id = self.jobs.insert(
                pids,
                pgid,
                text.strip_suffix(b";\n").unwrap_or(&text).to_vec(),
                true,
                self.env.options.pipefail,
            );
            let _ = sys::write_fd(
                2,
                format!("[{id}] {}\n", self.env.last_background.unwrap_or(pgid)).as_bytes(),
            );
            return Ok(0);
        }
        let process = self.worker_process(&script, true, sys::closed_stdio() & !1, 0)?;
        let children = sys::spawn_pipeline(
            &[process],
            SpawnOptions {
                detached: true,
                null_stdin: true,
                ignore_int_quit: true,
                ..SpawnOptions::default()
            },
        )?;
        self.env.last_background = children.last().map(Child::pid);
        let pids: Vec<_> = children.iter().map(Child::pid).collect();
        let pgid = pids.first().copied().unwrap_or(0);
        self.jobs.insert(
            pids,
            pgid,
            script
                .to_source_bytes()
                .trim_ascii_end()
                .strip_suffix(b";")
                .unwrap_or_default()
                .to_vec(),
            true,
            self.env.options.pipefail,
        );
        Ok(0)
    }
    fn worker_process(
        &self,
        script: &Script,
        background: bool,
        closed: u8,
        reserved_floor: i32,
    ) -> Result<PreparedProcess, ExecError> {
        let mut words = vec![if self.env.noclobber {
            b"n=1".to_vec()
        } else {
            b"n=0".to_vec()
        }];
        words.push(format!("c={closed}").into_bytes());
        if let Some(history) = &self.history
            && let Ok(history) = history.lock()
        {
            words.push(
                [
                    b"H=".as_slice(),
                    &crate::interactive::history::encode(&history.entries),
                ]
                .concat(),
            );
        }
        if background || (self.env.options.interactive && self.env.options.monitor) {
            words.push(b"J=1".to_vec());
        }
        words.push([b"d=".as_slice(), self.env.cwd.as_os_str().as_bytes()].concat());
        words.push(format!("g={}", u8::from(self.env.noglob)).into_bytes());
        words.push(format!("p={}", self.env.shell_pid).into_bytes());
        words.push([b"z=".as_slice(), self.env.arg0.as_slice()].concat());
        if let Some(pid) = self.env.last_background {
            words.push(format!("b={pid}").into_bytes());
        }
        for arg in &self.env.positional {
            words.push([b"a=".as_slice(), arg.as_slice()].concat());
        }
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
        words.push(format!("h={}", self.env.function_depth).into_bytes());
        for (name, function) in &self.env.functions {
            let script = Script(vec![ListItem {
                separator: Separator::Sequential,
                and_or: AndOr {
                    first: Pipeline {
                        negated: false,
                        commands: vec![Command::FunctionDef {
                            name: name.clone(),
                            definition: function.as_ref().clone(),
                        }],
                    },
                    rest: Vec::new(),
                },
            }]);
            let data = crate::env::encode_snapshot(&[
                crate::parser::locations::lines(&script),
                script.to_source_bytes(),
            ]);
            words.push([b"f=".as_slice(), data.as_slice()].concat());
        }
        for name in crate::env::options::NAMES {
            words.push(
                format!(
                    "O={name}={}",
                    u8::from(*name != "monitor" && self.env.option(name).unwrap_or(false))
                )
                .into_bytes(),
            );
        }
        words.push(format!("E={}", u8::from(self.ignored_errexit > 0)).into_bytes());
        words.push(format!("s={}", self.env.source_depth).into_bytes());
        for (n, v) in &self.env.variables {
            if v.readonly {
                words.push([b"r=".as_slice(), n].concat());
            }
        }
        for (n, v) in &self.env.aliases {
            words.push([b"A=".as_slice(), n, b"=", v].concat());
        }
        let trap_only = script.0.len() == 1
            && matches!(script.0.first().and_then(|i|i.and_or.first.commands.first()),Some(Command::Simple(s)) if matches!(s.parts.as_slice(),[SimplePart::Word(Word(p))] if matches!(p.as_slice(),[WordPart::Literal(n)] if n==b"trap")));
        for (n, v) in &self.env.traps {
            if v.is_empty() || trap_only {
                words.push([format!("t={n}=").as_bytes(), v].concat());
            }
        }
        if trap_only {
            words.push(b"T=1".to_vec());
        }
        words.push([b"L=".as_slice(), &crate::parser::locations::lines(script)].concat());
        words.push(script.to_source_bytes());
        let payload = crate::env::encode_snapshot(&words);
        let source = sys::here_input(
            &payload,
            crate::redirect::source_floor(script, &self.env)
                .max(self.fd_floor)
                .max(reserved_floor),
        )?;
        let args = vec![
            self.executable.as_os_str().as_bytes().to_vec(),
            if background {
                b"--m3-worker".to_vec()
            } else {
                b"--subshell-worker".to_vec()
            },
            format!("@{}", source.as_raw_fd()).into_bytes(),
            self.env.last_status.to_string().into_bytes(),
        ];
        Ok(
            PreparedProcess::external(&args, &[], &self.env.cwd, None)?.with_redirects(
                sys::FdPlan {
                    ops: vec![sys::FdOp::Inherit { source }],
                },
                1,
            ),
        )
    }
    fn expand(
        &mut self,
        word: &Word,
        mode: crate::expand::Mode,
    ) -> Result<Vec<Vec<u8>>, ExecError> {
        let executable = self.executable.clone();
        let floor = self.fd_floor;
        let result = crate::expand::word(
            word,
            &mut self.env,
            &mut |source, env| {
                let mut local = env.clone();
                local.options.interactive = false;
                local.options.monitor = false;
                let mut shell = Shell::new(local, executable.clone());
                shell.fd_floor = floor;
                shell
                    .substitute(source)
                    .map_err(|e| crate::expand::ExpandError::new(e.to_string()))
            },
            mode,
        );
        if let Err(error) = &result {
            self.abort_line |= error.abort_line;
            self.env.exit_requested = Some(error.status);
        }
        Ok(result?)
    }
    fn expand_scalar(
        &mut self,
        word: &Word,
        mode: crate::expand::Mode,
    ) -> Result<Vec<u8>, ExecError> {
        Ok(self.expand(word, mode)?.join(b" ".as_slice()))
    }
    fn expand_redirect(&mut self, redir: &Redirection) -> Result<Redirection, ExecError> {
        if matches!(redir.op, RedirOp::HereString) && !self.env.options.here_strings {
            return Err(ExecError::Unsupported(
                "here-strings are disabled (set -o here_strings)",
            ));
        }
        let mut r = redir.clone();
        if let RedirOp::HereDoc(doc) = &mut r.op {
            if !doc.quoted {
                doc.raw =
                    self.expand_scalar(&Word(doc.parts.clone()), crate::expand::Mode::HereDoc)?;
                doc.quoted = true;
            }
        } else {
            r.target = Word(vec![WordPart::Literal(self.expand_scalar(
                &r.target,
                if matches!(r.op, RedirOp::HereString) {
                    crate::expand::Mode::HereString
                } else {
                    crate::expand::Mode::Scalar
                },
            )?)]);
        }
        Ok(r)
    }
    // File-backed capture lets the worker finish before reading without the
    // pipe-capacity deadlock a wait-before-drain implementation would create.
    fn substitute(&mut self, source: &[u8]) -> Result<(Vec<u8>, u8), ExecError> {
        use std::io::{Read, Seek};
        let script = crate::parser::parse(source)
            .map_err(|_| ExecError::Unsupported("invalid command substitution"))?;
        if script.0.is_empty() {
            return Ok((Vec::new(), 0));
        }
        let floor = crate::redirect::source_floor(&script, &self.env).max(self.fd_floor);
        let mut file = sys::capture_file(floor)?;
        let plan = sys::FdPlan {
            ops: vec![sys::FdOp::Dup {
                fd: 1,
                source: file.as_raw_fd(),
                context: b"command substitution".to_vec(),
            }],
        };
        let process = self
            .worker_process(
                &script,
                false,
                sys::closed_stdio() & !2,
                floor.max(file.as_raw_fd().saturating_add(1)),
            )?
            .with_redirects(plan, 1);
        let status = self.wait_processes(vec![process])?;
        file.rewind()?;
        let mut out = Vec::new();
        file.read_to_end(&mut out)?;
        Ok((out, status))
    }
    pub fn reap_background(&mut self) {
        self.jobs.reap();
        let mut active = Vec::new();
        for child in self.background.drain(..) {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.completed.insert(child.pid(), status);
                }
                Ok(None) => active.push(child),
                Err(_) => {}
            }
        }
        self.background = active;
    }
    pub fn background_count(&self) -> usize {
        self.background.len()
            + self
                .jobs
                .jobs
                .values()
                .filter(|j| j.state() != crate::jobs::State::Done)
                .count()
    }
}
