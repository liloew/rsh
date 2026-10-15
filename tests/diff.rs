use std::process::{Command, Output};

fn run(shell: &str, posix: bool, source: &str) -> std::io::Result<Output> {
    let mut command = Command::new(shell);
    if posix {
        command.arg("--posix");
    }
    command
        .args(["-c", source])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .output()
}

#[test]
fn m0_external_commands_match_reference_shells() {
    let references = [("dash", false), ("bash", true)];
    for (shell, posix) in references {
        match run(shell, posix, "") {
            Ok(output) => assert!(output.status.success()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("skipping differential tests: {shell} is not installed");
                return;
            }
            Err(error) => panic!("cannot run {shell}: {error}"),
        }
    }
    for source in include_str!("diff/m0.sh")
        .lines()
        .filter(|line| !line.is_empty())
    {
        let actual = run(env!("CARGO_BIN_EXE_rsh"), false, source).unwrap();
        for (shell, posix) in references {
            let expected = run(shell, posix, source).unwrap();
            assert_eq!(actual.stdout, expected.stdout, "stdout: {shell}: {source}");
            assert_eq!(actual.stderr, expected.stderr, "stderr: {shell}: {source}");
            assert_eq!(
                actual.status.code(),
                expected.status.code(),
                "status: {shell}: {source}"
            );
        }
    }
}
