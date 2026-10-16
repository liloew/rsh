mod support;
use support::{TempDir, capture};

fn run(d: &TempDir, shell: &str, script: &str) -> std::process::Output {
    let mut command = d.command(shell);
    if shell == "bash" || shell.ends_with("/bash") {
        command.arg("--posix");
    }
    command.args(["-c", script]);
    capture(command).0
}
fn compare(script: &str, expected: &[u8]) {
    let d = TempDir::new();
    d.fixtures();
    std::fs::write(d.0.join("visible"), b"data").unwrap();
    std::fs::write(d.0.join(".hidden"), b"data").unwrap();
    let actual = run(&d, env!("CARGO_BIN_EXE_rsh"), script);
    assert!(actual.status.success(), "{script}: {actual:?}");
    assert!(!actual.stderr.windows(3).any(|s| s == b"BAD"));
    assert_eq!(actual.stdout, expected, "{script}: {actual:?}");
    let reference_shell = if std::path::Path::new("/opt/homebrew/bin/bash").is_file() {
        "/opt/homebrew/bin/bash"
    } else {
        "bash"
    };
    if std::process::Command::new(reference_shell)
        .args(["-c", ":"])
        .status()
        .is_ok()
    {
        // Bash before 4.2 does not have the -v variable predicate.
        let supports_v = run(&d, reference_shell, "x=; [[ -v x ]]").status.success();
        if !script.contains("-v ") || supports_v {
            let reference = run(&d, reference_shell, script);
            assert_eq!(actual.stdout, reference.stdout, "{script}: {reference:?}");
            assert_eq!(actual.status.code(), reference.status.code(), "{script}");
        }
    }
}
#[test]
fn conditional_bash_differential() {
    for (script, expected) in [
        ("[[ x ]]; echo $?; [[ '' ]]; echo $?", "0\n1\n"),
        (
            "x='a b *'; [[ $x == 'a b *' ]]; echo $?; [[ -n $x && ! -z $x ]]; echo $?",
            "0\n0\n",
        ),
        (
            "[[ abc == a* ]]; echo $?; [[ abc == 'a*' ]]; echo $?; [[ abc != b* ]]; echo $?",
            "0\n1\n0\n",
        ),
        (
            "p='a*'; [[ abc = $p ]]; echo $?; [[ abc = \"$p\" ]]; echo $?",
            "0\n1\n",
        ),
        (
            "[[ x == x || x == y && x == z ]]; echo $?; [[ ! ( x == x && x == y ) ]]; echo $?",
            "0\n0\n",
        ),
        (
            "[[ x == x || $(echo BAD >&2) ]]; echo $?; [[ '' && $(echo BAD >&2) ]]; echo $?",
            "0\n1\n",
        ),
        (
            "[[ x < y ]]; echo $?; [[ y > x ]]; echo $?; [[ z == [!a-c] ]]; echo $?",
            "0\n0\n0\n",
        ),
        (
            "n=3; [[ n+1 -eq 4 && 0x10 -gt 07 ]]; echo $?; [[ 2 -ne 3 && 2 -le 2 && 3 -ge 3 ]]; echo $?",
            "0\n0\n",
        ),
        (
            "[[ -e visible && -f visible && -s visible && -r visible && -d . ]]; echo $?; [[ -e '' ]]; echo $?",
            "0\n1\n",
        ),
        (
            "x=; [[ -v x && ! -v missing ]]; echo $?; [[ -o nounset ]]; echo $?",
            "0\n1\n",
        ),
        (
            "[[ visible -ef ./visible ]]; echo $?; [[ visible -nt missing ]]; echo $?; [[ missing -ot visible ]]; echo $?",
            "0\n0\n0\n",
        ),
        (
            "[[ abc =~ ^a(b|c)+$ ]]; echo $?; [[ 123 =~ ^[[:digit:]]+$ ]]; echo $?",
            "0\n0\n",
        ),
        (
            "p='^a(b|c)+$'; [[ abc =~ $p ]]; echo $?; [[ abc =~ \"$p\" ]]; echo $?",
            "0\n1\n",
        ),
        (
            "[[ a.b =~ a'.'b ]]; echo $?; [[ axb =~ a'.'b ]]; echo $?; p='['; [[ x =~ $p ]]; echo $?",
            "0\n1\n2\n",
        ),
        ("[[ 1/0 -eq 1 ]]; echo $?; echo alive", "1\nalive\n"),
        (
            "if [[ x == x ]]; then echo yes; fi; f() { [[ x == x ]]; }; f | cat; echo $?; ( [[ x == x ]] ); echo $?",
            "yes\n0\n0\n",
        ),
    ] {
        compare(script, expected.as_bytes());
    }
}
#[test]
fn shopt_bash_differential() {
    for (script, expected) in [
        (
            "shopt -s nullglob; set -- [; echo $#; shopt -s failglob; set -- [; echo $#",
            "1\n1\n",
        ),
        (
            "shopt -u nullglob; shopt -q nullglob; echo $?; shopt -s nullglob; shopt -q nullglob; echo $?; shopt -p nullglob",
            "1\n0\nshopt -s nullglob\n",
        ),
        (
            "shopt -u nullglob; shopt -su nullglob; echo $?; shopt -q nullglob; echo $?",
            "1\n1\n",
        ),
        (
            "shopt -s nullglob; set -- no-match-*; echo $#; shopt -u nullglob; set -- no-match-*; echo $#",
            "0\n1\n",
        ),
        (
            "shopt -s dotglob; printf '%s\\n' *; shopt -u dotglob; printf '%s\\n' *",
            ".hidden\nblocked\nchild\nhome\nterm\nvisible\nblocked\nchild\nhome\nterm\nvisible\n",
        ),
        (
            "shopt -s nocasematch; [[ AbC == a* ]]; echo $?; [[ a == [A-Z] ]]; echo $?; [[ a == [[:upper:]] ]]; echo $?; case A in a) echo yes;; esac; [[ ABC =~ ^a ]]; echo $?",
            "0\n0\n1\nyes\n0\n",
        ),
        (
            "shopt -s -o nounset; shopt -q -o nounset; echo $?; shopt -p -o nounset; shopt -u -o nounset",
            "0\nset -o nounset\n",
        ),
        (
            "shopt -u nullglob dotglob; (shopt -s nullglob); shopt -q nullglob; echo $?; shopt -s nullglob; echo $(shopt -q nullglob; echo $?)",
            "1\n0\n",
        ),
        ("shopt -q imaginary; echo $?; shopt -Z; echo $?", "1\n2\n"),
    ] {
        compare(script, expected.as_bytes());
    }
}
#[test]
fn sourcepath_is_explicit_and_unknown_options_are_not_noops() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::create_dir(d.0.join("search")).unwrap();
    std::fs::write(d.0.join("search/load"), b"echo loaded\n").unwrap();
    let actual = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "PATH=search:/usr/bin:/bin; source load; echo $?; shopt -s sourcepath; source load; shopt -s extglob; echo $?; set +o double_brackets; [[ x ]]; echo $?",
    );
    assert_eq!(actual.stdout, b"1\nloaded\n1\n2\n");
    assert!(String::from_utf8_lossy(&actual.stderr).contains("extglob: invalid shell option name"));
}
#[test]
fn syntax_incomplete_roundtrip_and_bounds() {
    use rsh::parser::{ParseError, parse};
    for source in [b"[[ x".as_slice(), b"[[ -n", b"[[ x &&", b"[[ 'open"] {
        assert!(
            matches!(parse(source), Err(ParseError::Incomplete(_))),
            "{source:?}"
        );
    }
    for source in [
        b"[[ x y ]]".as_slice(),
        b"]]",
        b"[[ ]]",
        b"[[ && x ]]",
        b"[[ x == ]]",
        b"[[ ( x ]]",
        b"[[ x ; ]]",
        b"[[ x\n]]",
    ] {
        assert!(
            matches!(parse(source), Err(ParseError::Syntax { .. })),
            "{source:?}"
        );
    }
    for source in [
        b"[[ a == a* && ! ( -z x || x > z ) ]]".as_slice(),
        b"[[ abc =~ ^a(b|c)+$ ]]",
        b"[[ x ]] >out",
    ] {
        let ast = parse(source).unwrap();
        assert_eq!(parse(&ast.to_source_bytes()).unwrap(), ast);
    }
    let deep = format!("[[ {}x ]]", "! ".repeat(1000));
    assert!(matches!(
        parse(deep.as_bytes()),
        Err(ParseError::NestingLimit)
    ));
    for byte in 0..=255u8 {
        let mut source = b"[[ ".to_vec();
        source.push(byte);
        source.extend(b" ]]");
        let _ = parse(&source);
    }
}

#[test]
fn failglob_aborts_the_command_and_shopt_listing_roundtrips() {
    let d = TempDir::new();
    d.fixtures();
    compare(
        "shopt -s failglob; echo missing-*; echo WRONG\necho next",
        b"next\n",
    );
    compare(
        "shopt -s failglob; f() { echo missing-*; echo WRONG; }; f\necho next",
        b"next\n",
    );
    let script = "shopt -s failglob; echo missing-*; echo WRONG";
    let actual = run(&d, env!("CARGO_BIN_EXE_rsh"), script);
    let reference = run(&d, "bash", script);
    assert_eq!(actual.status.code(), Some(1));
    assert_eq!(actual.status.code(), reference.status.code());
    assert!(actual.stdout.is_empty());
    assert!(!actual.stderr.is_empty());
    let actual = run(
        &d,
        env!("CARGO_BIN_EXE_rsh"),
        "shopt -s nullglob dotglob; shopt -p >options; shopt -u nullglob dotglob; . ./options; shopt -q nullglob dotglob; echo $?; type shopt; command -v shopt",
    );
    assert_eq!(actual.stdout, b"0\nshopt is a shell builtin\nshopt\n");
    assert!(rsh::interactive::builtin_names().contains(&"shopt"));
}

#[test]
fn conditional_control_flow_substitutions_files_and_binary_input() {
    let d = TempDir::new();
    d.fixtures();
    std::fs::write(d.0.join("file"), b"test").unwrap();
    std::os::unix::fs::symlink("file", d.0.join("link")).unwrap();
    let script = "set -e; if [[ -L link && link -ef file && ! -e '' ]]; then echo yes; fi; [[ $(printf x) == x && $((2+3)) -eq 5 ]]; echo $?; shopt -s nocasematch; echo $( [[ X == x ]]; echo $? ); [[ '' && $(echo BAD >&2) ]] || echo skipped";
    let output = run(&d, env!("CARGO_BIN_EXE_rsh"), script);
    assert_eq!(output.stdout, b"yes\n0\n0\nskipped\n");
    assert!(output.stderr.is_empty());
    let mut command = d.command(env!("CARGO_BIN_EXE_rsh"));
    use std::os::unix::ffi::OsStringExt;
    command.arg("-c").arg(std::ffi::OsString::from_vec(
        b"[[ '\xff' == '\xff' ]]; echo $?".to_vec(),
    ));
    assert_eq!(capture(command).0.stdout, b"0\n");
}

#[test]
fn conditional_function_body_alias_and_canonical_node_limit() {
    compare(
        "f() [[ x == x ]]; f; echo $?; shopt -s expand_aliases\nalias cond='[['\ncond x == x ]]; echo $?",
        b"0\n0\n",
    );
    let source = format!(
        "[[ {} ]]",
        std::iter::repeat_n("x", 30)
            .collect::<Vec<_>>()
            .join(" && ")
    );
    let ast = rsh::parser::parse(source.as_bytes()).unwrap();
    assert_eq!(rsh::parser::parse(&ast.to_source_bytes()).unwrap(), ast);
}
