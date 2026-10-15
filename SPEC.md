# SPEC: `rsh`, a POSIX-compatible shell in Rust with a fish-style interactive layer

> `rsh` is a placeholder name. Rename freely.

## 1. Goals and non-goals

**Goals**
- A shell whose *language* follows POSIX.1-2024 Shell Command Language (`sh`), so ordinary `sh` scripts run unmodified.
- A shell whose *interactive experience* is fish-like: syntax highlighting, autosuggestions, good tab completion, sane defaults.
- Safe, readable Rust. All `unsafe` and raw syscalls are isolated in one module.
- Behavior verified by differential testing against `dash` and `bash --posix`.

**Non-goals (initially)**
- Full bash compatibility (arrays, `[[ ]]`, process substitution, `select`, coprocesses). These are optional extensions (Milestone 9).
- fish *syntax* (`set`, `end`, `function ... end`). We copy fish's UX, not its language.
- Windows support. Target Linux and macOS.

**Normative references**
- POSIX Shell Command Language: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
- Bash Reference Manual (for documented bash extensions): https://www.gnu.org/software/bash/manual/bash.html
- glibc manual, "Implementing a Job Control Shell": https://www.gnu.org/software/libc/manual/html_node/Implementing-a-Shell.html

When this file and POSIX disagree, POSIX wins unless this file marks a deliberate deviation in section 12.

## 2. Architecture

Cargo workspace or single crate with these modules (one responsibility each):

| Module | Responsibility |
|---|---|
| `lexer` | Source text to tokens. Context-sensitive (quoting, heredocs, alias expansion hook). |
| `parser` | Tokens to AST. Hand-written recursive descent following section 4. |
| `ast` | Plain data types. No behavior beyond `Display`/`Debug`. |
| `expand` | Tilde, parameter, command substitution, arithmetic, field splitting, globbing, quote removal. |
| `exec` | Walks the AST: simple commands, pipelines, lists, compound commands, functions. |
| `redirect` | Applying and restoring file descriptor redirections. |
| `builtins` | Built-in commands (section 7). |
| `env` | Variables, exported environment, positional parameters, function table, alias table, shell options. |
| `jobs` | Process groups, job table, `fg`/`bg`/`jobs`/`wait`, SIGCHLD handling. |
| `sys` | The only place `unsafe`, `libc`, or raw `nix` calls may appear. Thin safe wrappers. |
| `interactive` | Line editor, prompt, history, highlighting, suggestions, completion. |
| `main` | Argument parsing (`-c`, `-s`, script file, `-i`, `-l`), mode selection. |

Data flow: `source -> lexer -> parser -> AST -> exec (calls expand/redirect/builtins/jobs) -> exit status`.

Key design rules:
- The parser is **incremental-friendly**: it must report "incomplete input" (unclosed quote, `if` without `fi`, pending heredoc) distinctly from "syntax error", so the REPL can show a continuation prompt.
- Expansion happens **at execution time**, never at parse time. The AST keeps words as unexpanded structured parts (see 4.3).
- Errors are values (`Result<_, ShellError>`). No `panic!`/`unwrap()` on user-controlled input.

## 3. Lexical structure

Follow POSIX token recognition (section 2.3 of the standard):

1. Input is read as bytes. Treat as UTF-8 where possible, but do not require valid UTF-8 for words or filenames (use `OsString`/byte strings internally).
2. Operators: `&& || ;; ; & | ( ) < > << >> <& >& <> <<- >|` and newline.
3. Quoting: backslash, single quotes, double quotes. Inside double quotes `$`, `` ` ``, `\`, and `"` keep special meaning per POSIX.
4. Expansions recognized by the lexer as parts of a word: `$name`, `${...}`, `$(...)`, `` `...` ``, `$((...))`.
5. Comments: `#` at the start of a word to end of line.
6. Here-documents: after the lexer sees `<<` or `<<-` with a delimiter, the body is read after the next newline. Quoted delimiter means no expansion in the body. `<<-` strips leading tabs.
7. Reserved words are recognized only in command position: `if then elif else fi do done case esac while until for in { } ! function`(`function` is an extension, off by default).
8. Aliases are expanded in command position, before reserved-word recognition, only in interactive mode or when `expand_aliases` is set.

## 4. Grammar

### 4.1 Commands (EBNF, derived from POSIX 2.10)

```
complete_command = list [ separator_op ] ;
list             = and_or { separator_op and_or } ;
and_or           = pipeline { ( "&&" | "||" ) linebreak pipeline } ;
pipeline         = [ "!" ] command { "|" linebreak command } ;

command          = simple_command
                 | compound_command { redirection }
                 | function_definition ;

compound_command = brace_group | subshell | for_clause | case_clause
                 | if_clause | while_clause | until_clause ;

subshell         = "(" compound_list ")" ;
brace_group      = "{" compound_list "}" ;
compound_list    = linebreak term [ separator ] ;
term             = and_or { separator and_or } ;

for_clause       = "for" NAME ( do_group
                             | sequential_sep do_group
                             | linebreak "in" { WORD } sequential_sep do_group ) ;
case_clause      = "case" WORD linebreak "in" linebreak { case_item } "esac" ;
case_item        = [ "(" ] pattern ")" ( compound_list | linebreak ) [ ";;" linebreak ] ;
pattern          = WORD { "|" WORD } ;
if_clause        = "if" compound_list "then" compound_list
                   { "elif" compound_list "then" compound_list }
                   [ "else" compound_list ] "fi" ;
while_clause     = "while" compound_list do_group ;
until_clause     = "until" compound_list do_group ;
do_group         = "do" compound_list "done" ;

function_definition = NAME "(" ")" linebreak compound_command { redirection } ;

simple_command   = { assignment | redirection } [ WORD { WORD | redirection } ] ;
assignment       = NAME "=" [ WORD ] ;        (* only before the command name *)

redirection      = [ IO_NUMBER ] redir_op WORD
                 | [ IO_NUMBER ] ( "<<" | "<<-" ) HEREDOC_DELIM ;
redir_op         = "<" | ">" | ">>" | ">|" | "<&" | ">&" | "<>" ;

separator_op     = "&" | ";" ;
separator        = separator_op linebreak | newline_list ;
sequential_sep   = ";" linebreak | newline_list ;
linebreak        = { NEWLINE } ;
newline_list     = NEWLINE { NEWLINE } ;
```

### 4.2 Parser behavior requirements
- `IO_NUMBER` is a digit-only token *immediately* followed by `<` or `>`.
- A word is a command name or argument unless it matches `assignment` before the first non-assignment word.
- `NAME` follows `[A-Za-z_][A-Za-z0-9_]*`.
- Syntax errors report line and column and the unexpected token. Message format: `rsh: line 3: syntax error near unexpected token 'fi'`.
- Return `ParseError::Incomplete(reason)` for EOF inside quotes, compound commands, command substitution, or pending heredoc.

### 4.3 AST sketch

```rust
pub struct Script(pub Vec<CompleteCommand>);

pub enum Command {
    Simple(SimpleCommand),
    Compound(CompoundCommand, Vec<Redirection>),
    FunctionDef { name: String, body: Box<CompoundCommand>, redirs: Vec<Redirection> },
}

pub struct Pipeline { pub negated: bool, pub commands: Vec<Command> }
pub struct AndOr { pub first: Pipeline, pub rest: Vec<(AndOrOp, Pipeline)> }

pub struct Word(pub Vec<WordPart>);
pub enum WordPart {
    Literal(String),
    SingleQuoted(String),
    DoubleQuoted(Vec<WordPart>),
    Param(ParamExpr),            // $x, ${x:-y}, ${#x}, ${x%%pat} ...
    CommandSub(Box<Script>),     // $(...) and `...`
    Arith(Box<ArithExpr>),       // $((...))
    Tilde(Option<String>),
    Glob(GlobChar),              // unquoted * ? [
}
```

## 5. Expansion order (execution time)

For each word in a simple command, in this order (POSIX 2.6):
1. Tilde expansion, parameter expansion, command substitution, arithmetic expansion (left to right).
2. Field splitting (using `IFS`) on the *results of unquoted expansions only*.
3. Pathname expansion (globbing), unless `set -f`.
4. Quote removal.

Parameter forms to support: `$x ${x} ${x:-w} ${x-w} ${x:=w} ${x=w} ${x:?w} ${x?w} ${x:+w} ${x+w} ${#x} ${x%p} ${x%%p} ${x#p} ${x##p}` and special parameters `$@ $* $# $? $- $$ $! $0 $1..`. `"$@"` must expand to separate fields.

Arithmetic: signed 64-bit integers, C operators, variables by name, `$((...))`.

M5 execution details:
- Intermediate bytes retain independent quote and field-splitting eligibility flags. Empty quoted segments have explicit positional markers; only unquoted expansion results are split. Assignments and non-interactive redirection targets are scalar contexts, without splitting or pathname expansion. Here-documents with unquoted delimiters perform parameter/command/arithmetic expansion only.
- `expand/pattern.rs` owns C-locale byte pattern compilation/matching for both pathname expansion and parameter prefix/suffix removal; M6 case execution must reuse it. Pathname traversal matches one directory component at a time and sorts byte results. `set -f`/`+f` control globbing; the minimal set handler also accepts `--` positional arguments and the existing M4 `-C`/`+C` flag. Remaining builtin/options behavior stays M7.
- Positional arguments come from `-c command [arg0 [args...]]`, script arguments, stdin arguments or minimal `set --`. Worker snapshots preserve positional parameters, logical cwd, options, original shell pid and latest background pid. Snapshots and source use a length-delimited anonymous fd transport, not argv-sized variable values.
- Command substitution re-execs rsh with cloned shell state and captures stdout in a CLOEXEC anonymous seekable file, preserving stderr, waiting/reaping the child and stripping every trailing newline. No interpreter or allocator runs between fork and exec. Captured NUL bytes are discarded (as in the reference shells); other bytes are preserved. Pipeline/subshell expansion side effects stay in the child environment.
- Arithmetic includes assignment/compound assignment to variable names, right-associative assignment, comma and lazy logical/conditional operators. Signed 64-bit overflow wraps; shift counts use their low six bits and right shifts sign-extend. Division by zero is a fatal expansion error, not a panic. Expansion errors abort the non-interactive shell with status 2. See section 12 for reference differences and resource limits.


## 6. Execution semantics

- **Simple command**: perform expansions; handle assignments (persist if no command, else scoped to the command's environment); apply redirections; dispatch in order: special builtin, function, regular builtin, external (PATH lookup).
- **Exit status**: 0 success; 1-125 failure; 126 found but not executable; 127 not found; 128+N killed by signal N.
- **Pipeline**: each command in its own process (fork), connected by pipes; status is that of the last command (`set -o pipefail` as extension). `!` negates.
- **Lists**: `;` sequential, `&` background (own process group when job control active), `&&`/`||` short-circuit with equal precedence, left-associative.
- **Subshell** `( ... )`: forks. **Brace group** `{ ...; }`: current shell.
- **Functions**: own positional parameters, share variables (no `local` in POSIX; `local` is an optional extension). `return` exits the function.
- **Redirections**: processed left to right; restored after builtins/functions/compound commands. `n>&m` duplicates; `n>&-` and `n<&-` close. Here-doc bodies go through a pipe or temp file.
- **Redirection errors (M4)**: the command is not executed; diagnostics use `rsh: <path-or-fd>: <reason>`. Status is 1 for ordinary commands and 2 for special builtins. A direct special-builtin redirection error aborts the non-interactive shell, with parent descriptors restored before returning.
- **`set -e` (errexit)**, `-u` (nounset), `-x` (xtrace), `-f` (noglob), `-n` (noexec), `-o pipefail`: implement with POSIX exceptions for `errexit` (not triggered in `if`/`while` conditions, `&&`/`||` non-final commands, or `!` pipelines).
- **`trap`**: `EXIT` and signal traps; run in the shell's context.

## 7. Builtins

**Special builtins** (assignments persist; errors may abort a non-interactive shell):
`: . break continue eval exec exit export readonly return set shift times trap unset`

**Regular builtins:**
`cd pwd echo printf test [ read type command alias unalias getopts umask wait kill jobs fg bg true false ulimit(later) hash(later)`

Specifics:
- `cd`: handle `-`, `CDPATH`, `-L/-P`; update `PWD` and `OLDPWD`.
- `echo`: no option processing beyond `-n`; recommend `printf`.
- `printf`: full POSIX format support including `%b`, `%s`, `%d`, `%x`, `%o`, `%c`, width/precision.
- `test`/`[`: full POSIX expression grammar, including file tests and string/integer comparisons.
- `read`: `-r`, IFS splitting, multiple variables; byte-at-a-time input avoids consuming subsequent commands' data.
- M7 implements the special/regular builtin set above except `jobs`, `fg`, `bg` (M8) and optional `ulimit`. `hash` is an explicitly documented no-op, including `hash -r`.
- `.` searches PATH for slashless names; supplied arguments temporarily replace positional parameters, while an invocation without arguments shares them. Sourced files may return. `eval` parses in the current environment; `exec command` replaces the process without forking, and operand-less `exec` retains redirections.
- `export`/`readonly -p`, `unset -v/-f`, variable listings and trap listings use shell quoting. Readonly writes fail; direct special-builtin usage/assignment errors abort non-interactive execution. `command` bypasses functions and `-p` changes lookup without changing the child's PATH.
- `set` supports `-e -u -x -f -n -C`, named options and `pipefail`, positional parameters, and option listings. Startup accepts these options plus `-c`, `-s`, `--`, `-i`, `-l`; interactive UI/job control and login startup files remain M8/M9. Aliases expand only with interactive mode or `set -o expand_aliases`, at eligible command positions via the lexer/parser hook.
- Signal handlers only record pending signals; actions execute between commands in shell context. EXIT actions preserve the prior status unless explicitly overridden. Caught actions reset in subshells/command substitutions; ignored signals remain ignored. A substitution consisting solely of `trap` can inspect the parent's trap list without executing its EXIT action.
- Shell defaults initialize IFS, PATH, prompts, OPTIND/OPTERR, LINENO and PPID when absent, and HOME from passwd when needed. PWD/OLDPWD track directory changes; OPTARG remains unset when no option argument exists.

## 8. Job control and signals

- Job control is active only when interactive and stdin is a tty.
- Shell puts itself in its own process group and takes the terminal (`tcsetpgrp`). Children call `setpgid` (in both parent and child, to avoid the race), restore default signal dispositions, and foreground jobs get the terminal.
- Shell ignores `SIGINT`, `SIGQUIT`, `SIGTSTP`, `SIGTTIN`, `SIGTTOU` interactively; handles `SIGCHLD` via a self-pipe or `signal-hook`.
- Track job states: running, stopped, done. Report completions before the next prompt.
- `fg`, `bg`, `jobs`, `wait`, `kill %n`, `disown` (extension).
- Non-interactive background jobs ignore `SIGINT`/`SIGQUIT` and read stdin from `/dev/null`.

## 9. Interactive layer (fish-style UX)

Built on `reedline` (preferred) or `rustyline`.
- Prompt: configurable via `PS1`/`PS2` (POSIX) with a sensible default (cwd, git branch optional, status color on non-zero `$?`).
- Syntax highlighting using the real lexer: valid command in one color, unknown command in red, strings, variables, operators.
- Autosuggestions from history (accept with Right arrow or End).
- History: file at `~/.local/share/rsh/history`, dedup, timestamps, `Ctrl-R` search.
- Tab completion: commands (PATH, builtins, functions, aliases), file paths, variables after `$`, and pluggable per-command completion later.
- Multi-line editing and continuation prompts driven by `ParseError::Incomplete`.
- Config: `~/.config/rsh/config.sh` is sourced on interactive start. Login shells source `/etc/profile` and `~/.profile`.

## 10. Milestones

Each milestone is done only when its tests pass and `cargo clippy -- -D warnings` is clean.

- [x] **M0 Skeleton**: crate layout, CI (`fmt`, `clippy`, `test`), `-c 'cmd'` runs a single external command, exit status propagated.
  - M0 scope: `-c` accepts literal words separated by spaces/tabs, including non-UTF-8 bytes; empty input succeeds. Quoting, operators, expansion and full shell grammar remain deferred to M1 and later. Spawn errors currently report a diagnostic and status 1; 126/127 classification remains M3 work. Verified by `tests/cli.rs` and `tests/diff.rs`; CI checks Linux and macOS.
- [x] **M1 Lexer**: all tokens and quoting in section 3 except heredocs; golden token tests.
  - M1 exposes a byte-based lexer with physical token spans and structured, unexpanded words. Substitution bodies remain source bytes for later parsing; reserved words remain WORD tokens. Here-document collection was subsequently added in M4. Unit tests, `tests/golden/lexer`, and `fuzz/fuzz_targets/lexer.rs` cover recognition and robustness. The M0 CLI remains unchanged; integration with shell grammar is M2 work.
- [x] **M2 Parser**: full grammar in 4.1 minus heredocs and functions; golden AST tests; `Incomplete` vs `Error` distinction.
  - M2 implements recursive descent, ordered simple-command parts, compound commands, contextual reserved words, distinct incomplete/syntax errors, canonical Display and lossless byte serialization. `--dump-ast -c` exposes the AST without execution; ordinary `-c` stays at M0 until M3. Heredocs were subsequently added in M4 and functions in M6. Covered by parser unit/property tests, `tests/golden/parser`, syntax differential tests, and the parser fuzz target.
- [x] **M3 Basic execution**: simple commands, PATH lookup, assignments, pipelines, `&&`/`||`/`;`, exit statuses 126/127, `!`.
  - M3 replaces the M0 whitespace bootstrap with AST execution for simple commands, pipelines, sequential/AND-OR/background lists, and status propagation. Supports `-c`, script files, and stdin (`-s` or no arguments). Only `:`, `true`, `false`, `exit`, `cd`, `pwd`, `export` are builtins; remaining builtin behavior is TODO(M7). M4 subsequently adds redirections and brace/subshell execution; other compound execution remains M6.
  - M3 argument realization is deliberately minimal: literal/quoted bytes, basic `$name`/`${name}` and `$?`/`${?}` values. It does not implement IFS splitting, glob/tilde expansion, command substitution, arithmetic or parameter operators (M5). Complex expansions produce an explicit unsupported diagnostic; glob/tilde markers retain their literal spelling until M5. Positional parameters and interactive operation are still deferred.
  - `sys` owns all unsafe/raw process calls. Arguments, environment, diagnostics and builtin output are prepared before fork. Child paths only restore signals, set the process group, duplicate/close descriptors, chdir, execve, write prepared bytes and _exit. Pipeline builtins use cloned shell state. Background AND-OR lists re-exec an internal rsh worker with an explicit byte-preserving variable/export snapshot; stdin is /dev/null and SIGINT/SIGQUIT are ignored. Finished background workers are reaped at command boundaries; full SIGCHLD/job control remains M8.
  - Tested with 17 execution golden/differential fixtures plus CLI modes, PATH permission fallback/scoping, non-UTF-8 environments, NUL rejection, nonblocking background operation, and isolated no-child/no-zombie checks after pipelines. Temporary directories, controlled PATH, HOME and LC_ALL=C isolate execution fixtures.
- [x] **M4 Redirections and heredocs**: all redir ops, fd duplication/closing, here-docs (quoted and unquoted, `<<-`).
  - M4 adds an ordered redirection plan shared by child execution and parent fd guards. All raw calls stay in sys. Parent guards save above user-referenced descriptors with CLOEXEC, restore closed/open states and original flags on all paths, and relocate hidden backups if a later dynamic fd operand would expose or overwrite them. A noclobber state flag exists, without implementing the M7 set builtin; `>|` forces truncation.
  - Lexer queues multiple heredocs per lexical context, including nested command substitutions, and collects them after the next newline. AST data retains the delimiter word, quote flag, <<- flag, literal body bytes and unexpanded WordPart list. Quoted/escaped delimiters suppress expansion; unquoted body execution is deliberately literal with TODO(M5). Display emits queued bodies after newlines; incomplete EOF maps to ParseError::Incomplete.
  - Small bodies (at most 512 bytes, within POSIX's guaranteed pipe capacity) use prefilled pipes; larger bodies use 0600 temporary files unlinked immediately before writing and rewound for input. All internal descriptors are CLOEXEC and explicitly closed in children when no longer needed. Internal re-exec workers receive source through a dedicated fd, so >1MB heredocs do not depend on ARG_MAX.
  - Only brace/subshell routing and redirection-only exec are added to support M4. No if/loop/case/function execution or command substitution evaluation is introduced. Parent builtin/group guards restore stdin/stdout/stderr; subprocess groups leave parent descriptors untouched, and re-exec workers preserve explicitly closed stdio despite Rust startup sanitization.
  - Tested by 16 redirection golden scripts, three-shell differential cases, lexer/parser heredoc goldens and round-trips, binary data, >1MB runtime tests (including unused input and pipeline worker sources), immediate-unlink checks, fd restoration/error paths and dynamic-fd collision regressions. Linux-only /proc/self/fd tests cover pipe and file-backed heredocs plus hidden restore descriptors; those checks are conditional on Linux. Both fuzz corpora include heredoc seeds and round-trip invariants.
- [x] **M5 Expansion**: tilde, full parameter expansion, command substitution, arithmetic, field splitting, globbing, quote removal; `"$@"` semantics.
  - Implemented in `src/expand` with execution-time integration only; parser ASTs remain unexpanded. Tests cover nine expansion golden scripts, differential scripts vs dash/bash --posix, unit arithmetic/pattern/splitting tests, deterministic byte robustness and split/join properties, >1MB captures and variable snapshots, passwd fallback, pipeline/subshell scope, and isolated descriptor/reaping checks. A Linux-only /proc/self/fd assertion additionally checks child inheritance; it is conditional on Linux. No M6 control flow or function implementation is introduced.

- [x] **M6 Control flow and functions**: `if/while/until/for/case`, `break/continue`, functions, `return`, subshells, brace groups.
  - AST and parser support NAME() compound bodies with attached redirections, lossless source/Display and --dump-ast. Runtime implements all compound kinds and Flow::{Status,Break,Continue,Return,Exit}; loops consume only their own signals and functions consume Return. Function calls restore positional parameters and temporary prefix assignments on success and error, share other variables, and preserve $0. Definitions are stored as immutable Rc bodies so redefinition during an active call is safe.
  - The dispatch order is special builtin, function, regular builtin, external. Every POSIX special-builtin name is protected at definition time, including names whose implementation remains M7. Only break/continue/return/shift are added; read/trap and other remaining builtins stay M7. Subshells/pipeline elements still fork through sys and immediately re-exec before interpreting Rust, using byte-preserving snapshots that now include functions and call depth.
  - Tests: 12 control golden/differential scripts; function AST golden; generated AST function round-trips; incomplete/syntax/name checks; control/return/shift/error restoration; recursion limits; two consecutive 100,000-iteration in-process loops with stable descriptor count and bounded peak-RSS growth; no remaining children. Lexer/parser fuzz seeds include the new grammar nodes and each target completed 20,000 stable smoke runs (no sanitizer/coverage instrumentation).

- [x] **M7 Builtins and options**: section 7's M7 builtin set, `set` options, `trap`, `getopts`, `eval`, `.`; jobs/fg/bg remain M8 and optional ulimit is deferred.
  - Implemented and audited special-builtin persistence/error handling, source/eval/exec, readonly variables, formatting/tests/read, command lookup, aliases, getopts, umask, PID wait/kill, traps and startup/runtime options. hash is an explicit no-op. No job-control or interactive/login UI was added.
  - Validation: cargo fmt --all and format check; cargo clippy --all-targets -- -D warnings; cargo test (121 passed, 4 isolated helper entries ignored by the main harness and invoked by their isolation tests). 32 builtin golden fixtures, 32 common three-shell differential fixtures and 19 builtin integration tests pass. Linux aarch64 cross-target Clippy also passes; Linux runtime-only /proc checks were not executed on this macOS host. Fuzz workspace fmt/Clippy pass and parser stable smoke completed 20,000 runs without sanitizer/coverage instrumentation. Reference differences and implementation limits are recorded in section 12.
- [ ] **M8 Job control**: section 8, interactive and non-interactive behavior, signal handling correctness.
- [ ] **M9 Interactive UX**: section 9 (REPL, history, highlighting, suggestions, completion, continuation prompts).
- [ ] **M10 Conformance pass**: run the ported Oils spec-test cases and differential fuzzing; fix or document every deviation.
- [ ] **M11 Optional extensions** (each behind a flag/option): arrays, `[[ ]]`, `local`, `function` keyword, `{a,b}` brace expansion, `<(...)`, `pipefail` default, `select`, `**` globstar.

## 11. Test strategy

**Layers**
1. **Unit tests** (in-module `#[cfg(test)]`): lexer tokens, parser output, individual expansions, arithmetic evaluator, glob matcher.
2. **Golden tests** (`tests/golden/*.sh` + `.ast` / `.out`): AST dumps and script output compared to checked-in files. Update with `UPDATE_GOLDEN=1 cargo test`.
3. **Differential tests** (`tests/diff/*.sh`): run each script under `rsh`, `dash`, and `bash --posix`; compare stdout, stderr (normalized), and exit status. Any case where `dash` and `bash --posix` agree and `rsh` differs is a bug. Where they disagree, record the chosen behavior in the test header.
4. **Spec-case tests**: port cases from the Oils spec tests (https://www.oilshell.org/release/latest/test/spec.wwz/) into a simple format:
   ```
   #### test name
   <script>
   ## stdout: expected
   ## status: 0
   ```
   Keep a `KNOWN_FAILURES` list; CI fails if a listed case starts passing without being removed.
5. **PTY tests** (`rexpect` or `expectrl`): interactive behavior, prompts, job control (`Ctrl-Z`, `fg`, `bg`), history, completion, continuation prompts.
6. **Fuzzing**: `cargo-fuzz` targets for the lexer and parser (never panic, always terminate); property test that `parse(display(ast)) == ast` for generated ASTs.

**Rules**
- Every bug fix lands with a regression test.
- Tests must not depend on the host's `$HOME`, locale, or PATH beyond coreutils; set `LC_ALL=C` and use temp dirs.
- Tests that fork must clean up children; assert no zombies in job-control tests.

## 12. Deliberate deviations from POSIX

- Lexer resource limit: recursive quote/substitution scanning is capped at 64 scanner levels (`lexer::MAX_NESTING`); excess nesting returns `LexError::NestingLimit` rather than risking stack exhaustion. This is an implementation limit, not a syntax rule. Pinned by `lexer::tests::deep_nesting_returns_an_error_without_stack_overflow`.

- Parser resource limit: compound-command nesting is capped at 64 command levels (`parser::MAX_NESTING`), returning `ParseError::NestingLimit`. Lists, AND-OR tails and elif branches use iteration. Pinned by `parser::tests::deep_and_wide_inputs_are_bounded`.
- M2 grammar clarifications (POSIX behavior, not deviations):
  - POSIX 2.4 and 2.10.2 rules 1/7: only unquoted words in the appropriate grammar context are reserved; assignment and redirection prefixes do not cause a following word to be reclassified as a compound-command introducer. Quoted assignment names are ordinary words. Tests: `reserved_words_are_contextual`, `assignments_only_before_first_command_word_and_redirections_keep_order`.
  - POSIX 2.10.2 `for_clause`: both `for i do ...` and `for i; do ...` are valid, as is a newline separator. The earlier section 4.1 shorthand omitted the semicolon form; M2 deliberately corrects that production. A newline followed by another semicolon is invalid (dash accepts it, bash --posix rejects it). Tests: `for_omitted_in_uses_exactly_one_separator_or_none`, `for_newline_semicolon_uses_posix_grammar_when_shells_disagree`.
  - POSIX 2.10.2 rule 1 and its note permit a closing reserved word after a clearly delimited compound command without an intervening separator, e.g. `if (false) then (echo x) else (echo y) fi`. A simple-command argument `then` stays a word. Test: `boundaries_and_newlines`.
  - POSIX 2.10.2 rule 4: `esac` ends a case construct only at the start of an unparenthesized pattern list; after `(` or `|` it can be a pattern WORD. Empty case bodies are allowed; only the final item may omit `;;`. Tests: `tests/golden/parser/case.sh`, syntax differential fixtures.
  - Reference-shell discrepancies are explicit in `tests/diff/parser/invalid.sh` and `tests/diff.rs`: dash accepts a missing first case pattern (`|)`); bash `-n` postpones rejecting invalid/quoted for-loop names until execution. rsh requires the WORD/NAME productions specified by POSIX 2.10.2.

M2 source rendering normalizes optional pattern parentheses, final `;;`, and sequential separators (newline/semicolon) without changing the AST. Background separators remain distinct. `Display` is UTF-8 text; for non-UTF-8 AST data it is a lossy preview. `Script::to_source_bytes()` is the lossless shell-source API and is also covered by byte-input round-trip tests. Expansion/substitution bodies retain their M1 representation and are not evaluated.

M3 execution choices (SPEC 6; POSIX 2.9.1.2, 2.9.2, 2.9.3.1):
- `A=1 :` persists the assignment, but a previously unexported variable remains unexported. dash behaves this way on the reference host; bash --posix adds an export attribute. POSIX explicitly leaves acquisition/persistence of this attribute unspecified for special-builtin prefixes. rsh preserves the existing flag; `export` explicitly sets it. The difference is pinned by the header in `tests/diff/exec/special-prefix.sh` and asserted separately in `tests/exec_diff.rs`.
- Signal termination is represented as 128+N. bash may replace its own process with the last external command, causing the reference process itself to die by signal rather than return a numeric code. Differential tests normalize both forms to 128+N, and normalize shell-specific diagnostic prefixes/signal notices. `env | grep a` enumeration order is normalized by sorting just that fixture's lines, without dropping variables.
- PATH unset uses `/usr/bin:/bin`; an empty PATH component searches the current (logical shell) directory. EACCES candidates do not prevent searching later PATH entries. Missing commands return 127, found-but-unexecutable commands return 126. Executable files requiring ENOEXEC interpreter fallback currently return 126; that POSIX fallback remains a conformance gap for a later milestone (no fallback to sh/bash/dash is introduced).
- Shell variables and cwd are stored in `ShellEnv`, without mutating the host process's global environment or cwd. `cd` updates logical PWD/OLDPWD; each child changes to the stored cwd before execution. CDPATH and remaining builtin options are M7 work.
- The old CLI test's unquoted awk program is now quoted because M3 uses the actual lexer/parser; no-argument invocation is now tested as stdin mode rather than invalid usage. Existing lexer/parser goldens remain unchanged.

M4 redirection and heredoc decisions (POSIX 2.7, 2.7.4, 2.8.1; task-specific status policy):
- Error status choices are explicit in `tests/diff/redirect` headers: dash uses 2 for bad numeric fd and missing-file redirections; bash generally uses 1. rsh uses the requested ordinary=1 / special=2 distinction, and aborts on direct special-builtin errors. Tests assert the reference statuses separately instead of hiding them in diagnostic normalization.
- A nonnumeric `>&word` is rejected at execution time, before running the command, per the M4 request. dash treats it as a syntax error (2); bash --posix accepts its stdout+stderr-to-filename extension. rsh does not introduce that extension. Pinned by `invalid-dup.sh` and `redirect_differential`.
- EOF before a heredoc terminator is ParseError::Incomplete as requested, even though dash/bash accept EOF as an implicit end (bash versions may warn). Pinned by lexer/parser tests and `incomplete_document_reference_difference_is_explicit`.
- Here-document backslash-newline joining is lexical and honors backslash parity for unquoted delimiters. <<- removes tabs only, not spaces. In M4, parameter/command/arithmetic expansions in heredoc bodies remain unevaluated; raw bytes are supplied to the consumer. The unexpanded parts are retained for M5. The heredoc-in-$(...) differential check therefore compares syntax via -n, not command-substitution execution.
- AST serialization deliberately normalizes list separators to semicolon/newline layouts so pending heredoc bodies can be emitted at their proper read point. Existing lexer/parser goldens are unchanged; new heredoc .tokens/.ast fixtures assert quote/tab flags, literal data and substitution boundaries. M6 subsequently reuses the same fd-guard API for functions and all compound commands.


M5 expansion choices (SPEC 5; POSIX 2.5, 2.6.1–2.6.7, 2.7.4):
- Overflow and out-of-range shifts: explicitly use wrapping i64 (including literal accumulation and MIN / -1), shift counts modulo 64, and arithmetic right shift. Overflow/invalid shift cases are outside portable arithmetic guarantees. Local dash and bash --posix both produce MIN for MAX+1 and 1 for 1<<64; these choices are unit-tested. Arithmetic scanning caps parse nesting at 64 and operand nodes at 256; evaluation caps tree depth at 128 and recursive variable evaluation at 64. Expansion nesting is capped at 64. Limit violations are errors rather than stack exhaustion.
- `$((` is recognized as arithmetic first, consistently with the lexical precedence of POSIX 2.6.3/2.6.4 and both reference shells. To request a command substitution beginning with a subshell, write `$( (command) )` with separation. Malformed arithmetic is an expansion error; rsh does not guess a command-substitution fallback. This lexical decision is distinct from the arithmetic overflow policy.
- Arithmetic variable values that themselves contain expressions are recursively evaluated, matching bash (`x='3+4'; $((x*2))` gives 14); dash rejects that value. Cycles produce bounded errors. Comma is implemented as explicitly requested, matching bash; dash rejects `1,2`. Assignment and compound assignment persist their changes. Prefix/postfix increment/decrement and floating arithmetic are not added.
- Unquoted empty positional parameters are removed during splitting, including with non-whitespace IFS, following the task's empty-field rule and dash; bash retains some such fields. In scalar contexts `$@` joins with the first IFS byte, matching dash; bash joins with spaces. Such scalar uses of `$@` are not portable. `$-` reports only implemented option letters, while reference shells also expose their own enabled flags.
- Positional empty-quote markers follow dash at field-splitting boundaries: for IFS=space and x=' a ', `""$x` yields an empty field then a, and `$x""` yields a then an empty field. Local bash --posix drops the leading empty field. Ordinary `"$@"`, `"$*"`, quoted empty variables and common IFS fixtures agree across all shells. The disagreement cases are asserted separately instead of normalizing away fields.
- Local reference bash is macOS bash 3.2.57: it eagerly diagnoses division by zero in an unselected arithmetic conditional branch, and misparses a quoted heredoc body containing both `)` and quotes inside `$(...)`. rsh uses lazy evaluation like dash and the correctly collected heredoc body; common differential fixtures avoid those bash bugs while rsh goldens retain the punctuation case. These are reference discrepancies, not new language extensions.
- Split/join property clarification: excluding leading/trailing IFS alone does not make arbitrary strings unchanged after joining with one space (internal repeated delimiters necessarily collapse). Tests assert exact identity for canonical single-space-separated fields, and normalization idempotence for general whitespace strings.
- M3/M4 notes above describe their historical scope; M5 now replaces minimal word realization and literal unquoted heredoc handling. M6 subsequently adds control flow/functions; full builtins/options remain TODO(M7).


M6 control-flow/functions results and choices (SPEC 4.1, 6, 7; POSIX 2.9.4, 2.9.5, 2.10.2, 2.14):
- Validation includes all M4/M5 regressions, the new control golden/differential suite and generated/function AST round-trips. The stress helper runs in an isolated process, executes 100,000 iterations twice, verifies the final counter, equal fd counts, peak-RSS growth below 2 MiB between rounds and no unreaped/running children. This checks bounded resource growth rather than claiming an allocator-level proof. Direct and pipeline recursion limits and deeply nested recursive compound bodies produce diagnostics/status 2 without stack overflow.
- Resource limits: maximum active function depth is 32, preserved through internal workers/command substitutions so recursion cannot bypass it by forking. Maximum in-process AST evaluation depth is 128, covering the product of function recursion and nested compound bodies. These are deliberate implementation limits; exceeding either aborts that non-interactive execution environment with status 2. While/for iteration itself uses Rust loops, not recursive calls.
- break/continue without an enclosing loop silently return 0, following dash (POSIX leaves this case unspecified). N must be a positive decimal integer; excess N selects the outermost enclosing loop. Malformed/zero N diagnoses and aborts with status 2, matching dash. Function calls establish a loop boundary: a function's break/continue does not target the caller's non-lexically enclosing loop. POSIX leaves that case unspecified; dash uses this boundary, whereas local bash 3.2 can break a caller's loop. Modern bash may also use the boundary; the policy test accepts either documented reference behavior but pins rsh/dash.
- return outside a function/source context diagnoses and returns 1, per the task and bash; dash instead stops a top-level script with the requested return status (e.g. return 7). No dot-source support is added ahead of M7. return inside a function defaults to the previous status; explicit signed numeric values are reduced modulo 256, consistent with the existing exit numeric policy.
- shift overflow preserves the argument list, diagnoses and returns 1 without aborting, as specifically requested and as bash behaves. dash instead aborts with status 2. Zero shift succeeds; non-decimal counts and extra operands diagnose/status 1. Reference disagreement tests assert statuses/output separately, not by discarding diagnostics.
- Function-prefix assignments are scoped and restored, including attributes, following dash; shared variables without a prefix remain global. POSIX leaves aspects of function-prefix persistence unspecified; bash --posix can retain such assignments. Function definition itself neither evaluates nor applies redirections; definition redirections are expanded on each call using that call's parameters, inside call-site redirections.
- Per the task's strict brace-closing rule, a closing } requires an explicit list separator (semicolon/newline, or the POSIX background separator). rsh rejects { (true) } although POSIX token recognition/reference shells can accept a closing reserved word after an already-delimited compound command. The existing permissive closing-keyword behavior for if/loop productions is retained. This is a deliberate M6 grammar restriction, pinned by parser tests.
- New parser functions.ast expectations were inspected: f has a Brace body and unevaluated $1/$2 with an attached output redirection; g has an If body calling f. Existing lexer/parser/redirection/execution golden expectations were not regenerated or changed. Control .out expectations were written explicitly and compared with both reference shells; wc output was avoided because its padding differs between macOS/Linux.
- Deferred explicitly: tests/control.rs::pipeline_while_read is ignored with TODO(M7) until read exists. Trap mutation cannot yet be exercised because trap is also M7; subshell execution already has an independent process/state boundary for its future trap state. No other M7 builtin or option implementation is introduced.


M7 builtins/options results and choices (SPEC 3.8, 6, 7; POSIX 2.3.1, 2.5, 2.8.1, 2.9.1, 2.14 and individual utility definitions):
- Final local validation: fmt, Clippy with -D warnings and cargo test passed (121 tests; four isolated helpers are run by wrapper tests). Linux cross-target Clippy and fuzz-workspace checks passed; parser stable smoke completed 20,000 runs. Added 32 builtin golden scripts and 32 common differential scripts, plus targeted integration cases for error paths, errexit exceptions (including functions in exempt contexts), nounset, xtrace/PS4, noclobber, noexec, pipefail, source/eval/exec, readonly/listing round-trips, aliases, getopts, read, traps and wait interruption. Tests use isolated temporary directories and LC_ALL=C; no dependency was added. Existing lexer/parser goldens remain unchanged: source line metadata is excluded from semantic AST equality and debug dumps, and preserved separately across internal worker serialization.
- M7 supersedes the historical M6 shift policy: a direct special-builtin shift error now aborts non-interactive execution with status 1 as requested; dash uses 2, while some bash versions continue. The former deferred pipeline-while-read test now runs, and trap isolation is exercised. Dot-source support and CDPATH are now implemented.
- echo deliberately recognizes only its first -n and leaves backslashes literal, as explicitly requested. This is an implementation-defined POSIX echo case: local dash interprets backslashes, and macOS bash 3.2 POSIX/XSI mode may also print -n literally. The request's suggestion that literal backslashes match dash is not true on this host; tests pin rsh's requested policy and record reference variants. Use printf for portable escapes.
- Dot arguments are supported even though local dash ignores them. When arguments are supplied, rsh always restores the caller's positional parameters, including after set -- inside the source; local bash passes arguments but can leak such changes. Without arguments, changes persist. CDPATH falls back to the current directory after unsuccessful candidates, following dash; older bash can reject that case.
- getopts keeps OPTIND on a partly consumed cluster, following local bash; local dash advances it early. OPTIND=1 resets scanning, leading-colon silent errors and OPTERR=0 are supported. OPTERR is an extension. Signal names with SIG prefix are accepted as requested even where local dash/bash reject kill -s SIGUSR1.
- Error diagnostics/statuses are checked separately where references differ: rsh uses 1 for missing type names and invalid umask, 127 for unknown wait operands, and 2 for operand-less unalias. Local dash respectively uses 127, 2, 2 and 0; its type failure is printed on stdout. Common successful differential fixtures compare output and status directly; error tests retain diagnostics rather than erasing these differences.
- printf implements the requested integer/string/byte conversions, flags, width/precision and stars, repeated formats and escapes; numeric errors diagnose/status 1 while output continues. Width/precision are bounded to 1 MiB. Floating conversions and optional ulimit are not included in the requested conversion set. hash is a no-op rather than a command cache.
- Additional defensive limits: test expression nesting, alias expansion chains, dot-source nesting and command-wrapper nesting are capped at 64. read discards NUL input bytes, does not provide bash's no-operand REPLY extension, and never reads ahead. Trap handling uses a pending bitset (repeated identical pending signals may coalesce); full job control, jobs/fg/bg and %job operands remain M8. No M8 work was started.
