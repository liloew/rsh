//! Bash-compatible interface for shell options with implemented semantics.
use super::Outcome;
use crate::env::ShellEnv;
const NAMES: &[&str] = &[
    "dotglob",
    "expand_aliases",
    "failglob",
    "nocasematch",
    "nullglob",
    "sourcepath",
];
pub fn run(args: &[Vec<u8>], env: &mut ShellEnv) -> Outcome {
    let (mut set, mut unset, mut quiet, mut printable, mut shell_options) =
        (false, false, false, false, false);
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == b"--" {
            index += 1;
            break;
        }
        if !arg.starts_with(b"-") || arg == b"-" {
            break;
        }
        for flag in arg.get(1..).unwrap_or_default() {
            match flag {
                b's' => set = true,
                b'u' => unset = true,
                b'q' => quiet = true,
                b'p' => printable = true,
                b'o' => shell_options = true,
                _ => return Outcome::error(b"shopt", b"invalid option", 2),
            }
        }
        index += 1;
    }
    if set && unset {
        return Outcome::error(
            b"shopt",
            b"cannot set and unset shell options simultaneously",
            1,
        );
    }
    let mode = if unset {
        Some(false)
    } else if set {
        Some(true)
    } else {
        None
    };
    let names = if shell_options {
        crate::env::options::NAMES
    } else {
        NAMES
    };
    let operands = args.get(index..).unwrap_or_default();
    let selected = if operands.is_empty() {
        names
            .iter()
            .map(|n| n.as_bytes().to_vec())
            .collect::<Vec<_>>()
    } else {
        operands.to_vec()
    };
    let mut out = Outcome::default();
    for bytes in selected {
        let name = std::str::from_utf8(&bytes)
            .ok()
            .filter(|name| names.contains(name));
        let Some(name) = name else {
            let mut message = bytes;
            message.extend(b": invalid shell option name");
            out.stderr
                .extend(Outcome::error(b"shopt", &message, 1).stderr);
            out.status = 1;
            continue;
        };
        if !operands.is_empty()
            && let Some(value) = mode
        {
            env.set_option(name, value);
        }
        let enabled = env.option(name).unwrap_or(false);
        if operands.is_empty() && mode.is_some_and(|value| value != enabled) {
            continue;
        }
        if !enabled && (quiet || !operands.is_empty() && mode.is_none()) {
            out.status = 1;
        }
        if quiet || mode.is_some() && !operands.is_empty() {
            continue;
        }
        if printable {
            if shell_options {
                out.stdout
                    .extend(if enabled { b"set -o " } else { b"set +o " });
            } else {
                out.stdout
                    .extend(if enabled { b"shopt -s " } else { b"shopt -u " });
            }
            out.stdout.extend(name.as_bytes());
            out.stdout.push(b'\n');
        } else {
            out.stdout
                .extend(format!("{name:<14}\t{}\n", if enabled { "on" } else { "off" }).as_bytes());
        }
    }
    out
}
