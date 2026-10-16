#[test]
fn spec_runner_infrastructure_tests() {
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_rsh"));
    let output = std::process::Command::new("python3")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "tests/spec/runner_tests",
            "-v",
        ])
        .env("RSH_SPEC_BIN_DIR", executable.parent().unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
