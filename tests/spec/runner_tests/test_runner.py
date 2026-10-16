import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

module_path = Path(__file__).resolve().parents[2] / 'spec_runner.py'
spec = importlib.util.spec_from_file_location('spec_runner', module_path)
r = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = r
spec.loader.exec_module(r)

class FormatTests(unittest.TestCase):
    def parse(self, source):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / 'a.test.sh'
            p.write_text(source)
            return r.parse_file(p, Path(d))
    def test_multiline_and_heredoc_and_metadata(self):
        c, = self.parse('#### a\n## tags: posix, quoting\ncat <<EOF\nx\nEOF\n## stdout: |\nx\n## status: 0\n## stderr-match: ^$\n## OK bash: {"status": 1}\n## N-I dash: version limitation\n')
        self.assertEqual(c.script, b'cat <<EOF\nx\nEOF\n')
        self.assertEqual(c.stdout, b'x\n')
        self.assertEqual(c.tags, ('posix', 'quoting'))
        self.assertEqual(c.alternatives['bash']['status'], 1)
    def test_no_trailing_newline_stdout(self):
        # A stdout block can finish at EOF; metadata may precede it.
        c, = self.parse('#### a\nprintf x\n## status: 0\n## stdout: |\nx')
        self.assertEqual(c.stdout, b'x')
    def test_missing_expectations_rejected(self):
        for source in ['#### a\n:\n', '#### a\n:\n## status: 0\n']:
            with self.assertRaises(ValueError): self.parse(source)
    def test_unknown_metadata_rejected(self):
        with self.assertRaises(ValueError): self.parse('#### a\n## banana: x\n')
    def test_empty_and_duplicate_names(self):
        with self.assertRaises(ValueError): self.parse('#### \n')
    def test_bad_regex_and_alternative(self):
        for line in ['## stderr-match: [\n', '## OK bash: {}\n', '## OK other: {"status": 1}\n']:
            with self.assertRaises(ValueError): self.parse('#### a\n' + line)
    def test_stderr_only_normalization(self):
        self.assertEqual(r.normalize_stderr(b'dash: /tmp/t/file: missing\nword bash stays\n', '/tmp/t'), 'SHELL: $TMP/file: missing\nword bash stays\n')
    def test_script_filename_is_not_a_shell_prefix(self):
        self.assertEqual(r.normalize_stderr(b'case.sh: 1: failed\n', '/tmp/x'), 'case.sh: 1: failed\n')
    def test_duplicate_case_ids_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / 'a.test.sh'
            p.write_text('#### a\n:\n## stdout: |\n## status: 0\n' * 2)
            with self.assertRaises(ValueError): r.load_cases(root=Path(d))
    def test_known_policy_new_and_stale(self):
        failed = {'id': 'a::x', 'outcome': 'FAIL', 'shell': 'rsh'}
        self.assertTrue(r.evaluate_policy([failed], {}, {'a::x'}))
        self.assertFalse(r.evaluate_policy([failed], {'a::x': ('reason', 'TODO')}, {'a::x'}))
        self.assertTrue(r.evaluate_policy([dict(failed, outcome='PASS')], {'a::x': ('reason', 'TODO')}, {'a::x'}))
    def test_missing_known_file_means_no_exemptions(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'KNOWN_FAILURES'
            known = r.load_known(path)
            self.assertEqual(known, {})
            self.assertFalse(path.exists())
            passed = {'id': 'a::x', 'outcome': 'PASS', 'shell': 'rsh'}
            self.assertFalse(r.evaluate_policy([passed], known, {'a::x'}))
            failed = dict(passed, outcome='FAIL')
            self.assertTrue(r.evaluate_policy([failed], known, {'a::x'}))
    def test_reference_failure_cannot_be_known(self):
        failed = {'id': 'x', 'outcome': 'FAIL', 'shell': 'dash'}
        self.assertTrue(r.evaluate_policy([failed], {'x': ('reason', 'TODO')}, {'x'}, True))
    def test_empty_reason_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / 'known'
            p.write_text('a::x | | TODO\n')
            with self.assertRaises(ValueError): r.load_known(p)
    def test_reports_escape_xml(self):
        with tempfile.TemporaryDirectory() as d:
            result = {'id': 'a/x::<&', 'outcome': 'FAIL', 'shell': 'rsh', 'actual': {'stdout': '\x00'}}
            r.write_reports([result], [], 'run', Path(d))
            self.assertEqual(r.ET.parse(Path(d) / 'spec-junit.xml').getroot().get('failures'), '1')

class ExecutionTests(unittest.TestCase):
    def test_isolated_fixed_environment_and_exact_stdout(self):
        c = r.Case('runner::isolation', b'printf "%s:%s\\n" "$LC_ALL" "${UNEXPECTED-unset}"; test -d "$HOME"\n', b'C:unset\n')
        result = r.execute(c, 'rsh')
        self.assertEqual(result['outcome'], 'PASS', result)
    def test_timeout_and_reference_skip(self):
        c = r.Case('runner::timeout', b'sleep 10\n')
        self.assertEqual(r.execute(c, 'dash', timeout=1)['outcome'], 'TIMEOUT')
        c.skips['dash'] = 'test skip'
        self.assertEqual(r.execute(c, 'dash')['outcome'], 'SKIP')

if __name__ == '__main__': unittest.main()
