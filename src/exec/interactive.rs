//! Narrow interpreter interface used by the editor. Editing never evaluates input.
use super::*;
impl Shell {
    /// Restore a private history copy in a re-exec worker. Clearing child history
    /// must not modify the parent session or its on-disk history file.
    pub fn restore_history_snapshot(&mut self, bytes: &[u8]) {
        let entries = crate::interactive::history::decode(bytes);
        self.history = Some(std::sync::Arc::new(std::sync::Mutex::new(
            crate::interactive::history::Store {
                size: entries.len(),
                entries,
                path: None,
                file_size: 0,
            },
        )));
    }
    pub(crate) fn interactive_motd(&mut self) {
        // Parse a fixed command without aliases, then use the ordinary evaluator:
        // functions override the regular builtin; no special dispatch shortcut.
        let Ok(interrupt) = sys::MotdInterrupt::install() else {
            let _ = sys::write_fd(2, b"rsh: motd: cannot install interrupt handler\n");
            return;
        };
        let original_trap = self.env.traps.get(&sys::INT).cloned();
        self.motd_startup = true;
        if let Ok(script) = crate::parser::parse(b"motd") {
            let _ = self.eval_script(&script);
        }
        self.motd_startup = false;
        drop(interrupt);
        if original_trap != self.env.traps.get(&sys::INT).cloned() {
            let _ = sys::trap_signal(sys::INT, self.env.traps.get(&sys::INT).map(Vec::as_slice));
        }
    }
    pub(crate) fn interactive_start(&mut self) {
        let _ = self.monitor();
    }
    pub(crate) fn interactive_eval(
        &mut self,
        source: &[u8],
        base: usize,
    ) -> Result<Flow, crate::parser::ParseError> {
        if let Some((cols, rows)) = sys::terminal_size() {
            self.env
                .set(b"COLUMNS".to_vec(), cols.to_string().into_bytes());
            self.env
                .set(b"LINES".to_vec(), rows.to_string().into_bytes());
        }
        let script = crate::parser::parse_at(
            source,
            if self.env.options.expand_aliases {
                Some(&self.env.aliases)
            } else {
                None
            },
            base,
        )?;
        let flow = self.eval_script(&script);
        let flow = self.recover_line_abort(flow);
        self.env.last_status = flow.status();
        Ok(flow)
    }
    pub(crate) fn interactive_notifications(&mut self) -> Vec<u8> {
        if self.env.options.monitor {
            self.job_notifications()
        } else {
            self.reap_background();
            Vec::new()
        }
    }
    pub(crate) fn interactive_signals(&mut self) -> Option<Flow> {
        self.signals_between_commands()
    }
    pub(crate) fn interactive_end(&mut self, status: u8) -> u8 {
        self.finish(Flow::Exit(status))
    }
    pub(crate) fn interactive_eof(&mut self) -> bool {
        self.stopped_exit()
    }
    pub(crate) fn interactive_prompt(&mut self, name: &[u8]) -> Result<Vec<u8>, ExecError> {
        let saved = self.env.last_status;
        let exiting = self.env.exit_requested;
        let raw = self.env.get(name).unwrap_or_default().to_vec();
        let result = crate::lexer::here_parts(&raw, 0)
            .map_err(|e| crate::expand::ExpandError::new(e.to_string()).into())
            .and_then(|parts| self.expand_scalar(&Word(parts), crate::expand::Mode::HereDoc));
        self.env.last_status = saved;
        self.env.exit_requested = exiting;
        result
    }
    pub(crate) fn interactive_jobs(&self) -> Vec<String> {
        self.jobs
            .jobs
            .values()
            .filter(|j| !j.disowned)
            .flat_map(|j| {
                [
                    format!("%{}", j.id),
                    format!("%{}", String::from_utf8_lossy(&j.command)),
                ]
            })
            .collect()
    }
    pub(crate) fn startup_file(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(source) => {
                self.startup_context = Some(path.to_path_buf());
                self.env.source_depth += 1;
                let flow = self.source_flow(&source);
                self.env.source_depth -= 1;
                let context = self
                    .startup_context
                    .take()
                    .unwrap_or_else(|| path.to_path_buf());
                if matches!(flow, Flow::Exit(_)) {
                    let line = String::from_utf8_lossy(self.env.get(b"LINENO").unwrap_or(b"1"));
                    let _ = sys::write_fd(
                        2,
                        format!(
                            "rsh: {}:{line}: warning: startup file failed (status {})\n",
                            context.display(),
                            flow.status()
                        )
                        .as_bytes(),
                    );
                }
                self.env.last_status = flow.status();
                self.env.exit_requested = None;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                let _ = sys::write_fd(
                    2,
                    format!("rsh: {}:1: warning: {e}\n", path.display()).as_bytes(),
                );
            }
        }
    }
}
