#!/usr/bin/env python3
"""Record a real rsh session; Python stdlib only, asciinema 3.x required.

This drives an outer PTY while asciinema owns the recorded shell's inner PTY.
Only terminal output is recorded. A cursor tracker answers terminal position
queries so Reedline paints at the same positions in the recording and preview.
No account login or upload is performed.
"""
import argparse
import errno
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import shlex
import shutil
import signal
import struct
import tempfile
import termios
import time

COLS, ROWS = 88, 22
ROOT = Path(__file__).resolve().parents[2]


class Cursor:
    """Track the cursor-only VT subset emitted by the recorder and line editor."""

    def __init__(self):
        self.row, self.col = 1, 1
        self.saved = (1, 1)
        self.state, self.sequence = "text", ""
        self.pending_wrap = False

    def feed(self, data):
        replies = []
        for byte in data:
            char = chr(byte)
            if self.state == "osc":
                if byte == 7:
                    self.state = "text"
                elif byte == 27:
                    self.state = "osc_escape"
            elif self.state == "osc_escape":
                self.state = "text" if char == "\\" else "osc"
            elif self.state == "escape":
                self.state = "text"
                if char == "[":
                    self.state, self.sequence = "csi", ""
                elif char == "]":
                    self.state = "osc"
                elif char == "7":
                    self.saved = (self.row, self.col)
                elif char == "8":
                    self.row, self.col = self.saved
                elif char in "DE":
                    self.row = min(ROWS, self.row + 1)
                    if char == "E":
                        self.col = 1
                elif char == "M":
                    self.row = max(1, self.row - 1)
            elif self.state == "csi":
                if 0x40 <= byte <= 0x7e:
                    self.state = "text"
                    reply = self.csi(self.sequence, char)
                    if reply:
                        replies.append(reply)
                else:
                    self.sequence += char
            elif byte == 27:
                self.state = "escape"
            elif byte == 13:
                self.col, self.pending_wrap = 1, False
            elif byte == 10:
                self.row = min(ROWS, self.row + 1)
                self.pending_wrap = False
            elif byte == 8:
                self.col, self.pending_wrap = max(1, self.col - 1), False
            elif byte == 9:
                self.col = min(COLS, ((self.col - 1) // 8 + 1) * 8 + 1)
            elif 32 <= byte < 127:
                if self.pending_wrap:
                    self.col, self.row = 1, min(ROWS, self.row + 1)
                self.pending_wrap = self.col == COLS
                if not self.pending_wrap:
                    self.col += 1
        return replies

    def csi(self, parameters, final):
        if parameters == "6" and final == "n":
            return f"\x1b[{self.row};{self.col}R".encode()
        if parameters.startswith(("?", ">", "=")):
            return None
        values = [int(v) if v.isdecimal() else 0 for v in parameters.split(";")]
        n = values[0] or 1
        if final in "Hf":
            self.row = min(ROWS, n)
            self.col = min(COLS, (values[1] or 1) if len(values) > 1 else 1)
        elif final in "Aa":
            if final == "A":
                self.row = max(1, self.row - n)
            else:
                self.col = min(COLS, self.col + n)
        elif final in "Be":
            self.row = min(ROWS, self.row + n)
        elif final == "C":
            self.col = min(COLS, self.col + n)
        elif final == "D":
            self.col = max(1, self.col - n)
        elif final in "EF":
            self.row = min(ROWS, self.row + n) if final == "E" else max(1, self.row - n)
            self.col = 1
        elif final in "G`":
            self.col = min(COLS, n)
        elif final == "d":
            self.row = min(ROWS, n)
        elif final == "s":
            self.saved = (self.row, self.col)
        elif final == "u":
            self.row, self.col = self.saved
        if final in "HfAaBeCDEFG`du":
            self.pending_wrap = False
        return None


class Session:
    def __init__(self, command, env, cwd):
        self.data = bytearray()
        self.cursor = Cursor()
        self.closed = False
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.chdir(cwd)
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
            os.execve(command[0], command, env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

    def pump(self, duration=0.03):
        if self.closed:
            return
        if select.select([self.fd], [], [], duration)[0]:
            try:
                data = os.read(self.fd, 65536)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                data = b""
            if not data:
                self.closed = True
                return
            self.data.extend(data)
            for reply in self.cursor.feed(data):
                os.write(self.fd, reply)

    def pause(self, duration):
        until = time.monotonic() + duration
        while time.monotonic() < until and not self.closed:
            self.pump(min(0.03, until - time.monotonic()))

    def wait(self, text, start=0, timeout=10):
        until = time.monotonic() + timeout
        while True:
            found = self.data.find(text, start)
            if found >= 0:
                return found + len(text)
            if self.closed or time.monotonic() > until:
                raise RuntimeError(f"waiting for {text!r}: {bytes(self.data[-800:])!r}")
            self.pump()

    def type(self, text):
        for byte in text.encode():
            os.write(self.fd, bytes([byte]))
            self.pause(0.022)

    def enter(self, continuation=False):
        start = len(self.data)
        os.write(self.fd, b"\r")
        if continuation:
            ready = self.wait(b"... ", start)
        else:
            boundary = self.wait(b"\x1b[?2004l", start)
            ready = self.wait(b"rsh> ", boundary)
        self.wait(b"\x1b[?25h", ready)
        self.pause(0.9)

    def line(self, text):
        self.type(text)
        self.enter()

    def finish(self):
        self.type("exit")
        os.write(self.fd, b"\r")
        until = time.monotonic() + 10
        while not self.closed and time.monotonic() < until:
            self.pump()
        if not self.closed:
            raise RuntimeError("recorder did not exit")
        _, status = os.waitpid(self.pid, 0)
        self.pid = None
        if os.waitstatus_to_exitcode(status) != 0:
            raise RuntimeError(f"recorder failed: {status}")

    def close(self):
        if self.pid is not None:
            # Only the recorder process created by this script is targeted.
            try:
                os.kill(self.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            os.waitpid(self.pid, 0)
        os.close(self.fd)


def validate(path):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    header, events = rows[0], rows[1:]
    if (header.get("version"), header.get("width"), header.get("height")) != (2, COLS, ROWS):
        raise ValueError("unexpected recording format or geometry")
    previous = 0
    for event in events:
        if len(event) != 3 or event[0] < previous or event[1] not in ("o", "x"):
            raise ValueError("recording must contain ordered output/exit events only")
        previous = event[0]
    if events[-1][1:] != ["x", "0"]:
        raise ValueError("recorded shell did not exit successfully")
    output = "".join(event[2] for event in events if event[1] == "o")
    for expected in ["Hello from rsh!\r\n", "Tab completed a local file.\r\n", "alpha\r\nbeta\r\n", "conditions work\r\n", "Hello, Rust!\r\n", "Here-strings work too.\r\n", "last status: 1\r\n"]:
        if expected not in output:
            raise ValueError(f"missing demo output: {expected}")
    for private in ["/Users/", "/home/", "rsh-demo-"]:
        if private in path.read_text():
            raise ValueError(f"unexpected private path: {private}")
    print(f"Validated {len(events)} events, {previous:.1f}s, {COLS}x{ROWS}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/rsh")
    parser.add_argument("--output", type=Path, default=Path(__file__).with_name("rsh.cast"))
    parser.add_argument("--validate", action="store_true", help="only validate the existing output")
    args = parser.parse_args()
    output = args.output.resolve()
    if args.validate:
        validate(output)
        return
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error(f"build rsh first; missing binary: {binary}")
    recorder = shutil.which("asciinema")
    if not recorder:
        parser.error("asciinema 3.x is required")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="rsh-demo-") as temporary:
        root = Path(temporary).resolve()
        home = root / "home"
        config = root / "config"
        home.mkdir()
        (config / "rsh").mkdir(parents=True)
        (config / "rsh/00-demo.rsh").write_text("PS1='rsh> '; PS2='... '\n")
        (home / "README.txt").write_text("Tab completed a local file.\n")
        (config / "rsh/motd").write_text("\x1b[1;36mrsh\x1b[0m  |  interactive tour\n\n")
        env = {
            "HOME": str(home), "XDG_CONFIG_HOME": str(config),
            "XDG_DATA_HOME": str(root / "data"), "ASCIINEMA_CONFIG_HOME": str(root / "recorder"),
            "PATH": f"{binary.parent}:/usr/bin:/bin", "SHELL": "/bin/sh",
            "USER": "demo", "LOGNAME": "demo", "LC_ALL": "C",
            "TERM": "xterm-256color", "PS1": "rsh> ", "PS2": "... ",
            "HISTFILE": str(root / "history"),
        }
        # A fixed shell name in metadata avoids recording a developer's build path.
        command = [recorder, "rec", "--quiet", "--return", "--overwrite",
                   "--output-format", "asciicast-v2", "--capture-env", "TERM",
                   "--window-size", f"{COLS}x{ROWS}", "--title", "rsh: interactive tour",
                   "--command", f"{shlex.quote(binary.name)}", str(output)]
        session = Session(command, env, home)
        try:
            ready = session.wait(b"rsh> ")
            session.wait(b"\x1b[?25h", ready)
            session.pause(0.8)
            session.line("printf 'Hello from rsh!\\n'")
            session.type("printf")
            session.pause(1.0)  # Show the dim history suggestion before accepting it.
            os.write(session.fd, b"\x1b[C")
            session.pause(0.6)
            session.enter()
            session.type("cat READ")
            session.pause(0.5)
            os.write(session.fd, b"\t")
            session.pause(0.7)
            session.enter()
            session.line("printf 'beta\\nalpha\\n' | sort")
            session.type("if [[ 7 -gt 3 ]]; then")
            session.enter(continuation=True)
            session.type("  printf 'conditions work\\n'; fi")
            session.enter()
            session.line("greet() { printf 'Hello, %s!\\n' \"$1\"; }; greet Rust")
            session.line("cat <<< 'Here-strings work too.'")
            session.line("false")
            session.pause(0.5)
            session.line("printf 'last status: %s\\n' \"$?\"")
            session.pause(1.3)
            session.finish()
        finally:
            session.close()
    validate(output)


if __name__ == "__main__":
    main()
