//! Builtins requiring the interpreter, child ownership, or current-shell replacement.
use super::*;
impl Shell {
    pub(super) fn context_builtin(&mut self, builtin: Builtin, args: &[Vec<u8>]) -> Outcome {
        let result = match builtin {
            Builtin::Eval => {
                let source = args.join(b" ".as_slice());
                if source.is_empty() {
                    Ok(Flow::Status(0))
                } else {
                    Ok(self.source_flow(&source))
                }
            }
            Builtin::Dot => self.dot(args),
            Builtin::Exec => {
                if args.is_empty() {
                    return Outcome::default();
                }
                let mut env = self.env.clone();
                for (name, _) in &self.active_assignments {
                    env.mark_exported(name.clone());
                }
                match PreparedProcess::external(args, &env.exported(), &env.cwd, env.get(b"PATH")) {
                    Ok(p) => {
                        let status = sys::replace_process(&p);
                        self.env.exit_requested = Some(status);
                        Ok(Flow::Exit(status))
                    }
                    Err(e) => Err(e.into()),
                }
            }
            Builtin::Wait => return self.wait_builtin(args),
            Builtin::Type | Builtin::Command => return self.command_builtin(builtin, args),
            _ => Ok(Flow::Status(0)),
        };
        match result {
            Ok(flow) => Outcome {
                status: flow.status(),
                flow: Some(flow),
                ..Outcome::default()
            },
            Err(e) => Outcome::error(b"builtin", e.to_string().as_bytes(), 1),
        }
    }
    fn dot(&mut self, args: &[Vec<u8>]) -> Result<Flow, ExecError> {
        let Some(name) = args.first() else {
            return Err(ExecError::Unsupported(".: filename required"));
        };
        let Some(path) = builtin_impl::misc::find(name, &self.env, false, false) else {
            return Err(ExecError::Unsupported(".: file not found"));
        };
        let source = std::fs::read(path)?;
        if self.env.source_depth >= 64 {
            return Err(ExecError::Unsupported("source nesting limit exceeded"));
        }
        let saved = if args.len() > 1 {
            Some(std::mem::replace(
                &mut self.env.positional,
                args.get(1..).unwrap_or_default().to_vec(),
            ))
        } else {
            None
        };
        self.env.source_depth += 1;
        let flow = self.source_flow(&source);
        self.env.source_depth -= 1;
        if let Some(saved) = saved {
            self.env.positional = saved;
        }
        Ok(if let Flow::Return(s) = flow {
            Flow::Status(s)
        } else {
            flow
        })
    }
    pub fn run_stdin(&mut self) -> u8 {
        let mut source = Vec::new();
        let mut base = 0;
        let mut flow = Flow::Status(0);
        loop {
            let next = match sys::read_byte(0) {
                Ok(b) => b,
                Err(e) => {
                    let _ = sys::write_fd(2, format!("rsh: input: {e}\n").as_bytes());
                    return 1;
                }
            };
            if let Some(b) = next {
                source.push(b);
                if b != b'\n' {
                    continue;
                }
            } else if source.is_empty() {
                break;
            }
            let parsed = crate::parser::parse_at(
                &source,
                if self.env.options.expand_aliases || self.env.options.interactive {
                    Some(&self.env.aliases)
                } else {
                    None
                },
                base,
            );
            match parsed {
                Ok(script) => {
                    self.env.stdin_source = true;
                    let old = self.env.stdin_lines;
                    flow = self.eval_script(&script);
                    self.env.stdin_source = false;
                    base += source.iter().filter(|b| **b == b'\n').count()
                        + self.env.stdin_lines.saturating_sub(old);
                    source.clear();
                    if !matches!(flow, Flow::Status(_)) {
                        break;
                    }
                }
                Err(crate::parser::ParseError::Incomplete(_)) if next.is_some() => continue,
                Err(e) => {
                    let _ = sys::write_fd(2, format!("{e}\n").as_bytes());
                    flow = Flow::Exit(2);
                    break;
                }
            }
            if next.is_none() {
                break;
            }
        }
        self.finish(flow)
    }
    pub fn run_source(&mut self, source: &[u8]) -> u8 {
        let flow = self.source_flow(source);
        self.finish(flow)
    }
    pub(super) fn source_flow(&mut self, source: &[u8]) -> Flow {
        let mut start = 0;
        let mut base = 0;
        let mut flow = Flow::Status(0);
        for end in source
            .iter()
            .enumerate()
            .filter_map(|(i, b)| (*b == b'\n').then_some(i + 1))
            .chain(std::iter::once(source.len()))
            .collect::<Vec<_>>()
        {
            if end <= start {
                continue;
            }
            let chunk = source.get(start..end).unwrap_or_default();
            let parsed = crate::parser::parse_at(
                chunk,
                if self.env.options.expand_aliases || self.env.options.interactive {
                    Some(&self.env.aliases)
                } else {
                    None
                },
                base,
            );
            match parsed {
                Ok(script) => {
                    if !script.0.is_empty()
                        && (!self.env.options.noexec || self.env.options.interactive)
                    {
                        flow = self.eval_script(&script);
                    }
                    base += chunk.iter().filter(|b| **b == b'\n').count();
                    start = end;
                    if !matches!(flow, Flow::Status(_)) {
                        return flow;
                    }
                }
                Err(crate::parser::ParseError::Incomplete(_)) if end < source.len() => {}
                Err(e) => {
                    let _ = sys::write_fd(2, format!("{e}\n").as_bytes());
                    self.env.exit_requested = Some(2);
                    return Flow::Exit(2);
                }
            }
        }
        flow
    }
    fn command_builtin(&mut self, builtin: Builtin, args: &[Vec<u8>]) -> Outcome {
        let mut i = 0;
        let mut verbose = matches!(builtin, Builtin::Type);
        let mut query = verbose;
        let mut default = false;
        if matches!(builtin, Builtin::Command) {
            while let Some(a) = args.get(i) {
                match a.as_slice() {
                    b"-v" => query = true,
                    b"-V" => {
                        query = true;
                        verbose = true;
                    }
                    b"-p" => default = true,
                    b"--" => {
                        i += 1;
                        break;
                    }
                    a if a.starts_with(b"-") => {
                        return Outcome::error(b"command", b"invalid option", 2);
                    }
                    _ => break,
                }
                i += 1;
            }
        }
        let args = args.get(i..).unwrap_or_default();
        if query {
            let mut out = Outcome::default();
            for name in args {
                if let Some(d) = builtin_impl::misc::describe(name, &self.env, verbose, default) {
                    out.stdout.extend(d);
                } else {
                    out.status = 1;
                    if verbose {
                        out.stderr.extend(b"rsh: type: not found\n");
                    }
                }
            }
            return out;
        }
        if args.is_empty() {
            return Outcome::default();
        }
        if self.command_depth >= 64 {
            return Outcome::error(b"command", b"nesting limit exceeded", 2);
        }
        self.command_depth += 1;
        let simple = Simple {
            words: args.to_vec(),
            assignments: Vec::new(),
            builtin: args
                .first()
                .and_then(|n| builtin_impl::special(n).or_else(|| builtin_impl::regular(n))),
            special: false,
            redirs: Vec::new(),
        };
        let result = if simple.builtin.is_some() {
            self.redirected_builtin(&simple)
        } else if default {
            PreparedProcess::external(
                &simple.words,
                &self.env.exported(),
                &self.env.cwd,
                Some(b"/usr/bin:/bin"),
            )
            .map_err(ExecError::from)
            .and_then(|p| self.wait_processes(vec![p]))
            .map(Flow::Status)
        } else {
            self.prepare(&simple)
                .and_then(|p| self.wait_processes(vec![p]))
                .map(Flow::Status)
        };
        self.command_depth -= 1;
        let flow = self.report(result);
        Outcome {
            status: flow.status(),
            flow: Some(flow),
            ..Outcome::default()
        }
    }
    fn wait_pid(&mut self, pid: i32) -> u8 {
        loop {
            if let Some(status) = self.completed.remove(&pid) {
                return status;
            }
            let Some(i) = self.background.iter().position(|c| c.pid() == pid) else {
                return 127;
            };
            match self.background.get(i).map(Child::try_wait) {
                Some(Ok(Some(status))) => {
                    self.background.remove(i);
                    return status;
                }
                Some(Err(_)) => {
                    self.background.remove(i);
                    return 127;
                }
                _ => {}
            }
            if let Some(n) = sys::first_pending_signal() {
                return (128 + n) as u8;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    fn wait_builtin(&mut self, args: &[Vec<u8>]) -> Outcome {
        if args.is_empty() {
            while let Some(pid) = self.background.first().map(Child::pid) {
                let status = self.wait_pid(pid);
                if sys::first_pending_signal().is_some() {
                    return Outcome {
                        status,
                        ..Outcome::default()
                    };
                }
            }
            self.completed.clear();
            return Outcome::default();
        }
        let mut status = 0;
        for arg in args {
            let Some(pid) = std::str::from_utf8(arg)
                .ok()
                .and_then(|s| s.parse::<i32>().ok())
                .filter(|n| *n > 0)
            else {
                return Outcome::error(b"wait", b"invalid pid (%job is M8)", 127);
            };
            status = self.wait_pid(pid);
            if sys::first_pending_signal().is_some() {
                break;
            }
        }
        Outcome {
            status,
            ..Outcome::default()
        }
    }
    pub(super) fn signals_between_commands(&mut self) -> Option<Flow> {
        let signals = sys::pending_signals();
        for n in 1..64 {
            if signals & (1u64 << n) != 0
                && let Some(action) = self.env.traps.get(&n).cloned()
                && !action.is_empty()
            {
                let saved = self.env.last_status;
                let flow = self.source_flow(&action);
                self.env.last_status = saved;
                if !matches!(flow, Flow::Status(_)) {
                    return Some(flow);
                }
            }
        }
        None
    }
    pub(super) fn finish(&mut self, flow: Flow) -> u8 {
        let status = flow.status();
        if let Some(action) = self.env.traps.remove(&0)
            && !action.is_empty()
            && !self.env.trap_listing_only
        {
            self.env.last_status = status;
            self.env.exit_requested = None;
            let flow = self.source_flow(&action);
            if let Flow::Exit(s) = flow {
                return s;
            }
        }
        status
    }
}
