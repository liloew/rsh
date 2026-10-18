# AGENTS.md

Instructions for Codex (and any coding agent) working in this repository.

## Project

`rsh` is a POSIX-compatible shell written in Rust with a fish-style interactive layer. **`SPEC.md` is the source of truth** for the grammar, semantics, milestones, and test strategy. Read it before changing behavior. If code and SPEC disagree, ask or fix the code; only edit SPEC deliberately and say so.

## Commands

```sh
cargo build
cargo test                        # all tests
cargo test --test exec_diff       # external-command differential tests; cargo test covers the other suites
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
UPDATE_GOLDEN=1 cargo test        # regenerate golden files (review the diff!)
```

Before finishing any task: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` must all pass.

Differential tests need `dash` and `bash` installed. If they are missing, say so and skip those tests; do not delete them.

## Layout

```
src/
  lexer/  parser/  ast/  expand/  exec/  redirect/
  builtins/  env/  jobs/  sys/  interactive/  main.rs
tests/
  golden/   diff/   spec/   pty/
SPEC.md  AGENTS.md
```

Module responsibilities are in SPEC section 2. Do not blur them (e.g. no expansion logic in the parser, no parsing in `exec`).

## Working rules

1. **One milestone at a time.** Work in the order of SPEC section 10. Do not start milestone N+1 until N's tests pass. State which milestone you are on at the start of each task.
2. **Tests first or alongside.** Add or update tests with every behavior change. Bug fix means regression test.
3. **Small, reviewable changes.** Prefer one focused commit per feature. Do not mix refactors with behavior changes.
4. **Check boxes honestly.** Only tick a milestone item in SPEC.md when the tests for it exist and pass.

## Code conventions

- Rust stable, edition 2024 or later. No nightly features.
- **`unsafe` and raw `libc`/syscalls live only in `src/sys/`.** Expose safe wrappers. Every `unsafe` block gets a `// SAFETY:` comment.
- No `unwrap()`, `expect()`, `panic!`, or unchecked indexing on anything derived from user input, environment, or the filesystem. Use `Result` with `thiserror` error types. `unwrap` is fine in tests.
- Use `OsString`/`Vec<u8>` for words, paths, and environment values; do not assume UTF-8.
- Keep `ast` types plain data. Keep functions short; prefer enums and `match` over stringly-typed logic.
- After `fork`, only call async-signal-safe functions before `exec`. Restore default signal dispositions and set the process group in the child.
- Prefer these crates: `nix` (or `rustix`), `signal-hook`, `reedline`, `thiserror`, `glob`-like matching implemented in-house if POSIX bracket semantics are not met. Ask before adding other dependencies.
- Error messages follow `rsh: <context>: <message>` and go to stderr.

## Definition of done (per task)

- [ ] Behavior matches SPEC (cite the section)
- [ ] New/updated tests, including at least one edge case
- [ ] fmt, clippy, tests all green
- [ ] Differential test added when behavior is observable from a script
- [ ] SPEC.md checklist or deviations section updated if relevant
- [ ] Short summary: what changed, what was tested, anything left open

## Do not

- Do not shell out to `sh`, `bash`, or `dash` from the implementation (only tests may).
- Do not implement bash/fish-only syntax in the core language unless it is a SPEC milestone M11 item behind an option.
- Do not silently change behavior to make a test pass; if a test seems wrong, explain why and compare against `dash` and `bash --posix`.
- Do not modify or delete golden/spec files without reviewing the diff and explaining it.
- Do not run destructive commands (`rm -rf`, `kill -9 -1`, etc.) in tests outside temp directories.

## When unsure

Check, in order: SPEC.md, then the POSIX Shell Command Language spec, then what `dash` does, then `bash --posix`. If still ambiguous, pick the POSIX behavior, record it in SPEC section 12, and mention it in your summary.
