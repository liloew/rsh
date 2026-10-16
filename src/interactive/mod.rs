//! Reedline UI; the interpreter remains the only owner of execution semantics.
pub mod completion;
pub mod highlight;
pub mod history;
pub mod prompt;
pub mod sanitize;
use crate::{
    exec::{Flow, Shell},
    sys,
};
use reedline::{
    EditCommand as Edit, History, KeyCode as Key, KeyModifiers as Mod, MenuBuilder, Reedline,
    ReedlineEvent as Event,
};
use std::{
    cell::RefCell,
    os::unix::ffi::OsStrExt,
    sync::{Arc, Mutex},
};
thread_local! {static SHELL:RefCell<Option<Shell>>=const {RefCell::new(None)};}
fn shell<R>(f: impl FnOnce(&mut Shell) -> R) -> Option<R> {
    SHELL.with(|cell| cell.try_borrow_mut().ok()?.as_mut().map(f))
}
pub fn builtin_names() -> &'static [&'static str] {
    &[
        ":", ".", "alias", "bg", "break", "cd", "command", "continue", "disown", "echo", "eval",
        "exec", "exit", "export", "false", "fg", "getopts", "hash", "history", "jobs", "kill",
        "motd", "printf", "pwd", "read", "readonly", "return", "set", "shift", "test", "[",
        "times", "trap", "true", "type", "umask", "unalias", "unset", "wait",
    ]
}
#[derive(Debug, PartialEq, Eq)]
pub enum InputState {
    Complete,
    Incomplete,
    Syntax,
}
pub fn classify(source: &str) -> InputState {
    match crate::parser::parse(source.as_bytes()) {
        Ok(_) => InputState::Complete,
        Err(crate::parser::ParseError::Incomplete(_)) => InputState::Incomplete,
        Err(_) => InputState::Syntax,
    }
}
struct Validate {
    secondary: Arc<Mutex<String>>,
}
impl reedline::Validator for Validate {
    fn validate(&self, line: &str) -> reedline::ValidationResult {
        if shell(|s| {
            matches!(
                crate::parser::parse_with_aliases(line.as_bytes(), &s.env.aliases),
                Err(crate::parser::ParseError::Incomplete(_))
            )
        })
        .unwrap_or_else(|| classify(line) == InputState::Incomplete)
        {
            if let Some(text) = shell(|s| {
                let Ok(_execution) = completion::execution_guard() else {
                    return b"> ".to_vec();
                };
                s.interactive_prompt(b"PS2")
                    .unwrap_or_else(|_| b"> ".to_vec())
            }) && let Ok(mut secondary) = self.secondary.lock()
            {
                *secondary = String::from_utf8_lossy(&text).into_owned();
            }
            reedline::ValidationResult::Incomplete
        } else {
            reedline::ValidationResult::Complete
        }
    }
}
struct Hints {
    history: Arc<Mutex<history::Store>>,
    cwd: Arc<Mutex<completion::Context>>,
    hint: String,
}
impl reedline::Hinter for Hints {
    fn handle(
        &mut self,
        line: &str,
        pos: usize,
        _: &dyn History,
        ansi: bool,
        _cwd: &str,
    ) -> String {
        self.hint = if pos == line.len() {
            match (self.history.lock(), self.cwd.lock()) {
                (Ok(h), Ok(c)) => history::suggest(&h.entries, line, c.cwd.as_os_str().as_bytes()),
                _ => String::new(),
            }
        } else {
            String::new()
        };
        if ansi && !self.hint.is_empty() {
            format!("\x1b[90m{}\x1b[0m", self.hint)
        } else {
            self.hint.clone()
        }
    }
    fn complete_hint(&self) -> String {
        self.hint.clone()
    }
    fn next_hint_token(&self) -> String {
        let mut word = false;
        self.hint
            .chars()
            .take_while(|c| {
                if c.is_whitespace() {
                    !word
                } else {
                    word = true;
                    true
                }
            })
            .collect()
    }
}
fn bindings(mut keys: reedline::Keybindings) -> reedline::Keybindings {
    keys.add_binding(
        Mod::NONE,
        Key::Tab,
        Event::UntilFound(vec![Event::Menu("completion".to_string()), Event::MenuNext]),
    );
    keys.add_binding(
        Mod::NONE,
        Key::Right,
        Event::UntilFound(vec![
            Event::HistoryHintComplete,
            Event::Edit(vec![Edit::MoveRight { select: false }]),
        ]),
    );
    keys.add_binding(
        Mod::NONE,
        Key::End,
        Event::UntilFound(vec![
            Event::HistoryHintComplete,
            Event::Edit(vec![Edit::MoveToLineEnd { select: false }]),
        ]),
    );
    keys.add_binding(
        Mod::ALT,
        Key::Right,
        Event::UntilFound(vec![
            Event::HistoryHintWordComplete,
            Event::Edit(vec![Edit::MoveWordRight { select: false }]),
        ]),
    );
    keys.add_binding(
        Mod::CONTROL,
        Key::Right,
        Event::Edit(vec![Edit::MoveWordRight { select: false }]),
    );
    keys.add_binding(
        Mod::CONTROL,
        Key::Left,
        Event::Edit(vec![Edit::MoveWordLeft { select: false }]),
    );
    keys
}
fn edit_mode(vi: bool) -> Box<dyn reedline::EditMode> {
    if vi {
        Box::new(reedline::Vi::new(
            bindings(reedline::default_vi_insert_keybindings()),
            reedline::default_vi_normal_keybindings(),
        ))
    } else {
        Box::new(reedline::Emacs::new(bindings(
            reedline::default_emacs_keybindings(),
        )))
    }
}
pub fn startup(s: &mut Shell) {
    let home = s
        .env
        .get(b"HOME")
        .map(|p| std::path::PathBuf::from(std::ffi::OsStr::from_bytes(p)));
    if s.env.options.login {
        s.startup_file(std::path::Path::new("/etc/profile"));
        if let Some(home) = &home {
            s.startup_file(&home.join(".profile"));
        }
    }
    if s.env.options.interactive {
        let config = s
            .env
            .get(b"XDG_CONFIG_HOME")
            .filter(|s| !s.is_empty())
            .map(|p| std::path::PathBuf::from(std::ffi::OsStr::from_bytes(p)))
            .or_else(|| home.map(|p| p.join(".config")));
        if let Some(config) = config {
            s.startup_file(&config.join("rsh/config.sh"));
        }
    }
}
/// Called only on the top-level terminal REPL path, after configuration.
pub fn startup_motd(s: &mut Shell, disabled: bool) {
    if disabled
        || !s.env.options.interactive
        || !sys::terminal(0)
        || s.env.get(b"RSH_NO_MOTD").is_some_and(|v| !v.is_empty())
        || s.env.get(b"HOME").is_some_and(|home| {
            sys::motd_hushlogin(
                &s.env
                    .cwd
                    .join(std::ffi::OsStr::from_bytes(home))
                    .join(".hushlogin"),
            )
        })
    {
        return;
    }
    s.interactive_start();
    let status = s.env.last_status;
    let exiting = s.env.exit_requested;
    s.interactive_motd();
    s.env.last_status = status;
    s.env.exit_requested = exiting;
}
fn prompt_text(s: &mut Shell) -> String {
    if s.env.get(b"PS1").is_some() {
        match s.interactive_prompt(b"PS1") {
            Ok(p) => String::from_utf8_lossy(&p).into_owned(),
            Err(e) => {
                let _ = sys::write_fd(2, format!("rsh: prompt: {e}\n").as_bytes());
                "$ ".to_string()
            }
        }
    } else {
        prompt::default_prompt(
            &s.env.cwd,
            s.env.get(b"HOME"),
            sys::effective_uid() == 0,
            s.env.options.git_prompt,
        )
    }
}
fn memory_history(store: &history::Store) -> Box<dyn History> {
    let mut h = reedline::FileBackedHistory::new(store.size.max(1)).unwrap_or_default();
    for e in &store.entries {
        let _ = h.save(reedline::HistoryItem::from_command_line(&e.command));
    }
    Box::new(h)
}
pub fn run(mut s: Shell) -> u8 {
    s.interactive_start();
    let mut ansi = s
        .env
        .get(b"TERM")
        .is_some_and(|t| !t.is_empty() && t != b"dumb");
    let mut reservation: Option<sys::EditorDescriptors> = None;
    let mut editor_started = false;
    let _recovery = sys::terminal_recovery(ansi).ok();
    let store = match history::Store::open(&s.env) {
        Ok(h) => h,
        Err(e) => {
            let _ = sys::write_fd(2, format!("rsh: history: {e}\n").as_bytes());
            history::Store::default()
        }
    };
    let store = Arc::new(Mutex::new(store));
    s.history = Some(store.clone());
    let context = Arc::new(Mutex::new(completion::Context::from_env(&s.env)));
    let cache = completion::PathCache::default();
    let secondary = Arc::new(Mutex::new("> ".to_string()));
    SHELL.with(|cell| *cell.borrow_mut() = Some(s));
    let printer = reedline::ExternalPrinter::<String>::new(64);
    let tx = printer.sender();
    let idle = move || {
        if let Some(notices) = shell(|s| {
            if let Some((cols, rows)) = sys::terminal_size() {
                s.env
                    .set(b"COLUMNS".to_vec(), cols.to_string().into_bytes());
                s.env.set(b"LINES".to_vec(), rows.to_string().into_bytes());
            }
            s.interactive_notifications()
        }) && !notices.is_empty()
        {
            let _ = tx.try_send(String::from_utf8_lossy(&notices).trim_end().to_string());
        }
    };
    let mut editor = Reedline::create()
        .with_ansi_colors(ansi)
        .with_highlighter(Box::new(highlight::Highlight {
            context: context.clone(),
            cache: cache.clone(),
        }))
        .with_completer(Box::new(completion::Completion {
            context: context.clone(),
            registry: completion::Registry::standard(),
            cache: cache.clone(),
        }))
        .with_hinter(Box::new(Hints {
            history: store.clone(),
            cwd: context.clone(),
            hint: String::new(),
        }))
        .with_validator(Box::new(Validate {
            secondary: secondary.clone(),
        }))
        .with_history_exclusion_prefix(Some(" ".to_string()))
        .with_quick_completions(true)
        .with_partial_completions(true)
        .use_bracketed_paste(true)
        .with_external_printer(printer)
        .with_idle_callback(Box::new(idle))
        .with_menu(reedline::ReedlineMenu::EngineCompleter(Box::new(
            reedline::ColumnarMenu::default().with_name("completion"),
        )));
    let mut base = 0usize;
    let mut plain = Vec::new();
    let (status, _exit_execution) = loop {
        let initial_guard = completion::execution_guard().ok();
        if editor_started
            && initial_guard.is_some()
            && let Some(reservation) = reservation.take()
        {
            reservation.finish();
        }
        let Some((left, failed, vi, cwd, notices, signal_flow)) = shell(|s| {
            let signal_flow = if initial_guard.is_some() {
                s.interactive_signals()
            } else {
                None
            };
            if let Some((cols, rows)) = sys::terminal_size() {
                s.env
                    .set(b"COLUMNS".to_vec(), cols.to_string().into_bytes());
                s.env.set(b"LINES".to_vec(), rows.to_string().into_bytes());
            }
            s.env
                .set(b"LINENO".to_vec(), (base + 1).to_string().into_bytes());
            let notices = s.interactive_notifications();
            let left = if initial_guard.is_some() {
                prompt_text(s)
            } else {
                "$ ".to_string()
            };
            if let Ok(mut ctx) = context.lock() {
                *ctx = completion::Context::from_env(&s.env);
                ctx.jobs = s.interactive_jobs();
                if ansi {
                    cache.refresh(s.env.get(b"PATH").unwrap_or(b"/usr/bin:/bin"), &s.env.cwd);
                }
            }
            if let Ok(mut h) = store.lock() {
                h.configure(&s.env);
            }
            (
                left,
                s.env.last_status != 0,
                s.env.options.vi,
                s.env.cwd.clone(),
                notices,
                signal_flow,
            )
        }) else {
            break (1, initial_guard);
        };
        if let Some(Flow::Exit(status)) = signal_flow {
            break (status, initial_guard);
        }
        if !notices.is_empty() {
            let _ = sys::write_fd(2, &notices);
        }
        if let Ok(h) = store.lock() {
            editor = editor.with_history(memory_history(&h));
        }
        editor = editor
            .with_edit_mode(edit_mode(vi))
            .with_cwd(Some(cwd.display().to_string()));
        let prompt = prompt::ShellPrompt {
            left,
            secondary: secondary.clone(),
            failed,
        };
        drop(initial_guard);
        if ansi && !editor_started {
            reservation = match sys::reserve_editor_descriptors() {
                Ok(r) => Some(r),
                Err(e) => {
                    let _ = sys::write_fd(
                        2,
                        format!("rsh: editor: {e}; using plain input\n").as_bytes(),
                    );
                    ansi = false;
                    None
                }
            };
        }
        let input = if ansi {
            editor.read_line(&prompt).map_err(|e| e.to_string())
        } else {
            read_plain(&prompt, &mut plain)
        };
        editor_started = true;
        // Completion remains asynchronous during editing. Before execution,
        // wait for outstanding descriptor-owning scans to leave their critical
        // sections, then prevent new scans until command evaluation completes.
        let execution = if !matches!(&input, Ok(reedline::Signal::CtrlC)) {
            match completion::execution_guard() {
                Ok(guard) => {
                    if let Some(reservation) = reservation.take() {
                        reservation.finish();
                    }
                    Some(guard)
                }
                Err(e) => {
                    let _ = sys::write_fd(2, format!("rsh: completion: {e}\n").as_bytes());
                    let _ = shell(|s| s.env.last_status = 1);
                    if let Ok(reedline::Signal::Success(line)) = input {
                        editor.run_edit_commands(&[Edit::InsertString(line)]);
                    }
                    continue;
                }
            }
        } else {
            None
        };
        match input {
            Ok(reedline::Signal::Success(line)) => {
                let _ = shell(|s| {
                    if let Ok(mut h) = store.lock()
                        && let Err(e) = h.record(&line, s.env.cwd.as_os_str().as_bytes())
                    {
                        let _ = sys::write_fd(2, format!("rsh: history: {e}\n").as_bytes());
                    }
                });
                let result = shell(|s| s.interactive_eval(line.as_bytes(), base));
                base += line.bytes().filter(|b| *b == b'\n').count() + 1;
                match result {
                    Some(Ok(Flow::Exit(status))) => break (status, execution),
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        let _ = sys::write_fd(2, format!("{e}\n").as_bytes());
                        let _ = shell(|s| {
                            s.env.last_status = 2;
                            s.env.exit_requested = None;
                        });
                    }
                    None => break (1, execution),
                }
            }
            Ok(reedline::Signal::CtrlC) => {
                let _ = sys::take_interrupt();
                let _ = shell(|s| {
                    s.env.last_status = 130;
                    s.env.exit_requested = None;
                });
                plain.clear();
            }
            Ok(reedline::Signal::CtrlD) => {
                if shell(Shell::interactive_eof).unwrap_or(false) {
                    continue;
                }
                break (shell(|s| s.env.last_status).unwrap_or(0), execution);
            }
            Err(e) => {
                let _ = sys::write_fd(2, format!("rsh: editor: {e}\n").as_bytes());
                break (1, execution);
            }
        }
    };
    let status = shell(|s| s.interactive_end(status)).unwrap_or(status);
    SHELL.with(|cell| *cell.borrow_mut() = None);
    status
}
fn read_plain(
    prompt: &prompt::ShellPrompt,
    buffer: &mut Vec<u8>,
) -> Result<reedline::Signal, String> {
    use reedline::Prompt;
    let _ = sys::trap_signal(sys::INT, Some(b"pending"));
    let _ = sys::write_fd(
        2,
        if buffer.is_empty() {
            prompt.left.as_bytes()
        } else {
            b"> "
        },
    );
    loop {
        match sys::read_byte(0) {
            Ok(Some(b)) => {
                buffer.push(b);
                if b == b'\n' {
                    if matches!(
                        crate::parser::parse(buffer),
                        Err(crate::parser::ParseError::Incomplete(_))
                    ) {
                        let secondary = shell(|s| {
                            s.interactive_prompt(b"PS2")
                                .unwrap_or_else(|_| b"> ".to_vec())
                        })
                        .unwrap_or_else(|| {
                            prompt
                                .render_prompt_multiline_indicator()
                                .as_bytes()
                                .to_vec()
                        });
                        let _ = sys::write_fd(2, &secondary);
                        continue;
                    }
                    let mut line = std::mem::take(buffer);
                    if line.last() == Some(&b'\n') {
                        line.pop();
                    }
                    return Ok(reedline::Signal::Success(
                        String::from_utf8_lossy(&line).into_owned(),
                    ));
                }
            }
            Ok(None) => return Ok(reedline::Signal::CtrlD),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                if sys::take_interrupt() {
                    let _ = sys::write_fd(2, b"\n");
                    return Ok(reedline::Signal::CtrlC);
                }
                if let Some(notices) = shell(Shell::interactive_notifications) {
                    let _ = sys::write_fd(2, &notices);
                    let _ = sys::write_fd(2, prompt.left.as_bytes());
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests;
