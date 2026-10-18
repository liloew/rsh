# SPEC: `rsh`, a POSIX-compatible shell in Rust with a fish-style interactive layer

> `rsh` is a placeholder name. Rename freely.

## 1. Goals and non-goals

**Goals**
- A shell whose *language* follows POSIX.1-2024 Shell Command Language (`sh`), so ordinary `sh` scripts run unmodified.
- A shell whose *interactive experience* is fish-like: syntax highlighting, autosuggestions, good tab completion, sane defaults.
- Safe, readable Rust. All `unsafe` and raw syscalls are isolated in one module.
- Behavior verified by differential testing against `dash` and `bash --posix`.

**Non-goals (initially)**
- Full bash compatibility (arrays, `[[ ]]`, process substitution, `select`, coprocesses). These are optional extensions (Milestone 11); the requested [[ ]] subset is documented in section 6.2.
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
2. Operators: `&& || ;; ; & | ( ) < > << >> <& >& <> <<- >|` and newline. The explicitly authorized non-POSIX `<<<` extension is recognized as one operator (section 6.1).
3. Quoting: backslash, single quotes, double quotes. Inside double quotes `$`, `` ` ``, `\`, and `"` keep special meaning per POSIX.
4. Expansions recognized by the lexer as parts of a word: `$name`, `${...}`, `$(...)`, `` `...` ``, `$((...))`.
5. Comments: `#` at the start of a word to end of line.
6. Here-documents: after the lexer sees `<<` or `<<-` with a delimiter, the body is read after the next newline. Quoted delimiter means no expansion in the body. `<<-` strips leading tabs.
7. Reserved words are recognized only in command position: `if then elif else fi do done case esac while until for in { } ! function`(`function` is an extension, off by default).
8. Aliases are expanded in command position, before reserved-word recognition. `expand_aliases` defaults on, including script mode (POSIX 2.3.1); disabling it is an extension.

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
                 | if_clause | while_clause | until_clause | conditional_command ;
conditional_command = "[[" conditional_or "]]" ;  (* extension: section 6.2 *)
conditional_or = conditional_and { "||" conditional_and } ;
conditional_and = conditional_term { "&&" conditional_term } ;
conditional_term = "!" conditional_term | "(" conditional_or ")"
                 | WORD | UNARY_OP WORD | WORD BINARY_OP WORD ;

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
redir_op         = "<" | ">" | ">>" | ">|" | "<&" | ">&" | "<>"
                 | "<<<" ;  (* non-POSIX extension: section 6.1 *)

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

### 6.1 Here-string extension (`<<<`)

On explicit user request, support `[IO_NUMBER] <<< WORD`. It is a single,
longest-match lexer operator; its operand remains a structured word in the AST.
Execution performs tilde/parameter/command/arithmetic expansion and quote removal,
without field splitting or pathname expansion. One newline is appended, even for
an empty operand or one already ending in newline. `$@` fields are joined with
spaces (including quoted `$@`); `$*` retains its usual IFS joining. This follows
bash --posix here-string behavior, not POSIX grammar (dash does not support it).

Input defaults to fd 0; explicit fd selection and left-to-right redirection order
apply as usual. Builtins/groups/functions restore their original descriptors.
The existing here-document transport supplies a prefilled pipe for small inputs
and an immediately unlinked temporary file for larger ones, including unused
inputs. AST Display, worker serialization and nested command substitutions retain
`<<<` without ever queuing a here-document delimiter/body.

The named option `here_strings` defaults on for Fedora login-profile compatibility
as requested. `set +o here_strings` rejects execution of this redirection before
expanding its operand; `set -o here_strings` (also accepted at startup) enables it.
Workers preserve the option. Grammar recognition remains enabled with the option
off, including -n and unexecuted branches; it is an execution gate, not a claim of
a strict POSIX parser mode. Other extensions require separate explicit requests.

### 6.2 Requested Bash-style conditional expressions

`[[ expression ]]` is a compound command. Structured operands expand only at
execution time, without field splitting or pathname expansion. Supported syntax
includes string truth tests, unary/binary predicates, `!`, parentheses, `&&`
(higher precedence) and `||`, with short-circuit expansion. Empty [[ ]] is a
syntax error; EOF before closing ]] is incomplete input. Quoted pattern and
regex fragments match literally.

Unary predicates: `-a -b -c -d -e -f -g -h -k -L -N -O -G -p -r -s -S -t -u
-w -x -z -n -v -o -R` (-R is false because rsh has no nameref variables).
Binary predicates: `= == !=` (shell patterns), `< >` (C-locale byte ordering),
`-eq -ne -lt -le -gt -ge` (existing signed-64-bit arithmetic evaluator),
`-nt -ot -ef` (file comparisons), and `=~` (POSIX extended regular expressions).
Success returns 0; false/arithmetic-comparison errors return 1; invalid regex
returns 2. Expansion failures retain the existing expansion-error policy.
Redirections, workers, functions, pipelines and AST source roundtrips are supported.

The double_brackets option defaults on for this explicit request. Disabling it
via set +o double_brackets returns 2 before expansion; recognition stays enabled,
as with here_strings. Conditions have a 64-token (excluding grouping/newlines) and 64-level bound. This is a
specified subset, not full Bash compatibility: extglob, arrays/namerefs and
BASH_REMATCH capture arrays remain unsupported. Arithmetic retains rsh's existing
grammar/wrapping policy; shell patterns remain byte-based in the C locale.

## 7. Builtins

**Special builtins** (assignments persist; errors may abort a non-interactive shell):
`: . break continue eval exec exit export readonly return set shift times trap unset`

**Regular builtins:**
`cd pwd echo printf test [ read source shopt type command alias unalias getopts umask wait kill jobs fg bg disown history motd true false hash`

Specifics:
- `cd`: handle `-`, `CDPATH`, `-L/-P`; update `PWD` and `OLDPWD`.
- `echo`: no option processing beyond `-n`; recommend `printf`.
- `printf`: full POSIX format support including `%b`, `%s`, `%d`, `%x`, `%o`, `%c`, width/precision.
- `test`/`[`: full POSIX expression grammar, including file tests and string/integer comparisons.
- `read`: `-r`, IFS splitting, multiple variables; byte-at-a-time input avoids consuming subsequent commands' data.
- M7 implements the special/regular builtin set above except `jobs`, `fg`, `bg` (M8) and optional `ulimit`. `hash` is an explicitly documented no-op, including `hash -r`.
- `.` resolves filenames relative to cwd (a requested deviation from POSIX PATH lookup); supplied arguments temporarily replace positional parameters, while an invocation without arguments shares them. Sourced files may return. `eval` parses in the current environment; `exec command` replaces the process without forking, and operand-less `exec` retains redirections.
- `export`/`readonly -p`, `unset -v/-f`, variable listings and trap listings use shell quoting. Readonly writes fail; direct special-builtin usage/assignment errors abort non-interactive execution. `command` bypasses functions and `-p` changes lookup without changing the child's PATH.
- `set` supports `-e -u -x -f -n -C`, named options and `pipefail`, positional parameters, and option listings. Startup accepts these options plus `-c`, `-s`, `--`, `-i`, `-l`; see sections 8 and 9 for interactive/job-control behavior and the user-selected startup policy. Aliases expand at eligible command positions via the lexer/parser hook; M10 enables `expand_aliases` by default for POSIX script behavior.
- Signal handlers only record pending signals; actions execute between commands in shell context. EXIT actions preserve the prior status unless explicitly overridden. Caught actions reset in subshells/command substitutions; ignored signals remain ignored. A substitution consisting solely of `trap` can inspect the parent's trap list without executing its EXIT action.
- Shell defaults initialize IFS, PATH, prompts, OPTIND/OPTERR, LINENO and PPID when absent, and HOME from passwd when needed. PWD/OLDPWD track directory changes; OPTARG remains unset when no option argument exists.

### 7.1 Requested shopt interface

`shopt [-pqsu] [-o] [name ...]` is a regular builtin: -s enables, -u disables,
-q queries silently, -p prints reusable commands. With no names, list implemented
options, filtered by -s/-u when supplied. -o operates on implemented set -o
options, printing set commands with -p. A query returns 0 only if every selected
option is on. Conflicting -s/-u returns 1, invalid flags return 2; unknown or
unimplemented names diagnose and return 1 while valid named options are processed.

Implemented Bash names:
- expand_aliases: enable/disable alias expansion, including interactive shells.
- dotglob: include dotfiles in wildcard matches (but not implicit . and ..).
- nullglob: remove unmatched pathname patterns.
- failglob: abort the current parsed command unit with status 1 for unmatched
  pathname patterns; following input lines may still execute, as in Bash.
  Takes precedence over nullglob.
- nocasematch: ASCII case-insensitive conditional/case pattern matching and
  REG_ICASE regex matching. POSIX named character classes retain their meaning.
- sourcepath: opt into PATH-first source/dot lookup with cwd fallback.

sourcepath defaults off to preserve the explicitly requested cwd-only policy
(Bash defaults it on); expand_aliases retains its existing on default; other new
flags default off. Worker snapshots preserve all options. Unsupported Bash names,
including extglob/globstar/history switches, return errors instead of acting as
no-ops. Arbitrary .bashrc files are therefore not guaranteed compatible.

## 8. Job control and signals

- Job control requires interactive mode and monitor mode, with an available controlling terminal. Interactive mode is selected by `-i`, or terminal stdin without `-c`/script. Monitor defaults on interactively; `set -m`/`+m`, `set -o monitor`/`+o monitor` and `$-` expose the option. Non-interactive `-m` records the option without taking terminal control.
- Following the glibc job-control-shell example, initialization waits for foreground ownership using tcgetpgrp/SIGTTIN, creates the shell process group, takes the terminal and saves its modes. The shell ignores INT/QUIT/TSTP/TTIN/TTOU unless an explicit trap overrides the disposition. A private CLOEXEC terminal fd participates in redirection-backup relocation, including persistent exec redirections.
- A monitored pipeline is one process group led by its first PID; parent and child both setpgid. Two pipe barriers keep the leader alive and wait for every child's signal reset before the parent hands over the terminal. Post-fork execution uses only async-signal-safe calls followed by exec. Simple external pipeline workers replace themselves after expansion; compound/function/builtin workers interpret only after re-exec and keep descendants in the inherited group.
- Foreground waiting targets -pgid with WUNTRACED/WCONTINUED until every member exits or stops. Save stopped-job terminal modes, restore shell ownership/modes on success and error, and restore the job modes before fg/SIGCONT. Report signal termination except INT/PIPE. Stopped jobs remain available after wait.
- `signal-hook` records SIGCHLD in a lock-free pending flag. Clear SA_RESTART on its installed disposition so idle terminal reads can wake; no table mutation, parsing or trap execution occurs inside handlers. Background polling targets only owned PIDs using WNOHANG/WUNTRACED/WCONTINUED and cannot steal foreground statuses. Report changes before prompting; an internal CHLD wakeup without an announcement must not print another prompt.
- The table tracks job IDs, pgid, each process state, AST-derived byte command text, running/stopped/done state, terminal modes, current/previous markers and disown/HUP policy. `%n`, `%%`, `%+`, `%-`, `%prefix`, `%?substring` require a unique existing match. Missing/ambiguous specs diagnose `rsh: %x: no such job` with status 1.
- `jobs [-l|-p]`, `fg [job]`, `bg [job]`, PID/job `wait`, group-targeting `kill %job`, and extension `disown [-a] [-h]` are supported. Unknown wait PIDs return 127; no-argument wait returns 0 after waiting for all owned background jobs. Completion statuses survive notification for later wait. Disowned jobs are hidden but still reaped; -h retains visibility while suppressing exit HUP.
- Background launch announces `[N] PID` only with active job control and updates `$!` to the last pipeline PID. Without job control, preserve INT/QUIT ignore and /dev/null stdin before explicit command redirections. Subshells reset the table and monitor/interactive state; command substitutions use the existing capture path without becoming jobs or changing the parent's terminal ownership.
- The first interactive exit with stopped jobs warns `There are stopped jobs.`; a second exits. On interactive shell exit, send HUP followed by CONT to each remaining owned job unless disowned/HUP-exempt. Non-interactive exits do not apply that convention.
- Non-interactive foreground waits implement cooperative SIGINT: wait/reap all children, and terminate the shell only if it received INT and a child terminated from INT. Explicit traps run in shell context. Startup-ignored signals cannot be caught in non-interactive workers; CHLD uses a caught no-op internally to retain waitable statuses.
- M8 introduced a minimal byte-input loop to exercise terminal ownership. M9 now supplies the editor described in section 9 and retains a plain terminal fallback.

## 9. Interactive layer (fish-style UX)

Built on `reedline`.
- Prompt: configurable via `PS1`/`PS2` (POSIX) with a sensible default (cwd, git branch optional, status color on non-zero `$?`).
- Syntax highlighting using the real lexer: valid command in one color, unknown command in red, strings, variables, operators.
- Autosuggestions from history (accept with Right arrow or End).
- History: file at `~/.local/share/rsh/history`, dedup, timestamps, `Ctrl-R` search.
- Tab completion: commands (PATH, builtins, functions, aliases), file paths, variables after `$`, and pluggable per-command completion later.
- Multi-line editing and continuation prompts driven by `ParseError::Incomplete`.
- Config: every top-level invocation recursively sources regular files whose names end in `rsh` under `${XDG_CONFIG_HOME:-$HOME/.config}/rsh`, ordered by full path bytes. Login mode does not automatically load `/etc/profile`, `~/.profile` or Bash configuration files. See section 9.2.

M9 implementation details:
- Reedline supplies Emacs editing, opt-in `--vi` / `set -o vi`, reverse search,
  prefix history navigation, bracketed paste, menus and resize handling. `set -o
  git_prompt` enables a branch read from `.git/HEAD` (including worktrees); no
  external git command runs during editing. PS1/PS2 expansion uses execution-time
  parameter/command/arithmetic expansion and preserves the preceding status.
- HISTFILE overrides XDG_DATA_HOME/rsh/history, otherwise HOME/.local/share/rsh/history.
  Empty HISTFILE disables persistence. HISTSIZE defaults to 1000 and HISTFILESIZE
  to 2000 entries; configurable limits are capped at 100,000. Versioned timestamp,
  cwd and hex-encoded UTF-8 command records preserve multiline entries. A separate
  lock inode serializes merge/atomic rename; new files have mode 0600. `history`
  lists session entries, and `history -c` clears memory and disk.
- Completion exposes `interactive::completion::Registry::register` with pure
  candidate providers. Command-position detection uses lexer tokens; files,
  directories, variables, builtins/functions/aliases/PATH commands, static git/cargo
  subcommands and job specifiers are supported. Directory scans allow 2048 entries
  /20 ms, a 25 ms caller deadline, and at most two outstanding workers. PATH scans
  allow 2000 directories/20,000 entries/50 ms and are cached for two seconds,
  invalidated on PATH/cwd changes. Explicit executable paths have a bounded TTL
  cache too. Slow filesystem syscalls stay in bounded background workers.
- Job notifications use Reedline's external printer to redraw safely. Terminal
  dimensions are refreshed while idle and immediately before execution. Startup
  errors warn with filename/line and return to the caller; they do not terminate
  the interactive session. Non-TTY stdin uses the byte-based script reader;
  TERM=dumb or unset TERM uses plain PS1/PS2 without editor escape sequences.
- `tests/pty/MANUAL.md` tracks visual/real-terminal checks requiring human feedback;
  automated tests do not claim that those manual checks have been performed.


### 9.1 Message of the day (`motd`)

`motd` is a regular builtin and is included in type/command lookup, highlighting
and command completion. Startup invokes a literal `motd` command through section
6's normal dispatch order, without alias substitution: a configured shell
function takes precedence. `motd() { :; }` disables its automatic output.

The builtin reads `${XDG_CONFIG_HOME:-$HOME/.config}/rsh/motd` first. When that
path is absent, a login shell (`-l` or argv[0] starting with `-`) may read
`/etc/motd`; otherwise it succeeds silently. Empty user files take precedence.
Missing HOME suppresses the HOME-based lookup; an explicit XDG_CONFIG_HOME still
works. Relative configured paths resolve against the shell cwd. Unexpected
operands return 1. A selected but unreadable or non-regular file returns 1 with
`rsh: motd: <path>: <reason>`; it does not fall through to the system message.
Final symlinks are followed, using a nonblocking CLOEXEC/NOCTTY open and fstat
on the opened descriptor before any read. Directories, FIFO, sockets and devices
are rejected; ownership and world-write permission do not alter selection.
Read storage is capped at 64 KiB, with a one-byte stack probe to detect truncation;
truncated output gets an additional newline, including when its last byte was
already a newline. File-reported sizes never drive allocation.

Only the top-level terminal REPL automatically invokes motd, once, after startup
files and before the first prompt/editor read. Command/script modes, workers,
subshells, substitutions and non-TTY `-i` input never invoke it automatically.
`--no-motd`, nonempty RSH_NO_MOTD, or any existing HOME/.hushlogin directory entry
(including a symlink) suppress this invocation, without affecting manual calls.
No function runs in a new background job or implicit subshell: ordinary shell
function side effects persist. Startup ignores its exit/return status and restores
the prior `$?` and pending shell-exit request; stderr remains visible. Ctrl-C
aborts this startup evaluation (including builtin loops), restores the signal
disposition and permits the first prompt. Explicit INT-trap changes made by the
function are re-applied after the temporary startup handler is removed.

The byte-stream sanitizer replaces malformed UTF-8 with U+FFFD, removes C0/C1
controls except newline/tab, and discards OSC/DCS/SOS/PM/APC strings, CSI cursor/
screen operations, other escape sequences and incomplete trailing sequences.
Only ESC `[ ... m` SGR with numeric/semicolon/colon parameters survives on TTY
stdout; non-TTY stdout strips SGR too. C1 CSI/strings are recognized and removed
(or canonicalized to ESC-form SGR). OSC ends at BEL or ST; other control strings
end only at ST. User-defined functions retain ordinary output semantics; their
output is not passed through the file-content sanitizer.

System-message path overrides exist only under cfg(test), using the internal
RSH_TEST_SYSTEM_MOTD shell variable in unit tests. No production binary, including
an ordinary debug build, reads that override. This is an M9 adjunct, not M10/M11.

Validation (macOS arm64, stable debug build): sanitizer/unit tests, six builtin
integration cases and six PTY cases cover file selection, all suppression paths,
regular-file validation, bounded input, manual/noninteractive invocation, real
editor Tab completion, function overrides/failure, foreground/negated/builtin-loop
Ctrl-C cancellation, and SGR versus plain output. Two timing tests are opt-in.
Final fmt/check, all-target Clippy -D warnings and cargo test pass (179 passed,
12 ignored helper/benchmark entries); both new opt-in timing tests were also run.
The direct complete startup hook averaged 15.51 microseconds over 10,000 terminal
invocations with absent files. This isolates motd's increment from REPL setup.
A coarse 50-sample process/PTY benchmark measured -c : at 8.33 ms and interactive
startup at 212.89 ms versus 211.84 ms with --no-motd; another run's difference was
0.59 ms. These include PTY harness overhead and jitter and do not establish an
absolute interactive-versus--c startup difference below 1 ms. No unrelated editor
or test-harness performance refactor is included in this small feature.

### 9.2 User configuration and explicit source commands

Every top-level execution loads the user rsh configuration once before executing
commands: interactive/login sessions, -c, script files and terminal/nonterminal
stdin. Internal pipeline/subshell/command-substitution workers inherit the shell
snapshot and never re-run startup. Parsing-only --dump-ast bypasses startup, and
-n does not execute configuration commands. The root is XDG_CONFIG_HOME/rsh when
the variable is nonempty, otherwise HOME/.config/rsh; a missing root is silent.

The current working directory is never scanned for automatic configuration.
Discovery includes hidden files/directories and filenames ending in the exact
lowercase byte suffix `rsh` (not only `.rsh`), without requiring executable bits.
It collects absolute paths before execution and sorts by path bytes, so config
commands changing cwd cannot break later loads. Recursive traversal uses an
iterative worklist. Subdirectory symlinks are not followed to avoid cycles;
symlinks to regular files are accepted, and the root itself may be a symlink.
Non-regular entries are skipped. Discovery/read/parse/runtime failures warn with
file context and startup continues with later files. Configuration shares shell
variables, functions and positional parameters, just like explicit sourcing.
Use names such as `00-env.rsh`, `aliases.rsh`, or `conf.d/20-functions.rsh`.
`config.sh` is no longer implicitly loaded; rename it to `config.rsh` to migrate.
The motd automatic hook still runs only on the top-level terminal REPL path,
after all configuration files have been processed.

No generic Bash/sh startup files are implicitly read: this includes /etc/profile,
~/.profile, ~/.bash_profile, ~/.bash_login, ~/.bashrc, ENV and BASH_ENV, even with
-l or a leading-dash argv[0]. This is an explicit user-selected startup policy;
login mode still controls unrelated login semantics such as /etc/motd fallback.
Users may explicitly source a compatible legacy file if they choose.

`source file [args]` is a user-requested regular builtin sharing `.`'s file loader.
It participates in normal lookup, type/command -v and completion; functions may
override source. `.` remains a special builtin. By default, per explicit user policy, both
resolve slashless filenames in the current shell directory without PATH lookup.
Names containing `/` select relative or absolute paths directly, without any
restriction to the configuration directory. They execute in the
current shell, support return, preserve $0, share variables/functions/cwd, and
restore redirections after returning. With supplied args, rsh restores the
caller's positional parameters on return; without args, set/shift changes remain.
This preserves the existing rsh contract even if the sourced file executes set --
when args were supplied (local Bash instead retains that explicit modification).
Apple dash-16 also ignores supplied dot arguments, so those cases compare against
Bash; argument-free dot cases compare against both references. An empty/comment-only
file returns 0, and the first command of a file observes the caller's prior $?.
Missing filename/file errors return 1 for source without terminating the shell;
Dot retains special-builtin error semantics. No parser keyword or dependency is
introduced for source.

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
- [x] **M8 Job control**: section 8, interactive and non-interactive behavior, signal handling correctness.
  - Added the job/process state table, monitor option, terminal/group lifecycle, signal-hook CHLD flag, foreground/group waits, notifications/specifiers, jobs/fg/bg/PID-or-job wait/kill/disown and stopped-job exit/HUP/CONT handling. Re-exec workers preserve the async-signal-safe fork boundary; shell-private terminal descriptors relocate around user redirections. No M9 line editor, history, completion or profile behavior was added.
  - Validation: fmt and strict all-target Clippy pass; cargo test passes 138 tests. Six isolated helper entries are invoked by wrapper tests rather than run concurrently. Eight PTY scenarios pass 20 consecutive full-suite runs on the final code; the pipeline-stop scenario additionally passed 20 isolated repetitions. Both monitored/non-monitored 200-job status checks, PID accuracy, mixed-load no-zombie checks, dash/bash differentials and manual bash interaction comparison pass. CI repeats the PTY suite 20 times on Linux/macOS. Linux cross-target Clippy and fuzz-workspace checks pass; Linux runtime execution is left to CI.
- [x] **M9 Interactive UX**: section 9 (REPL, history, highlighting, suggestions, completion, continuation prompts).
  - Automated M9 coverage passes: nine UI unit tests, three startup/fallback integration tests, nine real-editor/plain-terminal PTY scenarios, and 173 existing M3–M8 script fixtures rerun with -i and PTY stdin. The full suite has 160 passing tests. The human visual/terminal checklist in tests/pty/MANUAL.md remains explicitly unchecked; it is not represented as automated verification.
  - [x] **Requested configuration adjunct**: source/. commands, recursive rsh
    config loading in all top-level modes, and no implicit legacy startup files.
    Validation: fmt and Clippy pass; full cargo test: 194 passed, 12 ignored.
  - [x] **M9 adjunct: motd**: regular builtin, post-config normal dispatch,
    suppression, safe bounded file reads, terminal sanitizing, and PTY coverage.
- [ ] **M10 Conformance pass**: run the ported Oils spec-test cases and differential fuzzing; fix or document every deviation.
- [ ] **M11 Optional extensions** (each behind a flag/option): arrays, `[[ ]]`, `local`, `function` keyword, `{a,b}` brace expansion, `<(...)`, `pipefail` default, `select`, `**` globstar.
  - [x] **Requested adjunct: Bash-style conditionals and shopt subset**: sections
    6.2/7.1; structured AST, short-circuit evaluation, quoted patterns/ERE, real
    option semantics, Bash differential tests and parser/lexer fuzz seeds.
  - [x] **Requested adjunct: here-strings** (`here_strings` option): lexer/AST, expansion, ordered redirection, large inputs and Bash differential coverage.

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
   Keep a `KNOWN_FAILURES` list only when there are known failures; an absent file means no exemptions. CI fails on unexpected failures or if a listed case starts passing without being removed.
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
- PATH unset uses `/usr/bin:/bin`; an empty PATH component searches the current (logical shell) directory. EACCES candidates do not prevent searching later PATH entries. Missing commands return 127, found-but-unexecutable commands return 126. M10 closes the ENOEXEC text-script fallback gap by re-executing rsh itself with the candidate script and original arguments. All fallback argv storage is prepared before fork; no sh/bash/dash delegation is introduced.
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
- return outside a function/source context diagnoses and returns 1, per the task and bash 3.2; bash 5.3 instead diagnoses and returns 2. The reference-policy test accepts either Bash error status and requires a diagnostic, while pinning rsh to 1. dash instead stops a top-level script with the requested return status (e.g. return 7). No dot-source support is added ahead of M7. return inside a function defaults to the previous status; explicit signed numeric values are reduced modulo 256, consistent with the existing exit numeric policy.
- shift overflow preserves the argument list, diagnoses and returns 1 without aborting, as specifically requested and as bash behaves. dash instead aborts with status 2. Zero shift succeeds; non-decimal counts and extra operands diagnose/status 1. Reference disagreement tests assert statuses/output separately, not by discarding diagnostics.
- Function-prefix assignments are scoped and restored, including attributes, following dash; shared variables without a prefix remain global. POSIX leaves aspects of function-prefix persistence unspecified; bash --posix can retain such assignments. Function definition itself neither evaluates nor applies redirections; definition redirections are expanded on each call using that call's parameters, inside call-site redirections.
- M10 removes M6's stricter brace-closing restriction: POSIX 2.10.2 permits { (true) } after an already delimited compound command. Simple-command endings still require a separator to recognize the closing reserved word. The parser regression now checks the accepted form and AST round-trip, matching both references.
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


M8 job-control choices and validation (SPEC 6–8; POSIX 2.11, 2.12, 2.14; glibc manual 29.5):
- Implementation reference: https://sourceware.org/glibc/manual/latest/html_node/Implementing-a-Shell.html and its initialization, launching, foreground/background and stopped-job subsections. The parent controls terminal transfer after an explicit readiness barrier; this closes the same launch races without needing a second child-side tcsetpgrp. Both sides still call setpgid.
- Non-interactive SIGINT policy follows the task's explicit cooperative-exit rule and local bash. Local dash instead exits after receiving INT while waiting even when the child ignores INT and returns successfully (the longstanding behavior described in Debian #683671). The differential test asserts that discrepancy explicitly; claiming identical dash behavior would contradict the observation. Signal deaths and 128+signal numeric statuses are normalized only for that comparison.
- Notifications use canonical AST bytes rather than preserving original whitespace/quoting. WCONTINUED marks resumed processes; repeated identical pending signals may coalesce, while waitpid events retain every child completion. jobs -l prints the group leader rather than a bash-style line for every pipeline element. No ERR/DEBUG traps or additional M9 UI is introduced.
- PTY tests compare terminal modes against their starting state (expectrl disables echo), poll foreground ownership before control characters, and run a controlled stopped helper to verify HUP/CONT rather than relying on reference-shell trap timing. The initial reference-shell HUP fixture failed on repeat 8 and was replaced. Internal CHLD interrupts exposed duplicate prompt output; prompted-state tracking now prevents that. A test cleanup bug involving terminal group 0 was corrected to signal only positive owned groups.
- Manual observation of macOS /bin/bash 3.2 POSIX interactive mode confirmed Ctrl-Z, jobs, bg, fg and Ctrl-C transitions; bash prints Stopped(SIGTSTP) and different spacing, while rsh uses the requested Stopped label. Automated comments record that comparison. All tests use temporary HOME/cwd and LC_ALL=C. expectrl is an approved development dependency; signal-hook is the preferred signal dependency from AGENTS.md.

- M8 verification: the full suite passes with 138 tests and six isolated helper entries invoked by their wrapper tests. Added one job-table unit test, eight non-PTY integration tests (including the isolated mixed-load/no-zombie probe), eight PTY tests (plus a stopped HUP helper), and three common non-interactive differential scripts. Both monitored and non-monitored 200-job cases compare every completion status; a ps scan and no-child syscall assertion check mixed-load reaping. Simple background external commands replace the worker so $! equals the executing program PID and kill $! does not merely kill a wrapper; an explicit differential PID test pins this correction.
- The repeated pipeline stop assertion now waits until ps confirms the requested sleep programs have exec'd, then inspects both process states directly from the test process. A separate command-substitution check verifies they stay stopped. This avoids measuring transient worker startup or consuming asynchronous prompt output as the command's completion. Job notifications are deduplicated by aggregate state transition, with a repeated-stop unit regression. A stopped-job cleanup test resumes before KILL so wait cannot legitimately return the previous stopped status while termination is still pending.
- Validation commands: cargo fmt --all; cargo fmt --all -- --check; cargo clippy --all-targets -- -D warnings; cargo test. Linux aarch64 cross-target Clippy and fuzz-workspace format/Clippy checks also pass. Runtime tests were executed on macOS with PTY permissions; Linux runtime coverage remains CI's responsibility. No M9 milestone item is checked.

- Final stability result: after fixing the test synchronization and state-notification issues above, the final eight-scenario PTY suite passed 20/20 consecutive runs (160 scenario executions, including 4,000 monitored short jobs). The pipeline-stop case also passed 20/20 isolated runs. No intermittent failure remained in those final runs; this is measured repeat coverage, not a proof that no race exists. CI repeats the full PTY suite 20 times and preserves failed command output.
- macOS CI later exposed a missed foreground transition in the HUP/CONT fixture:
  the helper could stop and the shell reclaim the terminal before the parent
  polled terminal ownership. The helper now waits for a file acknowledgement
  written after the parent observes the foreground handoff, then stops itself.
  The stopped-job warning and receipt of HUP after CONT remain asserted.
  The updated scenario passed 20 consecutive local macOS runs; fmt, all-target
  Clippy and a standalone full cargo test run passed. A full run overlapping
  the repeat loop had one control differential stdout failure; it passed on
  the standalone rerun, and the cause of that separate failure is unconfirmed.

M9 interactive implementation choices and validation (SPEC 9; POSIX shell
parameters PS1/PS2 and shell input/alias rules):
- The UI edits Unicode text; core scripts, words, environment values and paths
  remain byte-based. Non-UTF-8 filesystem names cannot be offered as Unicode
  completion candidates. The plain TERM=dumb fallback offers continuation and
  signal handling, without editing/search/menus/colors. Unset TERM deliberately
  chooses that same fallback.
- Unlike fish, the core language remains POSIX shell syntax. Unlike bash's default
  history navigation, Up/Down use prefix search when there is input; suggestions
  prefer the most recent matching entry in the current directory. History uses
  rsh's versioned multiline/cwd record format, not fish/bash's file formats.
  Git/cargo completion is a static subcommand provider, not external-tool probing.
  Huge or slow listings may produce partial results instead of blocking editing.
- Startup-file failures (including an explicit exit from a startup file) warn and
  return to startup processing, as required for resilient configuration loading.
  The later user-requested policy in section 9.2 replaces the original M9
  /etc/profile/HOME/.profile sequence with sorted recursive rsh-only config
  loading in all top-level modes, using XDG_CONFIG_HOME or HOME/.config. Aliases are enabled in
  the interactive parser; parsing/validation/highlighting never performs expansion.
- PTY fixtures answer one cursor-position query at a time. The first harness used
  an unanchored expectrl regex that consumed several matches and silently lost
  later queries; anchored, single-frame matching and completed-paint/submission
  boundaries fixed the resulting artificial cursor-query timeouts. Foreground
  control characters are sent after the editor releases raw mode. Panic recovery
  compares terminal flags to expectrl's initial echo-disabled, canonical state.
- The script regression adapter runs 173 existing M3–M8 golden/differential scripts
  with -i and PTY stdin, comparing output/status against ordinary execution.
  Monitor is disabled for output parity, i is normalized in option-flag output,
  times compares row structure, and both runs wait for outstanding jobs before
  exiting (an interactive exit would otherwise intentionally HUP them). It does
  not replace the separate real-editor PTY scenarios or the existing three-shell
  differential comparisons.

- Descriptor isolation: asynchronous directory scans hold a shared IO guard;
  command execution/prompt substitution takes the exclusive guard, with a 75 ms
  deadline. A hung filesystem yields a retry diagnostic and keeps the line for
  another Enter, instead of hanging or executing across a live scanner fd. Plain
  terminals do not start background PATH scans. Reedline/crossterm's opaque fds
  are initialized above fd 64, marked CLOEXEC and protected against explicit
  user redirection references. Those few implementation-reserved high fd numbers
  produce a diagnostic rather than being clobbered; ordinary low descriptors
  (including 3 and 10) work normally. This is an interactive implementation limit;
  script mode retains M4's descriptor behavior. Recovery reopens /dev/tty so
  persistent exec redirections cannot overwrite a retained recovery descriptor.
- History is copied into re-exec pipeline/subshell workers even with HISTFILE
  disabled. A child history -c clears its own copy, preserving the parent session.
  The PTY suite serializes independent terminal scenarios; its dedicated two-shell
  test still performs concurrent, locked history writes. CI repeats job-control
  and editor scenarios 20 times, and runs the larger script corpus once per suite.

- Final M9 validation on macOS: cargo fmt --all; cargo fmt --all -- --check;
  cargo clippy --all-targets -- -D warnings; cargo test all pass (160 passed,
  0 failed, 10 ignored harness entries: seven isolated helpers invoked by their
  wrapper tests and three opt-in benchmarks). The existing dash/bash --posix
  differential tests pass; no golden expectation file was changed. Linux-only
  /proc fd assertions are added and remain for Linux CI runtime verification.
- Added nine UI unit tests (highlighting/incomplete syntax, suggestions, pure
  completion/registry, prompts/git HEAD, history limits/format/configuration,
  executable-cache invalidation and bounded scanner handoff), three startup/
  non-TTY integration tests, nine PTY scenarios plus an isolated panic helper,
  and the 173-script PTY adapter. Two real shells concurrently write 60 history
  records; every record survives and decodes, and history -c clears the store.
- Stability: an eight-scenario editor suite passed three consecutive runs after
  descriptor isolation was introduced. The final complete suite additionally
  covers idle job notifications and the final initialization/UTF-8 fixes; all
  nine editor scenarios pass. Earlier cursor-query harness failures and actual
  fd collisions were fixed rather than ignored. These repetitions do not prove
  absence of races; CI retains 20 repetitions of job-control/editor scenarios.
- Opt-in reference benchmarks (unoptimized local test build): 200-character full
  highlighter averages 138 microseconds, completion over 2000 command names
  averages 904 microseconds, and a 2000-entry PATH cache hit averages 0.9
  microseconds. These are reference measurements, not timing assertions in CI.
  Visual colors, detailed vi key behavior and real-terminal compatibility still
  require the human feedback tracked in tests/pty/MANUAL.md. No M10 work was done.

M10 progress (not yet complete):
- Step 1 adds the spec-runner test launcher, strict case metadata parser,
  per-case temporary cwd/HOME, fixed C locale/environment, ordered parallel runs,
  five-second deadlines, resource-limit helper, and owned-session cleanup (also
  covering background jobs that change process groups). Stdout/status are exact;
  only stderr diagnostic shell prefixes and temporary paths are normalized.
- verify-references rejects bad reference expectations independently of
  KNOWN_FAILURES. run/diff produce JSON and JUnit reports and reject unexpected,
  stale, unknown or malformed known failures. Shell alternatives are explicit
  JSON metadata, with skips requiring explanations. See tests/REFERENCE_VERSIONS.md
  for the format, actual reference versions/build hashes, and commands.
- Step-1 validation: 15 Python infrastructure checks, both bootstrap quoting
  cases under both references and rsh, full cargo test, fmt and all-target Clippy
  pass. The initial NPROC=64 limit was inappropriate on a shared desktop UID and
  prevented valid forks; the test helper now caps it at 1024 (or the existing hard
  limit). CPU/file limits apply on both platforms; Linux additionally caps address
  space. No M10 completion checkbox is checked and no M11 work is introduced.
- Step 2 adds 745 original POSIX-derived matrix cases (747 with the bootstrap
  cases), with a checked-in deterministic authoring source. Coverage counts:
  quoting 52; reserved words/aliases 30; parameters 60; expansion 120;
  splitting/globbing 70; redirections/heredocs 70; status/errors 40; commands 80;
  syntax 30; builtins 120; options 45; non-interactive jobs/signals 30.
  No Oils code or tests were copied. All areas were reference-verified before
  using rsh results: 1,489 reference passes, five explicit dash pipefail skips.
- Reference decisions: POSIX 2.6.4/C arithmetic short-circuiting excludes an
  unreachable division by zero. Apple bash 3.2 instead reports that division;
  five arithmetic precedence cases retain the normative dash output and an
  explicit bash alternative. pipefail cases are tagged as extensions and skip
  Apple dash-16, which lacks the option. command -v failure status is only
  required to be nonzero, so unalias tests inspect success/failure rather than
  assuming bash's status 1 (dash returns 127). Pattern fixtures use a dedicated
  subdirectory so the runner's script/HOME files cannot contaminate matches.
- Step 3 fixes the four groups exposed by the initial 726/747 rsh result:
  physical backslash-newline continuations are retained across input units
  (file/source and stdin); aliases default on in scripts; ENOEXEC text scripts
  re-enter rsh with the original arguments, including exec/PATH forms; and a
  closing brace is accepted after an already delimited compound command.
  These changes follow POSIX 2.2.1, 2.3.1, 2.9.1.6 and 2.10.2 respectively.
  No AST/token type or public module boundary was changed. The reviewed M6
  negative parser assertion now tests acceptance/round-trip, and the reviewed
  M7 alias assertion tests both the POSIX default and explicit opt-out.
- Three additional reference-verified ENOEXEC regressions bring the suite to
  750 cases. rsh passes all 750, with no KNOWN_FAILURES entries. A stdin
  continuation regression also compares both reference shells, including a
  backslash at the end of a comment. These results do not satisfy the remaining
  M10 fuzz-budget, robustness, performance and fresh-clone exit criteria.
- Initial instrumented cargo-fuzz smoke runs (nightly test toolchain only):
  lexer 1,163,413 executions and parser 537,807 executions, each reporting 61
  seconds elapsed, with no crash/timeout/panic finding. This is not the required
  24 CPU-hour campaign, and is not represented as having completed that budget.
- Count correction: the step-2 commit subject mistakenly says 847; its actual
  case count was 747. The per-area counts above and machine-readable reports
  are authoritative. The final 750-case reference run has 1,495 passes and
  five dash pipefail skips (1,500 total reference executions).
- Step-3 validation: cargo fmt --all / --check, all-target Clippy -D warnings,
  and cargo test pass (162 passed; 10 existing ignored helper/benchmark entries).
  No golden output files were changed. The new 750-case spec run passes with
  no unexpected or stale failures; the complete two-reference verification also
  passes under the recorded versions. M10 remains unchecked.

- Fedora login-profile regression: file predicates given an empty pathname must
  return false rather than joining it to cwd. Affected predicates include -e,
  -d, -r, -w, -x and -s; all supported file predicates have a three-shell
  regression. This fixes colorls.sh taking its INCLUDE-file branch for an empty
  INCLUDE. Nested sourced startup files now report their own path and absolute
  physical syntax-error line; the startup-failure summary retains the innermost
  failing file's context instead of attaching its line to the parent filename.
- Fedora 44's supplied lang.sh uses the non-POSIX <<< here-string operator.
  The initial login fix deliberately rejected it; a subsequent explicit user
  request authorizes the here_strings extension in section 6.1. The original
  Fedora locale test now runs unchanged under rsh and bash --posix. The portable
  printf-to-grep alternative remains tested against all three shells.
- Validation of this login regression fix: fmt/check and all-target Clippy
  -D warnings pass; full cargo test passes (182 passed, 12 existing ignored
  helper/benchmark entries). Three new regressions cover empty file predicates
  against both references, nested startup error locations, and the portable
  locale-filter replacement. Remote Fedora deployment/retest is not claimed.

- Here-string validation: fmt/check, all-target Clippy -D warnings and full
  cargo test pass (187 passed, 12 existing ignored helper/benchmark entries).
  New coverage includes longest-match lexing, parser round-trips/incomplete input,
  an independently authored redirect Golden fixture, bash --posix differential
  execution (including the reported Fedora locale predicate), IFS-sensitive
  positional parameters, fd/order/compound/pipeline behavior, >1 MiB input and
  unused input, plus disabled-option inheritance into workers. Lexer/parser
  fuzz corpora include the new syntax. The old startup-location negative test
  now uses genuinely invalid `> >` syntax instead of the newly supported `<<<`.
  No existing Golden stdout was regenerated, no dependency or unsafe code was
  added, and remote Fedora deployment remains unverified.

- User-selected startup policy deviation: section 9.2 deliberately does not follow
  Bash's login/profile/rc loading rules. Original M9 login tests now assert that
  legacy profiles are ignored, and fixtures migrated from config.sh to config.rsh.
  Source/. differential tests preserve known positional-parameter reference
  differences explicitly instead of changing expected output to match rsh.

- Requested source lookup policy: both `source name` and `. name` read from
  the current shell directory, without searching PATH. Relative and absolute
  paths work outside the config tree. This intentionally departs from POSIX dot
  PATH lookup and Bash PATH-first lookup. Automatic startup never scans cwd.
  Locked by `tests/source.rs::source_and_dot_resolve_paths_from_cwd_without_path_search`
  and `tests/source.rs::startup_does_not_source_files_in_cwd`. Existing common
  differential tests use explicit paths to retain cross-shell comparability.

- Requested conditional/shopt compatibility: sections 6.2/7.1 define the tested
  subset and limitations. Bash 5.3.15 is the primary local reference; Bash 3.2
  lacks -v and has older regex quoting behavior. sourcepath defaults off to
  retain requested cwd-only source/dot lookup. BASH_REMATCH, extglob, arrays and
  additional shopt names remain unsupported; no option with missing semantics
  reports success. Added unsafe regex/identity calls live only in src/sys with
  SAFETY comments. No dependency was added.

- Conditional/shopt final verification: fmt, all-target Clippy -D warnings and
  full cargo test pass (203 passed, 12 ignored helpers/manual checks). Seven new
  integration groups cover Bash comparisons, quoting, precedence, file/arithmetic
  predicates, regex errors, option inheritance/listing, failglob line recovery,
  sourcepath opt-in, bounds, arbitrary bytes and AST roundtrips. Lexer and parser
  stable fuzz smoke each completed 20,000 runs without crash/hang/panic; these
  were not sanitizer/coverage-instrumented campaigns.
