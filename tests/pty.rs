#[path = "pty/job_control.rs"]
mod job_control;
mod support;

#[path = "pty/interactive.rs"]
mod interactive;
#[path = "pty/regression.rs"]
mod regression;

// Serialize controlling-terminal scenarios, including the large script corpus.
// Two-shell concurrency remains exercised inside its dedicated scenario.
static PTY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[path = "pty/motd.rs"]
mod motd;
