import pathlib
import subprocess
import sys
import tempfile
import tomllib
import unittest

SCRIPT = pathlib.Path(__file__).with_name('set-version.py')
ROOT = pathlib.Path(__file__).resolve().parents[2]


class VersionTests(unittest.TestCase):
    def stage(self, version):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        root = pathlib.Path(directory.name)
        for name in ['Cargo.toml', 'Cargo.lock']:
            (root / name).write_bytes((ROOT / name).read_bytes())
        before = (root / 'Cargo.lock').read_text()
        result = subprocess.run([sys.executable, str(SCRIPT), version], cwd=root, capture_output=True)
        return root, before, result

    def test_updates_only_root_package_and_keeps_dependencies(self):
        root, before, result = self.stage('5.1.0-rc.1')
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        manifest = tomllib.loads((root / 'Cargo.toml').read_text())
        self.assertEqual(manifest['package']['version'], '5.1.0-rc.1')
        old = tomllib.loads(before)['package']
        new = tomllib.loads((root / 'Cargo.lock').read_text())['package']
        self.assertEqual([p for p in old if p['name'] != 'decay'], [p for p in new if p['name'] != 'decay'])
        self.assertEqual(next(p['version'] for p in new if p['name'] == 'decay'), '5.1.0-rc.1')

    def test_invalid_versions_leave_files_unchanged(self):
        for version in ['v5.0.0', '01.0.0', '5.0', '5.0.0-01', '5.0.0+build.2', '5.0.0-' + 'a' * 123, '5.0.0\n', '$(touch unwanted)']:
            with self.subTest(version=version):
                root, before, result = self.stage(version)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual((root / 'Cargo.lock').read_text(), before)
                self.assertEqual((root / 'Cargo.toml').read_bytes(), (ROOT / 'Cargo.toml').read_bytes())


if __name__ == '__main__':
    unittest.main()
