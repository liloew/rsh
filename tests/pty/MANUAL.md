# M9 interactive checks requiring a person

Automated coverage uses expectrl on a real PTY, answers cursor-position queries,
and checks output bytes and terminal flags. These visual checks are intentionally
unchecked until a person performs them. Run in a temporary HOME/XDG directory.

- [ ] In Terminal.app, iTerm2, and a Linux terminal, check default cwd/~ prompt,
      root `#`, red failure prompt, PS1/PS2 expansion, optional `set -o git_prompt`.
- [ ] Check highlight colors for commands/functions/aliases, unknown commands,
      quotes, `$var`, substitutions, operators, redirects, comments, unfinished input.
- [ ] Check Emacs Ctrl-A/E/K/U/W/L/R/Y, Alt-B/F/D, Ctrl-Left/Right; confirm
      prefix Up/Down and Right/End/Alt-Right history acceptance feel natural.
- [ ] Start with `--vi`, then toggle `set -o vi` / `set +o vi`; check insert and
      normal modes, Escape, movement, deletion, undo, and history search.
- [ ] Check completion menu positioning, common prefixes, escaped filenames,
      Unicode names, `$VAR`/`${VAR}`/`~` paths, git/cargo/job candidates.
- [ ] Paste a multiline if or here-document using bracketed paste; verify nothing
      executes before Enter; recall and edit the complete multiline history entry.
- [ ] Resize while editing a wrapped line, with a completion menu or reverse
      search open; check no ghost characters remain.
- [ ] Suspend/resume foreground pipelines, observe idle job notifications, run
      terminal programs that change echo, then verify editor restoration.
- [ ] Check TERM=dumb uses plain PS1/PS2 without escape sequences; check a remote
      terminal with high latency and a terminal that does not answer DSR queries.

Intentional UI choices: Emacs is the default; vi is opt-in. The interpreter uses
POSIX shell syntax rather than fish syntax. History uses timestamped, hex-encoded
records (including cwd), rather than bash's history file format. Git/cargo
completion supplies static subcommands; it does not run external tools. Scans
are bounded, so exceptionally large/slow directories can yield partial results.
