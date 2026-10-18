# Contributing to rsh

Read [SPEC.md](SPEC.md) before changing behavior. It is the source of truth for
module responsibilities, grammar, semantics and milestone status. [AGENTS.md](AGENTS.md)
records repository conventions for both people and coding agents.

## Local setup

Use current stable Rust, rustfmt, Clippy, a native build toolchain, Python 3.9+,
dash and Bash. Runtime development does not require nightly; only instrumented
cargo-fuzz runs need it. Build the shell with:

```sh
cargo build --locked --bin rsh
```

Run examples with a temporary HOME and XDG_CONFIG_HOME if your own rsh startup
configuration would affect results. Top-level `-c` and script invocations load
configuration too. Tests create their own temporary fixtures.

## Verification before a pull request

```sh
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps
cargo run --bin spec-runner -- verify-references
cargo run --bin spec-runner -- run
```

To focus on a subsystem:

```sh
cargo test --test exec_diff
cargo test --test conditional
cargo test --test source
cargo test --test pty -- --test-threads=1
cargo run --bin spec-runner -- diff --filter expansion
```

`cargo test` includes other differential suites in builtin, control-flow, expansion,
redirect and job-control tests. If dash or Bash is unavailable, report which
comparisons were skipped. The specification runner requires its chosen references
and treats missing executables as an error. PTY/process tests need OS permissions
that some containers or sandboxes restrict.

Do not run every ignored test with a blanket `--ignored`: some are isolated helper
processes invoked by normal wrapper tests; others are manual benchmarks. See
[conformance documentation](docs/CONFORMANCE.md) and the test's ignore reason.

## Change conventions

- Keep patches focused. Do not advance unrelated milestones or add unrelated features.
- Keep AST data unexpanded; parsing and execution are separate stages.
- Preserve byte-oriented words, filenames and environment values. Interactive display
  may be UTF-8, but the interpreter must not assume all input is valid UTF-8.
- Put unsafe code and raw libc/nix calls only in `src/sys/`; explain each unsafe block
  with a SAFETY comment. The fork-to-exec path must remain async-signal-safe.
- Return errors for user/filesystem input; do not introduce unwrap, expect or panic
  paths for that input. Expected shell flow uses `Flow`, not Rust panics.
- Add a regression for every bug fix. Compare observable script behavior with dash
  and `bash --posix`, or Bash alone for Bash-specific syntax.
- Never change expectations simply to match rsh. Explain reference disagreements
  and deliberate deviations in SPEC section 12.
- Review Golden changes; `UPDATE_GOLDEN=1 cargo test` rewrites expected files.
  Prefer regenerating a focused test target, then inspect the diff.
- Ask before adding dependencies outside the approved list in AGENTS.md.

A pull request should explain the problem, resulting behavior, relevant SPEC
section, tests run, reference-shell differences and any remaining limitation.
Only mark a checklist item complete when its tests exist and pass.

## Documentation and release review

Keep README examples, configuration semantics and compatibility limits aligned with
code and SPEC. Verify local links and the Rust documentation build after editing.
Do not claim full Bash/POSIX conformance, completed manual terminal testing, a
performance guarantee or a finished fuzz campaign without the corresponding evidence.

Before distributing binaries, build/test the intended OS and architecture. Review
[the manual terminal checklist](tests/pty/MANUAL.md), Apache-2.0 obligations in
[LICENSE](LICENSE), and dependency licenses. Do not commit generated reports,
history files, local configuration, credentials or fuzz crash artifacts.

Contributions are intended to be distributed under the project's Apache-2.0 license.
Preserve any third-party attribution and license notices when importing material.
