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

for_clause       = "for" NAME linebreak [ "in" { WORD } sequential_sep ] do_group ;
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

## 6. Execution semantics

- **Simple command**: perform expansions; handle assignments (persist if no command, else scoped to the command's environment); apply redirections; dispatch in order: special builtin, function, regular builtin, external (PATH lookup).
- **Exit status**: 0 success; 1-125 failure; 126 found but not executable; 127 not found; 128+N killed by signal N.
- **Pipeline**: each command in its own process (fork), connected by pipes; status is that of the last command (`set -o pipefail` as extension). `!` negates.
- **Lists**: `;` sequential, `&` background (own process group when job control active), `&&`/`||` short-circuit with equal precedence, left-associative.
- **Subshell** `( ... )`: forks. **Brace group** `{ ...; }`: current shell.
- **Functions**: own positional parameters, share variables (no `local` in POSIX; `local` is an optional extension). `return` exits the function.
- **Redirections**: processed left to right; restored after builtins/functions/compound commands. `n>&m` duplicates; `n>&-` closes. Here-doc bodies go through a pipe or temp file.
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
- `read`: `-r`, IFS splitting, multiple variables.

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
- [ ] **M1 Lexer**: all tokens and quoting in section 3 except heredocs; golden token tests.
- [ ] **M2 Parser**: full grammar in 4.1 minus heredocs and functions; golden AST tests; `Incomplete` vs `Error` distinction.
- [ ] **M3 Basic execution**: simple commands, PATH lookup, assignments, pipelines, `&&`/`||`/`;`, exit statuses 126/127, `!`.
- [ ] **M4 Redirections and heredocs**: all redir ops, fd duplication/closing, here-docs (quoted and unquoted, `<<-`).
- [ ] **M5 Expansion**: tilde, full parameter expansion, command substitution, arithmetic, field splitting, globbing, quote removal; `"$@"` semantics.
- [ ] **M6 Control flow and functions**: `if/while/until/for/case`, `break/continue`, functions, `return`, subshells, brace groups.
- [ ] **M7 Builtins and options**: everything in section 7, `set` options, `trap`, `getopts`, `eval`, `.`.
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

None yet. Record each as: *what, why, and the test that pins it*.
