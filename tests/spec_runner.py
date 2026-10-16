#!/usr/bin/env python3
"""M10 test infrastructure. Python 3.9+, standard library only."""
import argparse
import concurrent.futures
import dataclasses
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / 'tests/spec'
KNOWN = ROOT / 'KNOWN_FAILURES'

@dataclasses.dataclass
class Case:
    id: str
    script: bytes
    stdout: bytes = b''
    status: int = 0
    stderr: str = None
    tags: tuple = ()
    alternatives: dict = dataclasses.field(default_factory=dict)
    skips: dict = dataclasses.field(default_factory=dict)


def parse_file(path, root=SPEC):
    """Strict metadata avoids silently swallowing a malformed expectation."""
    cases = []
    case = None
    script = []
    stdout = []
    state = 'script'
    seen_stdout = False
    seen_status = False
    def finish():
        if case is None:
            return
        if not seen_stdout or not seen_status:
            raise ValueError(f'{case.id}: stdout and status must be explicit')
        case.script = ''.join(script).encode('utf-8')
        case.stdout = ''.join(stdout).encode('utf-8')
        cases.append(case)
    for number, line in enumerate(path.read_text(encoding='utf-8').splitlines(True), 1):
        if line.startswith('#### '):
            finish()
            name = line[5:].strip()
            if not name:
                raise ValueError(f'{path}:{number}: empty name')
            case = Case(f'{path.relative_to(root).as_posix()}::{name}', b'')
            script, stdout = [], []
            state, seen_stdout, seen_status = 'script', False, False
        elif case is None:
            if line.strip() and not line.startswith('#'):
                raise ValueError(f'{path}:{number}: text outside a case')
        elif line.startswith('## '):
            meta = line[3:].rstrip('\r\n')
            if meta.startswith('tags: '):
                case.tags = tuple(x.strip() for x in meta[6:].split(','))
            elif meta == 'stdout: |':
                if seen_stdout:
                    raise ValueError(f'{case.id}: duplicate stdout')
                state, seen_stdout = 'stdout', True
            elif meta.startswith('stdout: '):
                state, seen_stdout = 'meta', True
                stdout.append(meta[8:] + '\n')
            elif meta.startswith('status: '):
                if seen_status:
                    raise ValueError(f'{case.id}: duplicate status')
                case.status = int(meta[8:])
                if not 0 <= case.status <= 255:
                    raise ValueError(f'{case.id}: invalid status')
                state, seen_status = 'meta', True
            elif meta.startswith('stderr-match: '):
                case.stderr = meta[14:]
                try:
                    re.compile(case.stderr)
                except re.error as e:
                    raise ValueError(f'{case.id}: invalid regex: {e}') from e
            elif meta.startswith('OK '):
                shell, value = meta[3:].split(': ', 1)
                alt = json.loads(value)
                if shell not in ('rsh', 'dash', 'bash') or not isinstance(alt, dict) or not alt:
                    raise ValueError(f'{case.id}: malformed alternative')
                if set(alt) - {'stdout', 'status', 'stderr-match'}:
                    raise ValueError(f'{case.id}: unknown alternative field')
                case.alternatives[shell] = alt
            elif meta.startswith('N-I '):
                shell, note = meta[4:].split(': ', 1)
                if shell not in ('dash', 'bash') or not note.strip():
                    raise ValueError(f'{case.id}: invalid N-I')
                case.skips[shell] = note
            else:
                raise ValueError(f'{path}:{number}: unknown metadata: {meta}')
        elif state == 'script':
            script.append(line)
        elif state == 'stdout':
            stdout.append(line)
        elif line.strip() and not line.startswith('#'):
            raise ValueError(f'{path}:{number}: text after metadata')
    finish()
    return cases


def load_cases(pattern=None, root=SPEC):
    cases = [c for p in sorted(root.rglob('*.test.sh')) for c in parse_file(p, root)]
    ids = [c.id for c in cases]
    if len(ids) != len(set(ids)):
        raise ValueError('duplicate case identifiers')
    if pattern:
        import fnmatch
        cases = [c for c in cases if pattern in c.id or fnmatch.fnmatchcase(c.id, pattern)]
    if not cases:
        raise ValueError('no cases selected')
    return cases


def load_known(path=KNOWN):
    result = {}
    if not path.exists():
        return result
    for number, line in enumerate(path.read_text().splitlines(), 1):
        if not line.strip() or line.startswith('#'):
            continue
        parts = [p.strip() for p in line.split('|')]
        if len(parts) != 3 or not all(parts) or parts[0] in result:
            raise ValueError(f'{path}:{number}: malformed or duplicate known failure')
        if parts[2] not in ('extension', 'TODO') and not re.fullmatch(r'SPEC\.md#[-a-zA-Z0-9]+', parts[2]):
            raise ValueError(f'{path}:{number}: missing SPEC-12 link, extension or TODO')
        result[parts[0]] = (parts[1], parts[2])
    return result


def normalize_stderr(data, temp):
    text = data.decode('utf-8', 'backslashreplace')
    for path in sorted({str(temp), os.path.realpath(temp)}, key=len, reverse=True):
        text = text.replace(path, '$TMP')
    # Only diagnostic prefixes, never arbitrary occurrences in message content.
    return re.sub(r'(?m)(?<![\w./])(?:[^\s:]*[/])?(?:rsh|dash|bash|sh):', 'SHELL:', text)


def environment(directory):
    home = directory / 'home'
    home.mkdir(exist_ok=True)
    return {'LC_ALL': 'C', 'LANG': 'C', 'PATH': '/usr/bin:/bin',
            'HOME': str(home), 'PWD': str(directory), 'TMPDIR': str(directory), 'TZ': 'UTC'}


def bin_dir():
    return Path(os.environ.get('RSH_SPEC_BIN_DIR', ROOT / 'target/debug'))


def shell_binary(shell):
    if shell == 'rsh':
        path = bin_dir() / 'rsh'
    else:
        value = shutil.which(shell)
        if not value:
            raise ValueError(f'required reference shell unavailable: {shell}')
        path = Path(value).resolve()
    if not path.is_file():
        raise ValueError(f'missing executable: {path}; run cargo build --bins')
    return str(path)


def cleanup_session(session):
    """Background jobs may change pgid; clean the owned session, not just pgid.

    All targets must still report this case's fresh session ID. Never target
    pid/group 0, the runner's own session, or unrelated users' processes.
    """
    if session <= 1 or session == os.getsid(0):
        raise ValueError('unsafe cleanup session identifier')
    for _ in range(4):
        listing = subprocess.run(['/bin/ps', '-axo', 'pid='], stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, timeout=2, check=True).stdout
        owned = []
        for raw in listing.split():
            pid = int(raw)
            try:
                if pid > 1 and os.getsid(pid) == session:
                    owned.append(pid)
            except (ProcessLookupError, PermissionError):
                continue
        if not owned:
            return
        for pid in owned:
            try:
                if os.getsid(pid) == session:
                    os.kill(pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
        time.sleep(0.005)

def execute(case, shell, timeout=5, collect_files=False):
    if shell in case.skips:
        return {'id': case.id, 'shell': shell, 'outcome': 'SKIP', 'note': case.skips[shell]}
    with tempfile.TemporaryDirectory(prefix='rsh-spec-') as name:
        directory = Path(name)
        script = directory / 'case.sh'
        script.write_bytes(case.script)
        command = [str(bin_dir() / 'spec-sandbox'), str(timeout), shell_binary(shell)]
        if shell == 'bash':
            command.append('--posix')
        command.append('case.sh')
        start = time.monotonic()
        # Seekable capture files prevent unlimited pipe buffering/memory growth.
        with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
            child = subprocess.Popen(command, cwd=directory, env=environment(directory),
                                     stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                     start_new_session=True)
            timed_out = False
            try:
                child.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
            finally:
                # Also clean up background descendants after a successful shell exit.
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait()
                cleanup_session(child.pid)
            out.seek(0)
            err.seek(0)
            stdout = out.read(32 * 1024 * 1024 + 1)
            stderr = normalize_stderr(err.read(32 * 1024 * 1024 + 1), directory)
        status = child.returncode
        if status < 0:
            status = 128 - status
        actual = {'stdout': stdout.decode('utf-8', 'backslashreplace'), 'status': status, 'stderr': stderr}
        alternatives = [{'stdout': case.stdout.decode('utf-8'), 'status': case.status, 'stderr-match': case.stderr}]
        if shell in case.alternatives:
            alternatives.append(dict(alternatives[0], **case.alternatives[shell]))
        matches = any(stdout == e['stdout'].encode('utf-8') and status == e['status'] and
                      (e.get('stderr-match') is None or re.search(e['stderr-match'], stderr)) for e in alternatives)
        outcome = 'TIMEOUT' if timed_out else ('PASS' if matches else 'FAIL')
        result = {'id': case.id, 'shell': shell, 'outcome': outcome, 'actual': actual,
                  'expected': alternatives, 'seconds': time.monotonic() - start}
        if collect_files:
            result['files'] = {p.relative_to(directory).as_posix(): p.read_bytes().hex()
                               for p in sorted(directory.rglob('*')) if p.is_file() and p != script and not p.is_symlink()}
        return result


def evaluate_policy(results, known, selected, references=False):
    errors = []
    for r in results:
        if references:
            if r['outcome'] not in ('PASS', 'SKIP'):
                errors.append(f"INVALID REFERENCE CASE {r['shell']} {r['id']}")
        elif r['outcome'] == 'PASS' and r['id'] in known:
            errors.append(f"STALE KNOWN FAILURE {r['id']}")
        elif r['outcome'] not in ('PASS', 'SKIP') and r['id'] not in known:
            errors.append(f"UNEXPECTED FAILURE {r['id']}")
    if not references:
        errors += [f'UNKNOWN KNOWN_FAILURES ID {k}' for k in known if k not in selected]
    return errors


def write_reports(results, errors, mode, target):
    target.mkdir(parents=True, exist_ok=True)
    areas = {}
    suite = ET.Element('testsuite', name='rsh-spec-' + mode, tests=str(len(results)),
                       failures=str(sum(r['outcome'] not in ('PASS', 'SKIP') for r in results)))
    for r in results:
        area = r['id'].split('/')[0]
        row = areas.setdefault(area, {'PASS': 0, 'FAIL': 0, 'SKIP': 0, 'TIMEOUT': 0})
        row[r['outcome']] += 1
        node = ET.SubElement(suite, 'testcase', classname=area + '.' + r['shell'], name=r['id'], time=str(r.get('seconds', 0)))
        if r['outcome'] == 'SKIP':
            ET.SubElement(node, 'skipped', message=r['note'])
        elif r['outcome'] != 'PASS':
            ET.SubElement(node, 'failure', message=r['outcome']).text = json.dumps(r, ensure_ascii=True)
    for message in errors:
        node = ET.SubElement(suite, 'testcase', classname='policy', name=message)
        ET.SubElement(node, 'failure', message=message)
    suite.set('tests', str(len(results) + len(errors)))
    suite.set('failures', str(int(suite.get('failures')) + len(errors)))
    ET.ElementTree(suite).write(target / 'spec-junit.xml', encoding='utf-8', xml_declaration=True)
    report = {'mode': mode, 'areas': areas, 'policy_errors': errors, 'results': results}
    (target / 'spec-report.json').write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n')
    return areas


def versions():
    lines = []
    for shell in ('dash', 'bash'):
        path = shell_binary(shell)
        digest = hashlib.sha256(Path(path).read_bytes()).hexdigest()
        lines.append(f'{shell}: {path}\nSHA-256: {digest}')
        command = [path, '--version'] if shell == 'bash' else (['/usr/bin/what', path] if sys.platform == 'darwin' else ['dpkg-query', '-W', '-f=${Version}', 'dash'])
        try:
            version = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=5, check=False).stdout.decode('utf-8', 'replace').strip()
        except (OSError, subprocess.TimeoutExpired) as e:
            version = f'version metadata unavailable: {e}'
        lines.append(version)
    return '\n'.join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', nargs='?', choices=('run', 'verify-references', 'diff'), default='run')
    parser.add_argument('--shell', choices=('rsh', 'dash', 'bash'), default='rsh')
    parser.add_argument('--filter')
    parser.add_argument('--update-known-failures', action='store_true')
    parser.add_argument('--versions', action='store_true')
    parser.add_argument('--jobs', type=int, default=min(4, os.cpu_count() or 1))
    parser.add_argument('--report-dir', type=Path, default=ROOT / 'target')
    args = parser.parse_args(argv)
    try:
        if args.versions:
            print(versions())
            return 0
        if args.jobs < 1 or args.jobs > 64:
            raise ValueError('--jobs must be between 1 and 64')
        if args.update_known_failures and (args.mode == 'verify-references' or args.shell != 'rsh' or args.filter):
            raise ValueError('update requires unfiltered rsh run/diff mode')
        all_cases = load_cases()
        cases = load_cases(args.filter) if args.filter else all_cases
        known = load_known()
        shells = ('dash', 'bash') if args.mode == 'verify-references' else (args.shell,)
        tasks = [(c, sh) for c in cases for sh in shells]
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
            results = list(pool.map(lambda t: execute(*t), tasks))
        # map preserves task order even though execution is concurrent.
        if args.update_known_failures:
            entries = []
            for r in results:
                if r['outcome'] not in ('PASS', 'SKIP'):
                    reason, link = known.get(r['id'], ('Untriaged ' + r['outcome'].lower() + '; inspect target/spec-report.json', 'TODO'))
                    entries.append(f"{r['id']} | {reason} | {link}")
            KNOWN.write_text('# Every failure needs a reviewed reason and classification.\n' + '\n'.join(entries) + ('\n' if entries else ''))
            known = load_known()
        errors = evaluate_policy(results, known, {c.id for c in all_cases}, args.mode == 'verify-references' or args.shell != 'rsh')
        areas = write_reports(results, errors, args.mode, args.report_dir)
        for area, counts in sorted(areas.items()):
            print(f"{area:24} " + ' '.join(f'{k}={v}' for k, v in counts.items()))
        for r in results:
            if r['outcome'] not in ('PASS', 'SKIP'):
                print(f"{r['outcome']} {r['shell']} {r['id']}")
                if args.mode in ('diff', 'verify-references'):
                    expected = json.dumps(r['expected'], indent=2).splitlines(True)
                    actual = json.dumps(r['actual'], indent=2).splitlines(True)
                    print(''.join(difflib.unified_diff(expected, actual, fromfile='expected', tofile='actual')))
        for error in errors:
            print(error, file=sys.stderr)
        return int(bool(errors))
    except (OSError, ValueError, re.error, subprocess.SubprocessError, json.JSONDecodeError) as e:
        print(f'spec-runner: {e}', file=sys.stderr)
        return 2

if __name__ == '__main__':
    sys.exit(main())
