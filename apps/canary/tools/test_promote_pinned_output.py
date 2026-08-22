#!/usr/bin/env python3
"""Atomic-promotion and exact-rollback tests."""

import importlib.util
import pathlib
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


if __name__ == "__main__":
    unittest.main()
