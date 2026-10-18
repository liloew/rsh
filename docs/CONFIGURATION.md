# Configuration and interactive use

## Startup lifecycle

1. Parse command-line options and initialize shell variables.
2. Load rsh configuration once for the top-level invocation.
3. On the terminal REPL path only, invoke `motd` through normal command dispatch.
4. Report job notifications, display the prompt, read/parse input and execute it.

Internal workers restore a byte-preserving shell snapshot instead of repeating
startup. `--dump-ast` bypasses configuration; `-n` does not execute configuration
commands. Non-TTY stdin selects script reading, even with `-i`.

The configuration root is `$XDG_CONFIG_HOME/rsh` when XDG_CONFIG_HOME is nonempty,
otherwise `$HOME/.config/rsh`. A missing directory is silently ignored. Discovery
includes hidden directories/files and any regular file whose basename ends in
`rsh`, not just `.rsh`. For example:

```text
~/.config/rsh/
├── 00-environment.rsh
├── 10-aliases.rsh
├── conf.d/
│   └── 20-functions.rsh
└── motd
```

The list is collected before execution and sorted by full-path bytes, not directory
traversal order. Later `cd` commands do not change which files are loaded. Directory
symlinks are not followed; regular-file symlinks are accepted, and the root may be a
symlink. Config files need not be executable. Discovery or execution failures warn
with file context and do not prevent attempts to load later files.

Configuration runs in the shell itself, so variables, functions, cwd and positional
parameters can change. It is executable code, including for `rsh -c` and script
invocations. Keep output, terminal commands and interactive aliases inside a guard:

```sh
case $- in
    *i*)
        alias ll='ls -l'
        set -o git_prompt
        ;;
esac
```

No file in the current directory is implicitly sourced. `/etc/profile`, `~/.profile`,
`~/.bash_profile`, `~/.bash_login`, `~/.bashrc`, `ENV` and `BASH_ENV` are not implicitly
loaded. The `-l` flag (or a leading `-` in argv[0]) retains login-specific behavior
such as the system motd fallback; it does not change the startup-file policy.

## Explicit source commands

`source file [args...]` is a regular builtin; `.` is a special builtin. Both execute
in the current shell, allow `return`, share variables/functions/cwd and restore
ordinary temporary redirections. A function may override `source`, but not `.`.

| Operand | Default resolution |
| --- | --- |
| `file` | Current shell directory only. |
| `./file`, `../file`, `subdir/file` | Relative to the current shell directory. |
| `/path/to/file` | Absolute path. |

There is no filename-extension or configuration-directory restriction. Expansion
of `~` or `$HOME` happens through ordinary word expansion before lookup. Missing
files return 1; a direct dot error has the special-builtin shell-exit rules.
`shopt -s sourcepath` explicitly enables PATH-first lookup for slashless names,
with current-directory fallback. The default differs from POSIX dot and Bash.

Supplied arguments temporarily replace positional parameters and are restored on
return, including after an explicit `set --` in the sourced file. `$0` is unchanged.
Without supplied arguments, positional-parameter changes persist. This restoration
rule intentionally differs from some Bash versions; see SPEC section 12.

## Prompts and editing

| Setting | Behavior |
| --- | --- |
| `PS1` | Primary prompt with parameter, command and arithmetic expansion. |
| `PS2` | Continuation prompt for incomplete syntax. |
| Unset `PS1` | Default cwd prompt, `~` for HOME, `$ ` or root `# `; failures are red. |
| `PS4` | Trace prefix for `set -x`, default `+ `. |
| `set -o git_prompt` | Include Git branch information from `.git/HEAD`, without launching git. |
| `--vi` / `set -o vi` | Vi editing; `set +o vi` restores Emacs mode. |
| `TERM=dumb` or unset TERM | Plain terminal fallback, without the full editor. |

Emacs editing includes Ctrl-A/E/K/U/W/L/R/Y and Alt-B/F/D. Up/Down perform prefix
history navigation; Right/End accept the current suggestion; Alt-Right accepts a
word. Completion includes commands, variables, paths, directories after `cd`, static
Git/Cargo subcommands and job specifiers. Scans and caches are bounded; a very large
or slow filesystem can produce partial suggestions. Completion does not run git/cargo.

Multiline commands are retained as whole history entries. Bracketed paste requires
Enter before evaluation. Ctrl-C clears the input buffer; foreground jobs receive
terminal signals through their process group. `jobs`, `bg` and `fg` manage stopped
jobs. Exiting with stopped jobs first warns; a second attempt permits exit.

## History

History path precedence is nonempty `HISTFILE`, then
`$XDG_DATA_HOME/rsh/history`, then `$HOME/.local/share/rsh/history`.
An explicitly empty `HISTFILE` disables persistence. `HISTSIZE` defaults to 1000
entries; `HISTFILESIZE` defaults to 2000; each limit is capped at 100,000.

Records include timestamps, cwd and hex-encoded multiline command text; this is not
Bash's history-file format. Consecutive duplicate commands are deduplicated. Empty
commands, commands beginning with a space, and commands exceeding 1 MiB are omitted.
A separate lock file serializes merge/replace writes across instances; new files are
created with mode 0600. `history` lists loaded/session entries; `history -c` clears
both memory and the configured file.

## Message of the day

The regular `motd` builtin first reads the user configuration root's `motd` file.
For a login shell only, an absent user file permits fallback to `/etc/motd`.
No matching file produces no output and status 0. Read/type errors return 1.

On an interactive terminal startup, `motd` runs once after configuration and before
the first prompt. The usual dispatch order applies, so a function can customize it:

```sh
motd() { printf 'Welcome, %s\n' "$USER"; }
# Or silence startup output:
# motd() { :; }
```

Automatic invocation is suppressed by `--no-motd`, nonempty `RSH_NO_MOTD`, or an
existing `~/.hushlogin` entry. Manual invocation still works. `-c`, scripts,
non-TTY stdin, subshells and command substitutions never automatically invoke it.
The startup hook ignores the function's status, keeps stderr visible, and allows
Ctrl-C to return to the first prompt.

The builtin reads at most 64 KiB from a validated regular-file descriptor and appends
a newline if truncated. It replaces invalid UTF-8 and strips terminal control
sequences except SGR colors on terminal stdout; redirected/piped output strips SGR
as well. FIFO/device/directory inputs are rejected without blocking. A user-defined
motd function uses ordinary output semantics and is not passed through this sanitizer.

See [SPEC sections 9.1 and 9.2](../SPEC.md) for detailed contracts and tests.
