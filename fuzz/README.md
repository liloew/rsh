# Lexer fuzzing (M1)

From the repository root, with cargo-fuzz and a nightly fuzzing toolchain installed:

```sh
cargo +nightly fuzz run lexer -- -max_total_time=60 -timeout=2 -max_len=65536
```

The target accepts arbitrary bytes, checks token spans, and exercises both successful
and error paths. libFuzzer detects crashes and nontermination (the timeout applies
to each input). Seeds cover quotes, substitutions, continuations and invalid UTF-8.
The shell crate itself uses stable Rust; nightly is only needed for fuzz tooling.
`cargo test` also runs deterministic byte-input and nesting stress tests without fuzz tooling.

A stable-toolchain smoke run (without sanitizer or coverage instrumentation) is also possible:

```sh
cargo run --manifest-path fuzz/Cargo.toml --bin lexer -- fuzz/corpus/lexer -runs=10000 -timeout=2
```

This smoke run does not replace a coverage-guided `cargo +nightly fuzz` campaign.
libFuzzer may extend the corpus; use a temporary copy when preserving the checked-in seeds.

# Parser fuzzing (M2)

```sh
cargo +nightly fuzz run parser -- -max_total_time=60 -timeout=2 -max_len=65536
```

The parser target checks termination/no panic and successful AST round-trips through
both byte serialization and (for valid UTF-8 source) Display. Seeds cover every M2
compound-command kind, separators, redirections, incomplete input and non-UTF-8 bytes.
Compound nesting is bounded so that arbitrary input cannot exhaust the parser stack.
A stable smoke run can use `cargo run --manifest-path fuzz/Cargo.toml --bin parser --
<temporary-corpus-directory> -runs=20000 -timeout=2`; it has no sanitizer/coverage instrumentation.

# Here-document coverage (M4)

Both corpora include heredoc queues, quoted/escaped delimiters, <<- tabs, nested
command substitutions, pipeline/group attachment, binary bodies and incomplete EOF.
Lexer fuzzing checks quoted bodies remain literal and token spans stay ordered;
parser fuzzing checks canonical heredoc source is structurally lossless. Runtime
redirection stress tests, including >1MB bodies and fd restoration, live in
`tests/redirect.rs`; fuzz targets only lex/parse and never execute arbitrary input.
