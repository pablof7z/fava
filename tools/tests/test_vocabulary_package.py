"""Causal tests for the canonical vocabulary signing package."""

from __future__ import annotations

import hashlib
import sys
import tempfile
import unittest
from pathlib import Path


TOOLS = Path(__file__).parents[1]
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))
import vocabulary_package as package


class CanonicalPackageTest(unittest.TestCase):
    def test_order_is_utf8_name_byte_order_not_caller_order(self) -> None:
        unsorted = [("beta", "B"), ("alpha", "A")]
        expected = package.payload_frame("alpha", "A") + package.payload_frame(
            "beta", "B"
        )
        self.assertEqual(package.canonical_package(unsorted), expected)
        self.assertEqual(package.canonical_package(reversed(unsorted)), expected)

    def test_framing_is_exact_length_delimited_bytes(self) -> None:
        self.assertEqual(
            package.payload_frame("a", "bc"),
            b"\x00\x00\x00\x00\x00\x00\x00\x01a"
            b"\x00\x00\x00\x00\x00\x00\x00\x02bc",
        )
        # These collide under raw concatenation; length framing distinguishes them.
        self.assertNotEqual(
            package.payload_frame("a", "bc"),
            package.payload_frame("ab", "c"),
        )

    def test_payload_drift_changes_term_and_package_hashes(self) -> None:
        before = package.manifest_for_payloads("owner", [("Term", "payload")])
        after = package.manifest_for_payloads("owner", [("Term", "payload\n")])
        self.assertNotEqual(
            before["terms"][0]["markdown_sha256"],
            after["terms"][0]["markdown_sha256"],
        )
        self.assertNotEqual(before["package_sha256"], after["package_sha256"])
        self.assertEqual(
            after["terms"][0]["markdown_sha256"],
            hashlib.sha256(b"payload\n").hexdigest(),
        )

    def test_manifest_mismatch_is_rejected_as_exact_byte_drift(self) -> None:
        expected = package.render_manifest(
            package.manifest_for_payloads("owner", [("Term", "payload")])
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            path.write_bytes(expected.replace(b'"term_count": 1', b'"term_count": 2'))
            self.assertFalse(package.manifest_matches(path, expected))
            path.write_bytes(expected)
            self.assertTrue(package.manifest_matches(path, expected))

    def test_duplicate_semantic_identity_is_refused(self) -> None:
        with self.assertRaisesRegex(package.PackageError, "repeats term name"):
            package.canonical_package([("Term", "one"), ("Term", "two")])

    def test_reviewed_repository_package_has_the_accepted_bytes(self) -> None:
        root = Path(__file__).parents[2]
        manifest = package.expected_manifest(root)
        self.assertEqual(manifest["term_count"], 22)
        self.assertEqual(manifest["package_byte_length"], 104794)
        self.assertEqual(
            manifest["package_sha256"],
            "36321b93c4ba76c6069dbd759bdd5fdbaa916c5847b7b24b9e26d504e0b133f5",
        )
        self.assertEqual(
            [term["index"] for term in manifest["terms"]], list(range(22))
        )
        self.assertTrue(
            all(
                term["frame_byte_length"]
                == 16
                + term["name_utf8_byte_length"]
                + term["markdown_utf8_byte_length"]
                for term in manifest["terms"]
            )
        )


if __name__ == "__main__":
    unittest.main()
