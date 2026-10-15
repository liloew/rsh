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
