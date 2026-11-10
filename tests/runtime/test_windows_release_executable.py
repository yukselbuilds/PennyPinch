"""Opt-in acceptance of the actual release executable, not a source-text assertion."""
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
import os
import struct
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = spec_from_file_location('release_staging', ROOT / 'scripts/package-windows.py')
staging = module_from_spec(spec)
spec.loader.exec_module(staging)


@unittest.skipUnless(os.name == 'nt' and os.environ.get('HEADROOM_RELEASE_EXE_CANDIDATE'),
                     'requires an explicitly selected local Windows release executable')
class WindowsReleaseExecutableTests(unittest.TestCase):
    def test_release_executable_uses_the_windows_x64_gui_subsystem(self):
        path = staging.inside(Path(os.environ['HEADROOM_RELEASE_EXE_CANDIDATE']), ROOT / 'work')
        info = staging.no_links(path)
        with path.open('rb') as stream:
            self.assertEqual(stream.read(2), b'MZ')
            stream.seek(0x3C)
            offset = struct.unpack('<I', stream.read(4))[0]
            self.assertLessEqual(offset + 94, info.st_size)
            stream.seek(offset)
            header = stream.read(94)
        self.assertEqual(header[:4], b'PE\0\0')
        self.assertEqual(struct.unpack_from('<H', header, 4)[0], 0x8664)
        self.assertEqual(struct.unpack_from('<H', header, 24)[0], 0x20B)
        self.assertEqual(struct.unpack_from('<H', header, 92)[0], 2,
                         'a desktop release must not open an extra console window')


if __name__ == '__main__':
    unittest.main()
