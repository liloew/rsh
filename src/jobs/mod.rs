//! Job state is updated only in shell context, never in a signal handler.
use crate::sys::{self, ProcessEvent, TerminalModes, WaitEvent};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Running,
    Stopped,
    Done,
}
#[derive(Debug, Clone)]
pub struct Process {
    pub pid: i32,
    pub state: ProcessEvent,
}
impl Process {
    pub fn done(&self) -> bool {
        matches!(
            self.state,
            ProcessEvent::Exited(_) | ProcessEvent::Signaled(_)
        )
    }
    pub fn status(&self) -> u8 {
        match self.state {
            ProcessEvent::Exited(s) => s,
            ProcessEvent::Signaled(n) | ProcessEvent::Stopped(n) => (128 + n) as u8,
            _ => 0,
        }
    }
}
pub struct Job {
    pub id: usize,
    pub pgid: i32,
    pub processes: Vec<Process>,
    pub command: Vec<u8>,
    pub background: bool,
    pub notified: bool,
    pub no_hup: bool,
    pub disowned: bool,
    pub modes: Option<TerminalModes>,
    pub pipefail: bool,
}
impl Job {
    pub fn state(&self) -> State {
        if self.processes.iter().all(Process::done) {
            State::Done
        } else if self
            .processes
            .iter()
            .all(|p| p.done() || matches!(p.state, ProcessEvent::Stopped(_)))
        {
            State::Stopped
        } else {
            State::Running
        }
    }
    pub fn status(&self) -> u8 {
        if self.pipefail {
            self.processes
                .iter()
                .rev()
                .find(|p| p.status() != 0)
                .map(Process::status)
                .unwrap_or(0)
        } else {
            self.processes.last().map(Process::status).unwrap_or(0)
        }
    }
    pub fn label(&self) -> String {
        match self.state() {
            State::Running => "Running".into(),
            State::Stopped => "Stopped".into(),
            State::Done => {
                if let Some(Process {
                    state: ProcessEvent::Signaled(n),
                    ..
                }) = self.processes.last()
                {
                    sys::signal_message(*n).into()
                } else if self.status() == 0 {
                    "Done".into()
                } else {
                    format!("Exit {}", self.status())
                }
            }
        }
    }
    pub fn resume(&mut self) {
        for p in &mut self.processes {
            if matches!(p.state, ProcessEvent::Stopped(_)) {
                p.state = ProcessEvent::Continued;
            }
        }
        self.notified = false;
    }
}
#[derive(Default)]
pub struct Table {
    pub jobs: BTreeMap<usize, Job>,
    recent: Vec<usize>,
}
impl Table {
    pub fn insert(
        &mut self,
        pids: Vec<i32>,
        pgid: i32,
        command: Vec<u8>,
        background: bool,
        pipefail: bool,
    ) -> usize {
        let id = self.jobs.keys().next_back().copied().unwrap_or(0) + 1;
        self.jobs.insert(
            id,
            Job {
                id,
                pgid,
                processes: pids
                    .into_iter()
                    .map(|pid| Process {
                        pid,
                        state: ProcessEvent::Continued,
                    })
                    .collect(),
                command,
                background,
                notified: false,
                no_hup: false,
                disowned: false,
                modes: None,
                pipefail,
            },
        );
        self.touch(id);
        id
    }
    pub fn touch(&mut self, id: usize) {
        self.recent.retain(|i| *i != id);
        self.recent.push(id);
    }
    pub fn remove(&mut self, id: usize) -> Option<Job> {
        self.recent.retain(|i| *i != id);
        self.jobs.remove(&id)
    }
    pub fn marker(&self, id: usize) -> char {
        let active: Vec<_> = self
            .recent
            .iter()
            .filter(|i| self.jobs.get(i).is_some_and(|j| !j.disowned))
            .collect();
        if active.last().is_some_and(|i| **i == id) {
            '+'
        } else if active.iter().rev().nth(1).is_some_and(|i| **i == id) {
            '-'
        } else {
            ' '
        }
    }
    pub fn resolve(&self, spec: &[u8]) -> Option<usize> {
        let s = spec.strip_prefix(b"%")?;
        let active: Vec<_> = self
            .recent
            .iter()
            .rev()
            .filter(|i| self.jobs.get(i).is_some_and(|j| !j.disowned))
            .copied()
            .collect();
        match s {
            b"" | b"%" | b"+" => active.first().copied(),
            b"-" => active.get(1).copied(),
            _ => {
                if let Ok(s) = std::str::from_utf8(s)
                    && let Ok(id) = s.parse::<usize>()
                {
                    return self.jobs.get(&id).filter(|j| !j.disowned).map(|j| j.id);
                }
                let matches: Vec<_> = self
                    .jobs
                    .values()
                    .filter(|j| !j.disowned)
                    .filter(|j| {
                        if let Some(needle) = s.strip_prefix(b"?") {
                            needle.is_empty()
                                || j.command.windows(needle.len()).any(|w| w == needle)
                        } else {
                            j.command.starts_with(s)
                        }
                    })
                    .map(|j| j.id)
                    .collect();
                if matches.len() == 1 {
                    matches.first().copied()
                } else {
                    None
                }
            }
        }
    }
    pub fn update(&mut self, event: WaitEvent) {
        let mut stopped = None;
        for job in self.jobs.values_mut() {
            let before = job.state();
            if let Some(p) = job.processes.iter_mut().find(|p| p.pid == event.pid) {
                p.state = event.event;
                let after = job.state();
                if after != before {
                    job.notified = false;
                    if after == State::Stopped {
                        stopped = Some(job.id);
                    }
                }
                break;
            }
        }
        if let Some(id) = stopped {
            self.touch(id);
        }
    }
    pub fn line(&self, id: usize, long: bool) -> Vec<u8> {
        let Some(j) = self.jobs.get(&id) else {
            return Vec::new();
        };
        let mut out = format!(
            "[{}]{}  {}{}  ",
            id,
            self.marker(id),
            if long {
                format!("{} ", j.pgid)
            } else {
                String::new()
            },
            j.label()
        )
        .into_bytes();
        out.extend(&j.command);
        out.push(b'\n');
        out
    }
    /// Poll each owned PID, including stopped ones; never call waitpid(-1).
    pub fn reap(&mut self) {
        let pids: Vec<_> = self
            .jobs
            .values()
            .flat_map(|j| j.processes.iter().filter(|p| !p.done()).map(|p| p.pid))
            .collect();
        for pid in pids {
            loop {
                match sys::wait_event(pid, true) {
                    Ok(Some(e)) => self.update(e),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    _ => break,
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn states_and_specs() {
        let mut t = Table::default();
        let a = t.insert(
            vec![10, 11],
            10,
            b"sleep 10 | sleep 20".to_vec(),
            true,
            false,
        );
        let b = t.insert(vec![12], 12, b"sleep 30".to_vec(), true, false);
        assert_eq!(t.marker(a), '-');
        assert_eq!(t.marker(b), '+');
        assert_eq!(t.resolve(b"%sleep"), None);
        assert_eq!(t.resolve(b"%?20"), Some(a));
        assert_eq!(t.resolve(b"%-"), Some(a));
        assert_eq!(t.resolve(b"%%"), Some(b));
        t.update(WaitEvent {
            pid: 10,
            event: ProcessEvent::Stopped(20),
        });
        assert_eq!(t.jobs[&a].state(), State::Running);
        t.update(WaitEvent {
            pid: 11,
            event: ProcessEvent::Stopped(20),
        });
        assert_eq!(t.jobs[&a].state(), State::Stopped);
        t.jobs.get_mut(&a).unwrap().notified = true;
        t.update(WaitEvent {
            pid: 11,
            event: ProcessEvent::Stopped(20),
        });
        assert!(
            t.jobs[&a].notified,
            "a repeated process stop must not announce the same job twice"
        );
        t.jobs.get_mut(&a).unwrap().resume();
        assert_eq!(t.jobs[&a].state(), State::Running);
        for pid in [10, 11] {
            t.update(WaitEvent {
                pid,
                event: ProcessEvent::Exited(3),
            });
        }
        assert_eq!(t.jobs[&a].state(), State::Done);
        assert_eq!(t.jobs[&a].status(), 3);
    }
}
