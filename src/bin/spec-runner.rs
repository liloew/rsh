//! Test infrastructure launcher; this does not participate in shell execution.
use std::process::{Command, ExitCode};
fn run() -> std::io::Result<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let exe = std::env::current_exe()?;
    let directory = exe
        .parent()
        .ok_or_else(|| std::io::Error::other("missing executable directory"))?;
    let mut build = Command::new(option_env!("CARGO").unwrap_or("cargo"));
    build
        .current_dir(root)
        .args(["build", "--quiet", "--bin", "rsh", "--bin", "spec-sandbox"]);
    if directory.file_name().is_some_and(|s| s == "release") {
        build.arg("--release");
    }
    if !build.status()?.success() {
        return Ok(2);
    }
    let status = Command::new("python3")
        .arg(root.join("tests/spec_runner.py"))
        .args(std::env::args_os().skip(1))
        .env("RSH_SPEC_BIN_DIR", directory)
        .current_dir(root)
        .status()?;
    Ok(status
        .code()
        .and_then(|n| u8::try_from(n).ok())
        .unwrap_or(1))
}
fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("spec-runner: {e}");
            ExitCode::from(2)
        }
    }
}
