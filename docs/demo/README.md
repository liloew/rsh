# Interactive demo

[rsh.gif](rsh.gif) is the GitHub-friendly preview of [rsh.cast](rsh.cast), a real
asciinema recording of the release binary. The session is approximately 21 seconds,
with an 88-column by 22-row terminal. The GIF adds a short final-frame hold.

## What it shows

- A configured motd banner and a fixed `rsh> ` primary prompt.
- A typed command recalled through the dim history suggestion, accepted with Right.
- Tab completion of `README.txt` after `cat READ`.
- A pipeline sorting two lines.
- A multiline `if [[ ... ]]; then ...; fi` using the continuation prompt.
- A shell function, a here-string, and a failed command followed by `$?` reporting 1.

The driver types scripted keystrokes into the actual editor; terminal output is not
replaced with illustrative text. The banner, prompt and example file are fixtures,
not mandatory rsh defaults. No personal configuration is loaded.

## Replay

Install [asciinema](https://docs.asciinema.org/manual/cli/installation/), then run
from the repository root:

```sh
asciinema play docs/demo/rsh.cast
```

The checked-in format is [asciicast v2](https://docs.asciinema.org/manual/asciicast/v2/),
which is supported by asciinema 2.x/3.x and the web player. No keyboard-input events
are captured; output and the recorder's exit-status event are retained.

GitHub Markdown does not run the interactive player's JavaScript. The README embeds
an animated GIF instead of an inactive HTML player. The `.cast` file can be replayed
locally without uploading it or creating an account. See asciinema's
[embedding guide](https://docs.asciinema.org/manual/server/embedding/) for the hosted
image-link approach if you later choose to publish a recording yourself.

## Re-record

Required: Python 3.9+, asciinema **3.x**, a working PTY, and a built rsh binary.
Rendering additionally requires [agg](https://docs.asciinema.org/manual/agg/).
These are documentation tools, not additions to Cargo.toml.

For example, on macOS:

```sh
brew install asciinema agg
```

From the repository root:

```sh
cargo build --release --locked --bin rsh
python3 docs/demo/record.py
python3 docs/demo/record.py --validate
agg --theme github-dark \
    --text-font-family 'Menlo,DejaVu Sans Mono,Liberation Mono' \
    --font-size 16 --line-height 1.3 --fps-cap 12 \
    --last-frame-duration 2 docs/demo/rsh.cast docs/demo/rsh.gif
```

Use `--binary /path/to/rsh` for a nonstandard Cargo target directory, and
`--output /path/to/demo.cast` to avoid replacing the checked-in recording.
`--validate` checks the selected output's format, expected results, event ordering
and common private-path leaks without launching a shell.

The script creates a temporary HOME, configuration, history and cwd. Its PTY cursor
tracker answers position queries for Reedline; asciinema captures the shell's real
output and timing. It does not invoke login profiles, capture your normal terminal
session, authenticate to a recording service, or upload anything. Temporary fixtures
are removed on completion. Recording metadata contains the command `rsh`, a title,
terminal size/timestamp and TERM, not the local binary or workspace path.

The included files were recorded with asciinema 3.2.1 and rendered with agg 1.9.0,
using Menlo at 16 px. Fonts and platform timing can change the resulting bytes.
Review the animation and the recording's text before committing a replacement.
For a different terminal emulator or new demo commands, check the cursor tracker's
supported VT subset rather than assuming it implements a complete terminal.
