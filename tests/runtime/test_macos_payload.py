"""Contract tests for the read-only macOS runtime payload inventory."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('macos_payload', ROOT / 'scripts/macos-payload.py')
if spec is None or spec.loader is None:
    raise ImportError('macOS payload module is unavailable')
macos_payload = importlib.util.module_from_spec(spec)
spec.loader.exec_module(macos_payload)


class MacOSPayloadTests(unittest.TestCase):
    def setUp(self):
        parent = ROOT / 'work/proofs/macos-payload/unit'
        parent.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix='payload-', dir=parent)
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.assertTrue(self.root.resolve().is_relative_to(ROOT / 'work/proofs/macos-payload/unit'))
        self.temporary.cleanup()

    def tree(self, root, entries):
        root.mkdir()
        for name, content in entries:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)

    def make_symlink(self, target, link, *, directory=False):
        try:
            link.symlink_to(target, target_is_directory=directory)
        except (OSError, NotImplementedError) as error:
            if os.name == 'nt':
                self.skipTest(f'file symlink privilege unavailable: {type(error).__name__}')
            raise

    def test_hash_is_order_independent_and_ignores_payload_root_location(self):
        first = self.root / 'first'
        second = self.root / 'second'
        self.tree(first, [('z/data.bin', b'z'), ('a/data.bin', b'a')])
        self.tree(second, [('a/data.bin', b'a'), ('z/data.bin', b'z')])

        first_result = macos_payload.measure(first)
        second_result = macos_payload.measure(second)
        self.assertEqual(first_result['tree_sha256'], second_result['tree_sha256'])
        self.assertEqual(first_result['files'], 2)
        self.assertEqual(first_result['bytes'], 2)
        self.assertRegex(first_result['tree_sha256'], r'^[0-9a-f]{64}$')

        moved = self.root / 'relocated'
        first.rename(moved)
        self.assertEqual(first_result['tree_sha256'], macos_payload.measure(moved)['tree_sha256'])

    def test_single_byte_change_changes_tree_hash(self):
        tree = self.root / 'tree'
        self.tree(tree, [('payload.bin', b'abc')])
        before = macos_payload.measure(tree)['tree_sha256']
        (tree / 'payload.bin').write_bytes(b'abd')
        self.assertNotEqual(before, macos_payload.measure(tree)['tree_sha256'])

    def test_rejects_hardlinks_even_with_a_safe_file_name(self):
        tree = self.root / 'hardlink'
        self.tree(tree, [('payload.bin', b'payload')])
        outside = self.root / 'outside.bin'
        os.link(tree / 'payload.bin', outside)
        with self.assertRaises(ValueError):
            macos_payload.measure(tree)

    @unittest.skipUnless(os.name == 'posix', 'POSIX file permissions require a native host')
    def test_rejects_group_writable_files_and_directories(self):
        tree = self.root / 'permissions'
        self.tree(tree, [('payload.bin', b'payload')])
        path = tree / 'payload.bin'
        path.chmod(0o664)
        with self.assertRaises(ValueError):
            macos_payload.measure(tree)
        path.chmod(0o644)
        tree.chmod(0o770)
        with self.assertRaises(ValueError):
            macos_payload.measure(tree)

    def test_rejects_private_file_names_and_env_variants(self):
        for name in ('auth.json', 'credentials.json', '.env', '.env.local', '.ENV.production'):
            with self.subTest(name=name):
                tree = self.root / f'private-{len(list(self.root.iterdir()))}'
                self.tree(tree, [(name, b'redacted fixture')])
                with self.assertRaises(ValueError):
                    macos_payload.measure(tree)

    def test_rejects_casefold_collisions_when_filesystem_can_represent_them(self):
        tree = self.root / 'casefold'
        self.tree(tree, [('Name.txt', b'a')])
        try:
            (tree / 'name.TXT').write_bytes(b'b')
        except FileExistsError:
            self.skipTest('filesystem is case-insensitive')
        if len(list(tree.iterdir())) != 2:
            self.skipTest('filesystem cannot represent casefold-colliding entries')
        with self.assertRaises(ValueError):
            macos_payload.measure(tree)

    def test_accepts_only_relative_in_root_file_symlinks_and_hashes_link_text(self):
        first = self.root / 'link-one'
        second = self.root / 'link-two'
        for tree in (first, second):
            tree.mkdir()
            (tree / 'payload.bin').write_bytes(b'payload')
        one = first / 'alias.bin'
        two = second / 'alias.bin'
        self.make_symlink('payload.bin', one)
        self.make_symlink('./payload.bin', two)
        if os.readlink(one) == os.readlink(two):
            self.skipTest('filesystem normalized equivalent symlink text')

        result = macos_payload.measure(first)
        self.assertEqual(result['files'], 2)
        self.assertEqual(result['bytes'], len(b'payload'))
        self.assertNotEqual(result['tree_sha256'], macos_payload.measure(second)['tree_sha256'])

    def test_rejects_absolute_broken_external_and_directory_symlinks(self):
        outside = self.root / 'outside.bin'
        outside.write_bytes(b'outside')

        absolute_tree = self.root / 'absolute'
        absolute_tree.mkdir()
        self.make_symlink(outside, absolute_tree / 'link.bin')
        with self.assertRaises(ValueError):
            macos_payload.measure(absolute_tree)

        external_tree = self.root / 'external'
        external_tree.mkdir()
        self.make_symlink('../outside.bin', external_tree / 'link.bin')
        with self.assertRaises(ValueError):
            macos_payload.measure(external_tree)

        broken_tree = self.root / 'broken'
        broken_tree.mkdir()
        self.make_symlink('missing.bin', broken_tree / 'link.bin')
        with self.assertRaises(ValueError):
            macos_payload.measure(broken_tree)

        directory_tree = self.root / 'directory-link'
        directory_tree.mkdir()
        (directory_tree / 'subdir').mkdir()
        self.make_symlink('subdir', directory_tree / 'alias', directory=True)
        with self.assertRaises(ValueError):
            macos_payload.measure(directory_tree)

        if hasattr(os, 'mkfifo'):
            special_tree = self.root / 'special-link'
            special_tree.mkdir()
            os.mkfifo(special_tree / 'pipe')
            self.make_symlink('pipe', special_tree / 'alias')
            with self.assertRaises(ValueError):
                macos_payload.measure(special_tree)

    def test_rejects_symlink_payload_root(self):
        actual = self.root / 'actual'
        actual.mkdir()
        (actual / 'file.bin').write_bytes(b'x')
        alias = self.root / 'root-link'
        self.make_symlink('actual', alias, directory=True)
        with self.assertRaises(ValueError):
            macos_payload.measure(alias)


if __name__ == '__main__':
    unittest.main()
