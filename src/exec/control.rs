//! Compound execution and the boundaries that consume structured control flow.
use super::*;

enum LoopAction {
    Next,
    Stop,
    Propagate(Flow),
}
fn loop_action(flow: Flow, status: &mut u8) -> LoopAction {
    match flow {
        Flow::Status(s) => {
            *status = s;
            LoopAction::Next
        }
        Flow::Break(1) => {
            *status = 0;
            LoopAction::Stop
        }
        Flow::Break(n) => LoopAction::Propagate(Flow::Break(n.saturating_sub(1))),
        Flow::Continue(1) => {
            *status = 0;
            LoopAction::Next
        }
        Flow::Continue(n) => LoopAction::Propagate(Flow::Continue(n.saturating_sub(1))),
        flow => LoopAction::Propagate(flow),
    }
}
impl Shell {
    pub(super) fn redirected_compound(
        &mut self,
        compound: &CompoundCommand,
        redirs: &[Redirection],
    ) -> Result<Flow, ExecError> {
        let redirs = redirs
            .iter()
            .map(|r| self.expand_redirect(r))
            .collect::<Result<Vec<_>, _>>()?;
        let plan = crate::redirect::prepare(&redirs, &self.env);
        let guard = sys::FdGuard::save(&plan, self.fd_floor)?;
        let result = if let Err(failure) = plan.apply() {
            plan.report(failure);
            Ok(Flow::Status(1))
        } else {
            self.compound(compound)
        };
        drop(guard);
        result
    }
    fn compound(&mut self, compound: &CompoundCommand) -> Result<Flow, ExecError> {
        Ok(match compound {
            CompoundCommand::Conditional(expr) => return self.eval_conditional(expr),
            CompoundCommand::Brace(body) => self.eval_script(body),
            CompoundCommand::Subshell(body) => {
                let process = self.worker_process(body, false, sys::closed_stdio(), 0)?;
                Flow::Status(self.wait_processes(vec![process])?)
            }
            CompoundCommand::If {
                branches,
                else_body,
            } => {
                for branch in branches {
                    match self.condition(&branch.condition) {
                        Flow::Status(0) => return Ok(self.eval_script(&branch.body)),
                        Flow::Status(_) => {}
                        flow => return Ok(flow),
                    }
                }
                if let Some(body) = else_body {
                    self.eval_script(body)
                } else {
                    Flow::Status(0)
                }
            }
            CompoundCommand::While { condition, body } => self.while_loop(condition, body, false),
            CompoundCommand::Until { condition, body } => self.while_loop(condition, body, true),
            CompoundCommand::For { name, words, body } => {
                let values = if let Some(words) = words {
                    let mut values = Vec::new();
                    for word in words {
                        values.extend(self.expand(word, crate::expand::Mode::Argument)?);
                    }
                    values
                } else {
                    self.env.positional.clone()
                };
                self.for_loop(name, &values, body)
            }
            CompoundCommand::Case { word, items } => {
                let value = self.expand_scalar(word, crate::expand::Mode::Scalar)?;
                for item in items {
                    for pattern in &item.patterns {
                        let pattern = self.case_pattern(pattern)?;
                        if crate::expand::pattern::matches_case(
                            &pattern,
                            &value,
                            self.env.options.nocasematch,
                        ) {
                            return Ok(if item.body.0.is_empty() {
                                Flow::Status(0)
                            } else {
                                self.eval_script(&item.body)
                            });
                        }
                    }
                }
                Flow::Status(0)
            }
        })
    }
    fn condition(&mut self, script: &Script) -> Flow {
        self.ignored_errexit += 1;
        let flow = self.eval_script(script);
        self.ignored_errexit -= 1;
        flow
    }
    fn while_loop(&mut self, condition: &Script, body: &Script, until: bool) -> Flow {
        self.loop_depth += 1;
        let mut status = 0;
        let result = loop {
            match self.condition(condition) {
                Flow::Status(s) => {
                    if (s == 0) == until {
                        break Flow::Status(status);
                    }
                }
                flow => match loop_action(flow, &mut status) {
                    LoopAction::Next => continue,
                    LoopAction::Stop => break Flow::Status(status),
                    LoopAction::Propagate(f) => break f,
                },
            }
            match loop_action(self.eval_script(body), &mut status) {
                LoopAction::Next => {}
                LoopAction::Stop => break Flow::Status(status),
                LoopAction::Propagate(f) => break f,
            }
        };
        self.loop_depth -= 1;
        result
    }
    fn for_loop(&mut self, name: &[u8], values: &[Vec<u8>], body: &Script) -> Flow {
        self.loop_depth += 1;
        let mut status = 0;
        let mut result = None;
        for value in values {
            if let Err(error) = self.env.assign(name.to_vec(), value.clone()) {
                let _ = sys::write_fd(2, format!("rsh: {error}\n").as_bytes());
                self.env.exit_requested = Some(1);
                result = Some(Flow::Exit(1));
                break;
            }
            match loop_action(self.eval_script(body), &mut status) {
                LoopAction::Next => {}
                LoopAction::Stop => break,
                LoopAction::Propagate(f) => {
                    result = Some(f);
                    break;
                }
            }
        }
        self.loop_depth -= 1;
        result.unwrap_or(Flow::Status(status))
    }
    pub(super) fn case_pattern(
        &mut self,
        word: &Word,
    ) -> Result<Vec<crate::expand::pattern::Atom>, ExecError> {
        let executable = self.executable.clone();
        let floor = self.fd_floor;
        let result = crate::expand::compile_pattern(word, &mut self.env, &mut |source, env| {
            let mut shell = Shell::new(env.clone(), executable.clone());
            shell.fd_floor = floor;
            shell
                .substitute(source)
                .map_err(|e| crate::expand::ExpandError::new(e.to_string()))
        });
        if let Err(error) = &result {
            self.env.exit_requested = Some(error.status);
        }
        Ok(result?)
    }
    pub(super) fn call_function(
        &mut self,
        simple: &Simple,
        function: &Function,
    ) -> Result<Flow, ExecError> {
        if self.env.function_depth >= MAX_FUNCTION_DEPTH {
            self.env.exit_requested = Some(2);
            return Err(ExecError::Unsupported(
                "function recursion depth limit exceeded",
            ));
        }
        let positional = std::mem::replace(
            &mut self.env.positional,
            simple.words.get(1..).unwrap_or_default().to_vec(),
        );
        let loops = std::mem::replace(&mut self.loop_depth, 0);
        let saved = simple
            .assignments
            .iter()
            .map(|(name, _)| (name.clone(), self.env.variables.get(name).cloned()))
            .collect::<Vec<_>>();
        for (name, value) in &simple.assignments {
            self.env.set(name.clone(), value.clone());
            self.env.mark_exported(name.clone());
        }
        self.env.function_depth += 1;
        let result = (|| {
            let plan = crate::redirect::prepare(&simple.redirs, &self.env);
            let guard = sys::FdGuard::save(&plan, self.fd_floor)?;
            let result = if let Err(failure) = plan.apply() {
                plan.report(failure);
                Ok(Flow::Status(1))
            } else {
                self.redirected_compound(&function.body, &function.redirs)
            };
            drop(guard);
            result
        })();
        self.env.function_depth -= 1;
        self.env.positional = positional;
        self.loop_depth = loops;
        for (name, value) in saved {
            if let Some(value) = value {
                self.env.variables.insert(name, value);
            } else {
                self.env.variables.remove(&name);
            }
        }
        result.map(|flow| {
            if let Flow::Return(s) = flow {
                Flow::Status(s)
            } else {
                flow
            }
        })
    }
}
