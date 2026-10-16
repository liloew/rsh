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
                        sys::reset_shell_signals();
                        let status = sys::replace_process(&p);
                        self.env.exit_requested = Some(status);
                        Ok(Flow::Exit(status))
                    }
                    Err(e) => Err(e.into()),
                }
            }
            Builtin::Jobs | Builtin::Fg | Builtin::Bg | Builtin::Disown => {
                return self.jobs_builtin(builtin, args);
            }
            Builtin::Kill => return self.kill_builtin(args),
            Builtin::History => {
                let Some(history) = &self.history else {
                    return Outcome::default();
                };
                let Ok(mut history) = history.lock() else {
                    return Outcome::error(b"history", b"history lock poisoned", 1);
                };
                if args == [b"-c".to_vec()] {
                    return match history.clear() {
                        Ok(()) => Outcome::default(),
                        Err(e) => Outcome::error(b"history", e.to_string().as_bytes(), 1),
                    };
                }
                if !args.is_empty() {
                    return Outcome::error(b"history", b"usage: history [-c]", 2);
                }
                let stdout = history
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| format!("{}  {}\n", i + 1, e.command))
                    .collect::<String>()
                    .into_bytes();
                return Outcome {
                    stdout,
                    ..Outcome::default()
                };
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
        let source = std::fs::read(&path)?;
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
        let startup_context = self.startup_context.clone();
        if startup_context.is_some() {
            self.startup_context = Some(path);
        }
        self.env.source_depth += 1;
        let flow = self.source_flow(&source);
        self.env.source_depth -= 1;
        // Preserve the failing inner file until startup emits its final summary.
        if !matches!(flow, Flow::Exit(_)) {
            self.startup_context = startup_context;
        }
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
        let _ = self.monitor();
        let mut prompted = false;
        loop {
            if self.env.options.interactive && source.is_empty() && !prompted {
                if self.env.options.monitor && self.terminal.is_some() {
                    self.notify_jobs();
                }
                let _ = self.signals_between_commands();
                let _ = sys::write_fd(2, self.env.get(b"PS1").unwrap_or(b"$ "));
                prompted = true;
            }
            let next = match sys::read_byte(0) {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    let notified =
                        self.env.options.monitor && self.terminal.is_some() && self.notify_jobs();
                    let pending = sys::pending_signal_mask();
                    let trapped = self.env.traps.iter().any(|(n, a)| {
                        *n > 0 && *n < 64 && pending & (1u64 << n) != 0 && !a.is_empty()
                    });
                    if let Some(signal) = self.signals_between_commands() {
                        flow = signal;
                        break;
                    }
                    if self.env.options.interactive && (notified || trapped) {
                        prompted = false;
                    }
                    continue;
                }
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
            if next.is_some() && continued_line(&source) {
                continue;
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
                    prompted = false;
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
        let _ = self.monitor();
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
            if end < source.len() && continued_line(chunk) {
                continue;
            }
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
                    let line = match &e {
                        crate::parser::ParseError::Syntax { line, .. } => base + *line,
                        _ => base + chunk.iter().filter(|b| **b == b'\n').count() + 1,
                    };
                    self.env
                        .set(b"LINENO".to_vec(), line.to_string().into_bytes());
                    // Parser tokens refer to the current input chunk; diagnostics
                    // must use its physical offset in the complete source file.
                    let e = match e {
                        crate::parser::ParseError::Syntax { col, token, .. } => {
                            crate::parser::ParseError::Syntax { line, col, token }
                        }
                        other => other,
                    };
                    let message = if let Some(path) = &self.startup_context {
                        format!("rsh: {}:{line}: warning: {e}\n", path.display())
                    } else {
                        format!("{e}\n")
                    };
                    let _ = sys::write_fd(2, message.as_bytes());
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
        if let Some(id) = self
            .jobs
            .jobs
            .values()
            .find(|j| j.processes.iter().any(|p| p.pid == pid))
            .map(|j| j.id)
        {
            loop {
                self.jobs.reap();
                if let Some(j) = self.jobs.jobs.get(&id) {
                    if let Some(p) = j.processes.iter().find(|p| p.pid == pid)
                        && (p.done() || matches!(p.state, sys::ProcessEvent::Stopped(_)))
                    {
                        let status = p.status();
                        if j.state() == crate::jobs::State::Done
                            && let Some(j) = self.jobs.remove(id)
                        {
                            for p in j.processes {
                                if p.pid != pid {
                                    self.completed.insert(p.pid, p.status());
                                }
                            }
                        }
                        return status;
                    }
                } else {
                    return self.completed.remove(&pid).unwrap_or(127);
                }
                if let Some(n) = self.wait_interrupt() {
                    return (128 + n) as u8;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
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
            if let Some(n) = self.wait_interrupt() {
                return (128 + n) as u8;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    fn wait_interrupt(&self) -> Option<i32> {
        let pending = sys::pending_signal_mask();
        (1..64).find(|n| {
            pending & (1u64 << n) != 0
                && (*n != sys::CHLD || self.env.traps.contains_key(n))
                && !sys::inherited_ignored(*n)
        })
    }
    fn wait_job(&mut self, id: usize) -> u8 {
        loop {
            self.jobs.reap();
            if let Some(j) = self.jobs.jobs.get(&id) {
                if j.state() != crate::jobs::State::Running {
                    let status = j.status();
                    if j.state() == crate::jobs::State::Done {
                        self.jobs.remove(id);
                    }
                    return status;
                }
            } else {
                return self.completed_jobs.remove(&id).unwrap_or(127);
            }
            if let Some(n) = self.wait_interrupt() {
                return (128 + n) as u8;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    fn wait_builtin(&mut self, args: &[Vec<u8>]) -> Outcome {
        if args.is_empty() {
            let ids: Vec<_> = self
                .jobs
                .jobs
                .values()
                .filter(|j| !j.disowned)
                .map(|j| j.id)
                .collect();
            for id in ids {
                let status = self.wait_job(id);
                if self.wait_interrupt().is_some() {
                    return Outcome {
                        status,
                        ..Outcome::default()
                    };
                }
            }
            self.completed_jobs.clear();
            while let Some(pid) = self.background.first().map(Child::pid) {
                let status = self.wait_pid(pid);
                if self.wait_interrupt().is_some() {
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
            if arg.starts_with(b"%") {
                let id = self.jobs.resolve(arg).or_else(|| {
                    std::str::from_utf8(arg.get(1..).unwrap_or_default())
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .filter(|id| self.completed_jobs.contains_key(id))
                });
                let Some(id) = id else {
                    return Self::no_job(arg);
                };
                status = self.wait_job(id);
                continue;
            }
            let Some(pid) = std::str::from_utf8(arg)
                .ok()
                .and_then(|s| s.parse::<i32>().ok())
                .filter(|n| *n > 0)
            else {
                return Outcome::error(b"wait", b"invalid pid", 127);
            };
            status = self.wait_pid(pid);
            if self.wait_interrupt().is_some() {
                break;
            }
        }
        Outcome {
            status,
            ..Outcome::default()
        }
    }
    pub(super) fn signals_between_commands(&mut self) -> Option<Flow> {
        if self.motd_startup && sys::motd_interrupted() {
            return Some(Flow::Exit((128 + sys::INT) as u8));
        }
        let signals = sys::pending_signals();
        for n in 1..64 {
            if !self.env.options.interactive && sys::inherited_ignored(n) {
                continue;
            }
            if n == sys::INT
                && signals & (1u64 << n) != 0
                && !self.env.traps.contains_key(&n)
                && !self.env.options.interactive
            {
                self.env.exit_requested = Some((128 + n) as u8);
                return Some(Flow::Exit((128 + n) as u8));
            }
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
        self.cleanup_jobs();
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

// A physical newline removed by the lexer does not end an input unit. Checking
// its token span distinguishes a continuation from a comment or escaped slash.
fn continued_line(source: &[u8]) -> bool {
    source.ends_with(b"\\\n")
        && crate::lexer::lex(source).is_ok_and(|tokens| {
            tokens.last().is_some_and(|token| {
                token.span.end.offset == source.len()
                    && matches!(token.kind, crate::lexer::TokenKind::Word(_))
            })
        })
}
