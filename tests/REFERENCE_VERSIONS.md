# Reference shells

Recorded 2026-10-16 on macOS 26.5.1 (25F80), arm64.

| Reference | Executable | Reported version/build |
|---|---|---|
| dash | `/bin/dash` | Apple `PROGRAM:dash PROJECT:dash-16` (`what /bin/dash`); upstream version not advertised |
| bash --posix | `/bin/bash --posix` | GNU bash 3.2.57(1)-release, arm64-apple-darwin25 |

SHA-256:

- dash: `501c4a2ea3afcbf2a02d5ec81612d903e0aac3b15ab08b589a64363904cdb6e4`
- bash: `fde343ee184953c1fa1185abddeaa8be61c6acbebae4eb54db5d6b55b09a5755`

`cargo run --bin spec-runner -- --versions` prints the resolved binary paths,
versions/build metadata and binary hashes. CI prints its own versions; Linux
package versions and Homebrew/system shell versions can differ from this machine.
The test process uses LC_ALL=C and a fixed PATH, independently of which absolute
reference-shell binary the runner resolved before sanitizing the environment.

## Runner format and commands

Run `cargo run --bin spec-runner -- verify-references` before trusting a new case,
then `cargo run --bin spec-runner -- run` or `-- diff`. `--shell dash|bash|rsh`,
`--filter substring-or-glob`, and `--jobs 1..64` select runs. Each case gets a fresh
cwd/HOME and a five-second deadline; remaining descendants are cleaned up after
success as well as timeout. Output order follows sorted files and source order.

Every `#### name` needs explicit `## stdout: |` and `## status: N`. Bytes inside
the stdout block are exact, including final newlines. For output without a final
newline, place status before stdout and end the test file immediately after the
last output byte. `## stderr-match: REGEX` uses Python regular expressions on
normalized stderr only; omitted stderr expectations ignore stderr. Stdout and
status are never normalized. Only diagnostic shell prefixes and temporary paths
in stderr are normalized. Cases may have tags.

`## OK bash: {"stdout":"alternative\n","status":0}` accepts either the primary
expectation or the JSON alternative (partial fields inherit primary values).
`stderr-match` is also an alternative field. Explain the POSIX/reference decision
in a comment near the case. `## N-I dash: explanation` skips that reference only.

Reports are `target/spec-report.json` and `target/spec-junit.xml` by default.
`KNOWN_FAILURES` contains `area/file.test.sh::name | reason | classification`;
classification is a SPEC.md section-12 anchor, `extension`, or `TODO`. New failures,
stale entries that now pass, unknown IDs, malformed entries, and any reference
failure return nonzero. `--update-known-failures` refreshes an **unfiltered rsh**
run; newly recorded failures are untriaged TODOs and require review, not waivers
of the M10 core-conformance exit criterion. Do not use it to bless reference bugs.

The launcher builds rsh and the test-only limit/exec helper. Python 3.9+ is a
runner prerequisite, not a shell runtime dependency. These original POSIX-derived
cases do not copy Oils source; no Oils license/attribution exception is needed.
