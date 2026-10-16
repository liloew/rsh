//! Job-control integration. The table and all wait consumers run serially.
use super::*;
use crate::jobs::State;
impl Shell {
    pub(super) fn monitor(&mut self) -> bool {
        if !self.env.options.interactive || !self.env.options.monitor {
            return false;
        }
        if self.terminal.is_none() && !self.terminal_failed {
            match sys::Terminal::initialize() {
                Ok(t) => {
                    self.terminal = Some(t);
                    let _ = sys::watch_children();
                    for (n, action) in &self.env.traps {
                        if *n != 0 {
                            let _ = sys::trap_signal(*n, Some(action));
                        }
                    }
                }
                Err(e) => {
                    self.terminal_failed = true;
                    let _ = sys::write_fd(2, format!("rsh: job control: {e}\n").as_bytes());
                }
            }
        }
        self.terminal.is_some()
    }
    pub(super) fn job_foreground(&mut self, id: usize, resume: bool) -> Result<u8, ExecError> {
        let Some(j) = self.jobs.jobs.get_mut(&id) else {
            return Ok(127);
        };
        j.background = false;
        let group = j.pgid;
        let setup = (|| -> std::io::Result<()> {
            if let Some(t) = &self.terminal {
                t.give(group)?;
                if resume && let Some(m) = &j.modes {
                    sys::restore_modes(t.fd(), m)?;
                }
            }
            if resume {
                sys::send_signal(-group, sys::CONT)?;
                j.resume();
            }
            Ok(())
        })();
        if let Err(e) = setup {
            if let Some(t) = &self.terminal {
                let _ = t.reclaim();
            }
            return Err(e.into());
        }
        let result = (|| -> Result<(), std::io::Error> {
            while self
                .jobs
                .jobs
                .get(&id)
                .is_some_and(|j| j.state() == State::Running)
            {
                match sys::wait_event(-group, false) {
                    Ok(Some(e)) => self.jobs.update(e),
                    Ok(None) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        })();
        if let Some(t) = &self.terminal {
            if let Some(j) = self.jobs.jobs.get_mut(&id) {
                j.modes = sys::terminal_modes(t.fd()).ok();
            }
            t.reclaim()?;
        }
        result?;
        let status = self.jobs.jobs.get(&id).map(|j| j.status()).unwrap_or(127);
        let stopped = self
            .jobs
            .jobs
            .get(&id)
            .is_some_and(|j| j.state() == State::Stopped);
        if stopped {
            self.exit_warned = false;
            self.jobs.touch(id);
            let _ = sys::write_fd(2, &self.jobs.line(id, false));
            if let Some(j) = self.jobs.jobs.get_mut(&id) {
                j.background = true;
                j.notified = true;
            }
        } else if let Some(j) = self.jobs.remove(id)
            && let Some(p) = j.processes.last()
            && let sys::ProcessEvent::Signaled(n) = p.state
            && n != sys::INT
            && n != sys::PIPE
        {
            let _ = sys::write_fd(2, format!("{}\n", sys::signal_message(n)).as_bytes());
        }
        Ok(status)
    }
    pub(super) fn notify_jobs(&mut self) -> bool {
        self.jobs.reap();
        let ids: Vec<_> = self
            .jobs
            .jobs
            .values()
            .filter(|j| j.background && !j.disowned && !j.notified && j.state() != State::Running)
            .map(|j| j.id)
            .collect();
        let announced = !ids.is_empty();
        for id in ids {
            let _ = sys::write_fd(2, &self.jobs.line(id, false));
            if let Some(j) = self.jobs.jobs.get_mut(&id) {
                j.notified = true;
            }
        }
        // Keep completion statuses for wait, but no longer list notified completed jobs.
        let done: Vec<_> = self
            .jobs
            .jobs
            .values()
            .filter(|j| j.state() == State::Done && (j.notified || j.disowned))
            .map(|j| j.id)
            .collect();
        for id in done {
            if let Some(j) = self.jobs.remove(id) {
                for p in &j.processes {
                    self.completed.insert(p.pid, p.status());
                }
                self.completed_jobs.insert(id, j.status());
            }
        }
        announced
    }
    pub(super) fn no_job(spec: &[u8]) -> Outcome {
        let mut out = Outcome {
            status: 1,
            ..Outcome::default()
        };
        out.stderr.extend(b"rsh: ");
        out.stderr.extend(spec);
        out.stderr.extend(b": no such job\n");
        out
    }
    pub(super) fn jobs_builtin(&mut self, builtin: Builtin, args: &[Vec<u8>]) -> Outcome {
        self.jobs.reap();
        if matches!(builtin, Builtin::Jobs) {
            let mut long = false;
            let mut pid = false;
            let mut specs = Vec::new();
            for a in args {
                match a.as_slice() {
                    b"-l" => long = true,
                    b"-p" => pid = true,
                    b"--" => {}
                    a if a.starts_with(b"-") => {
                        return Outcome::error(b"jobs", b"invalid option", 2);
                    }
                    _ => specs.push(a),
                }
            }
            let ids = if specs.is_empty() {
                self.jobs
                    .jobs
                    .values()
                    .filter(|j| !j.disowned)
                    .map(|j| j.id)
                    .collect::<Vec<_>>()
            } else {
                let mut ids = Vec::new();
                for spec in specs {
                    match self.jobs.resolve(spec) {
                        Some(id) => ids.push(id),
                        None => return Self::no_job(spec),
                    }
                }
                ids
            };
            let mut out = Outcome::default();
            for id in ids {
                if pid {
                    if let Some(j) = self.jobs.jobs.get(&id) {
                        out.stdout.extend(format!("{}\n", j.pgid).as_bytes());
                    }
                } else {
                    out.stdout.extend(self.jobs.line(id, long));
                }
            }
            return out;
        }
        if matches!(builtin, Builtin::Disown) {
            let all = args.iter().any(|a| a == b"-a");
            let no_hup = args.iter().any(|a| a == b"-h");
            if args
                .iter()
                .any(|a| a.starts_with(b"-") && a != b"-a" && a != b"-h")
            {
                return Outcome::error(b"disown", b"invalid option", 2);
            }
            let specs: Vec<_> = args.iter().filter(|a| !a.starts_with(b"-")).collect();
            let ids = if all {
                self.jobs.jobs.keys().copied().collect::<Vec<_>>()
            } else {
                let defaults = [b"%+".to_vec()];
                let specs = if specs.is_empty() {
                    defaults.iter().collect::<Vec<_>>()
                } else {
                    specs
                };
                let mut ids = Vec::new();
                for spec in specs {
                    match self.jobs.resolve(spec) {
                        Some(id) => ids.push(id),
                        None => return Self::no_job(spec),
                    }
                }
                ids
            };
            for id in ids {
                if let Some(j) = self.jobs.jobs.get_mut(&id) {
                    j.no_hup = true;
                    if !no_hup {
                        j.disowned = true;
                    }
                }
            }
            return Outcome::default();
        }
        if !self.monitor() {
            return Outcome::error(b"job control", b"not enabled", 1);
        }
        if args.len() > 1 {
            return Outcome::error(b"job control", b"too many arguments", 2);
        }
        let spec = args.first().map(Vec::as_slice).unwrap_or(b"%+");
        let Some(id) = self.jobs.resolve(spec) else {
            return Self::no_job(spec);
        };
        if matches!(builtin, Builtin::Bg) {
            let Some(j) = self.jobs.jobs.get_mut(&id) else {
                return Self::no_job(spec);
            };
            if let Err(e) = sys::send_signal(-j.pgid, sys::CONT) {
                return Outcome::error(b"bg", e.to_string().as_bytes(), 1);
            }
            j.resume();
            j.background = true;
            self.jobs.touch(id);
            Outcome {
                stdout: self.jobs.line(id, false),
                ..Outcome::default()
            }
        } else {
            if let Some(j) = self.jobs.jobs.get(&id) {
                let _ = sys::write_fd(2, &[j.command.as_slice(), b"\n"].concat());
            }
            match self.job_foreground(id, true) {
                Ok(status) => Outcome {
                    status,
                    ..Outcome::default()
                },
                Err(e) => Outcome::error(b"fg", e.to_string().as_bytes(), 1),
            }
        }
    }
    pub(super) fn kill_builtin(&mut self, args: &[Vec<u8>]) -> Outcome {
        let mut translated = args.to_vec();
        for a in &mut translated {
            if a.starts_with(b"%") {
                let Some(id) = self.jobs.resolve(a) else {
                    return Self::no_job(a);
                };
                if let Some(j) = self.jobs.jobs.get(&id) {
                    *a = format!("{}", -j.pgid).into_bytes();
                }
            }
        }
        if args.first().is_some_and(|a| a.starts_with(b"%")) {
            translated.insert(0, b"--".to_vec());
        }
        builtin_impl::misc::kill(&translated)
    }
    pub(super) fn cleanup_jobs(&mut self) {
        if !self.env.options.interactive {
            return;
        }
        for j in self
            .jobs
            .jobs
            .values()
            .filter(|j| !j.no_hup && j.state() != State::Done)
        {
            let _ = sys::send_signal(-j.pgid, sys::HUP);
            let _ = sys::send_signal(-j.pgid, sys::CONT);
        }
        if let Some(t) = &self.terminal {
            let _ = t.reclaim();
        }
    }
    pub(super) fn stopped_exit(&mut self) -> bool {
        self.jobs.reap();
        if self.env.options.interactive
            && !self.exit_warned
            && self
                .jobs
                .jobs
                .values()
                .any(|j| !j.disowned && j.state() == State::Stopped)
        {
            self.exit_warned = true;
            let _ = sys::write_fd(2, b"There are stopped jobs.\n");
            true
        } else {
            false
        }
    }
}
