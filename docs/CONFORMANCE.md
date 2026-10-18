# Compatibility and verification

rsh targets POSIX-oriented shell scripting on Linux and macOS, with explicitly
selected extensions. It is experimental and has not completed the full M10
conformance/performance campaign. A passing finite suite is not POSIX certification
or a guarantee that arbitrary Bash startup files will work.

## Current automated baseline

As checked locally on macOS arm64 with stable Rust on 2026-10-18:

| Check | Result |
| --- | --- |
| Specification cases under rsh | 750 passed, no failures or timeouts. |
| dash + Bash reference verification | 1,495 passed, 5 declared dash pipefail skips. |
| Rust tests, including the crate documentation example | 205 passed, 12 ignored entries. |
| Conditional/shopt comparison reference | Homebrew Bash 5.3.15; some older comparisons use system Bash 3.2.57. |
| Previously recorded lexer/parser fuzz smoke (2026-10-16) | 20,000 runs each, without sanitizer/coverage instrumentation. |

Counts can change with new tests; rerun the commands below for the current checkout.
See [reference versions](../tests/REFERENCE_VERSIONS.md) for paths, hashes and format
details. Linux-only /proc checks are not exercised by a macOS run. Existing CI is
configured for Linux/macOS and repeated PTY tests, but local documentation does not
assert that a particular remote CI run has passed.

## Standard checks

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps
```

The test suites cover lexer/parser Golden files and roundtrips; expansion, arithmetic
and patterns; fd order/restoration, large here-documents and here-strings; external
commands, functions/control flow, builtin/options errors, signals/jobs, and editor/motd
PTY interactions. Fixtures use temporary directories, controlled HOME and LC_ALL=C.
The legacy CLI/M0 differential helpers also isolate HOME/cwd so startup cannot
accidentally load the developer's own rsh configuration.

Tests with `#[ignore]` are not all pending failures: several are isolated helpers
invoked by ordinary wrapper tests so process/fd observations do not race other tests.
Others are opt-in timing references. Check each annotation before running one.

## Specification runner

The launcher builds `rsh` and the test-only resource-limit helper; Python 3.9+,
dash and Bash are needed for reference verification.

```sh
cargo run --bin spec-runner -- --versions
cargo run --bin spec-runner -- verify-references --report-dir target/spec-references
cargo run --bin spec-runner -- run
cargo run --bin spec-runner -- diff --filter expansion
cargo run --bin spec-runner -- --shell dash --filter quoting
```

Each case runs in a separate temporary directory with a fixed minimal environment,
LC_ALL=C and a five-second deadline. Parallelism is bounded (`--jobs 1..64`), and
output ordering is deterministic. The runner cleans up remaining process-group
members. It is test isolation, not a security boundary for arbitrary hostile code.

Case files live in `tests/spec/<area>/*.test.sh`. Verify references before accepting
rsh results or changing expected output. `## OK bash:` records a justified alternative;
`## N-I dash:` skips an explicitly unsupported reference feature. Only stderr
normalizes shell diagnostic names and temporary paths; stdout and status stay exact.

Default outputs:

- `target/spec-report.json`: results, per-area counts and policy errors.
- `target/spec-junit.xml`: CI-compatible JUnit report.
- Reference reports go to a separate directory with `--report-dir`.

`KNOWN_FAILURES` is optional and currently absent. Missing means no exemptions;
unexpected failures still fail the run. If needed, entries must have an exact case
ID, reviewed reason and classification/link to SPEC section 12. Stale entries also
fail, so resolved failures must be removed. `--update-known-failures` is a triage tool,
not a way to redefine correct behavior; it can recreate the manifest even when empty.

## Selected compatibility boundaries

| Area | Current policy |
| --- | --- |
| Startup | rsh config loads for all top-level modes; Bash/system profile files never load implicitly. |
| `source` / `.` | Bare names use cwd by default; `shopt -s sourcepath` opts into PATH-first lookup. |
| Dot arguments | Explicit temporary arguments are always restored; argument-free changes persist. |
| `<<<` / `[[ ... ]]` | Enabled by default, gated at execution by here_strings/double_brackets; disabling does not make parsing strictly POSIX. |
| `shopt` | Six implemented names plus `-o` for implemented set options; unknown/unimplemented names fail. |
| Pattern matching | Byte-oriented C-locale semantics; no extglob/globstar extension. |
| `=~` | POSIX ERE with quote-aware literals; no BASH_REMATCH capture array. |
| Arithmetic | Signed 64-bit wrapping, bounded recursion; no complete Bash arithmetic/array language. |
| `echo` | Only `-n` is processed; backslashes stay literal. Prefer printf. |
| Optional builtins | `hash` is a no-op; `ulimit` is not implemented. |
| Other Bash syntax | Arrays, local, namerefs, process substitution, select, coprocesses and the function keyword remain unsupported. |
| Interactive exit | Warn once for stopped jobs; send HUP then CONT to non-exempt remaining jobs on exit. |

The authoritative, more detailed decisions and pinned regression tests are in
[SPEC section 12](../SPEC.md#12-deliberate-deviations-from-posix). Do not add a waiver
for a core POSIX bug to claim milestone completion.

## Fuzzing, stress and performance

See [fuzz/README.md](../fuzz/README.md) for lexer/parser targets. Both parse arbitrary
bytes and never execute generated shell input. Parser fuzzing checks lossless AST
serialization as well as termination. Copy seed corpora to a temporary location if
you do not want libFuzzer to mutate the checked-in corpus.

A stable smoke run lacks sanitizer/coverage instrumentation and does not replace an
instrumented campaign. The requested 24 CPU-hour campaign, grammar-based three-shell
differential fuzzer, broad robustness matrix and dash-relative performance targets
are not complete. No 3x performance guarantee or 25% regression gate is claimed.

Existing normal tests already include large here-documents/substitutions, recursive
limits, fd restoration/leak checks, process reaping, 200 short background jobs and
an isolated 100,000-iteration loop. Some Linux-only checks inspect /proc.

Opt-in UI timing references can be run separately:

```sh
cargo test --lib highlight_200_chars_benchmark -- --ignored --nocapture
cargo test --lib path_2000_entries_benchmark -- --ignored --nocapture
cargo test --test pty motd::absent_file_startup_timing -- --ignored --nocapture
```

These are developer observations, not CI performance thresholds. Full-system and
real-terminal manual checks remain in [tests/pty/MANUAL.md](../tests/pty/MANUAL.md).
