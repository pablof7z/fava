#!/usr/bin/env python3
"""Atomic-promotion and exact-rollback tests."""

import importlib.util
import pathlib
import subprocess
import sys
import tempfile
import unittest


PATH = pathlib.Path(__file__).with_name("promote-pinned-output.py")
SPEC = importlib.util.spec_from_file_location("promote_pinned_output", PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PromotionTests(unittest.TestCase):
    def fixture(self) -> tuple[tempfile.TemporaryDirectory, pathlib.Path, pathlib.Path]:
        temporary = tempfile.TemporaryDirectory()
        root = pathlib.Path(temporary.name)
        staging = root / ".fava-pinned-build-staging-test"
        destination = root / "output"
        staging.mkdir(mode=0o700)
        destination.mkdir(mode=0o700)
        for name, mode in MODULE.EXPECTED.items():
            item = staging / name
            item.write_bytes(name.encode())
            item.chmod(mode)
        return temporary, staging, destination

    def test_one_rename_publishes_only_the_complete_directory(self) -> None:
        temporary, staging, destination = self.fixture()
        self.addCleanup(temporary.cleanup)
        MODULE.promote(staging, destination)
        self.assertEqual({item.name for item in destination.iterdir()}, set(MODULE.EXPECTED))
        self.assertFalse(staging.exists())

    def test_post_rename_failure_rolls_back_to_empty_destination(self) -> None:
        temporary, staging, destination = self.fixture()
        self.addCleanup(temporary.cleanup)
        with self.assertRaisesRegex(RuntimeError, "controlled post-rename failure"):
            MODULE.promote(
                staging,
                destination,
                lambda: (_ for _ in ()).throw(RuntimeError("controlled post-rename failure")),
            )
        self.assertTrue(destination.is_dir())
        self.assertEqual(list(destination.iterdir()), [])

    def test_extra_staging_member_refuses_without_publishing(self) -> None:
        temporary, staging, destination = self.fixture()
        self.addCleanup(temporary.cleanup)
        (staging / "partial").write_bytes(b"unsafe")
        with self.assertRaisesRegex(RuntimeError, "inventory was not exact"):
            MODULE.promote(staging, destination)
        self.assertEqual(list(destination.iterdir()), [])

    def test_leaf_symlinks_never_redirect_publication(self) -> None:
        temporary, staging, destination = self.fixture()
        self.addCleanup(temporary.cleanup)
        victim = destination.parent / "victim"
        victim.mkdir()
        destination.rmdir()
        destination.symlink_to(victim, target_is_directory=True)
        result = subprocess.run(
            [sys.executable, str(PATH), str(staging), str(destination)],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(destination.is_symlink())
        self.assertTrue(destination.is_dir())
        self.assertEqual(list(victim.iterdir()), [])

        real_staging = staging.parent / "real-staging"
        staging.rename(real_staging)
        staging.symlink_to(real_staging, target_is_directory=True)
        with self.assertRaises(OSError):
            MODULE.promote(staging, destination)
        self.assertEqual(list(destination.iterdir()), [])

    def test_post_sample_staging_substitution_refuses_and_restores_destination(self) -> None:
        temporary, staging, destination = self.fixture()
        self.addCleanup(temporary.cleanup)
        sampled = staging.parent / "sampled-staging"

        def substitute() -> None:
            staging.rename(sampled)
            staging.symlink_to(sampled, target_is_directory=True)

        with self.assertRaisesRegex(RuntimeError, "staging changed"):
            MODULE.promote(staging, destination, before_rename=substitute)
        self.assertTrue(destination.is_dir())
        self.assertEqual(list(destination.iterdir()), [])

    def test_post_sample_destination_substitution_refuses_and_restores_exact_leaf(self) -> None:
        temporary, staging, destination = self.fixture()
        self.addCleanup(temporary.cleanup)
        victim = destination.parent / "victim"
        victim.mkdir()

        def substitute() -> None:
            destination.symlink_to(victim, target_is_directory=True)

        with self.assertRaisesRegex(RuntimeError, "destination reappeared"):
            MODULE.promote(staging, destination, before_rename=substitute)
        self.assertFalse(destination.is_symlink())
        self.assertTrue(destination.is_dir())
        self.assertEqual(list(destination.iterdir()), [])
        self.assertEqual(list(victim.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
