#!/usr/bin/env python3
"""Causal post-sample substitution tests for committed build inputs."""

import hashlib
import os
import pathlib
import subprocess
import sys
import tarfile
import tempfile
import unittest


HELPER = pathlib.Path(__file__).with_name("run-pinned-input-command.py")


class PinnedInputCommandTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.repository = pathlib.Path(self.temporary.name)
        subprocess.run(["git", "init", "-q", str(self.repository)], check=True)
        subprocess.run(
            ["git", "-C", str(self.repository), "config", "user.email", "canary@example.invalid"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(self.repository), "config", "user.name", "Fava Canary"],
            check=True,
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def assert_committed_input_wins(self, path: str, committed: bytes, hostile: bytes) -> None:
        candidate = self.repository / path
        candidate.parent.mkdir(parents=True, exist_ok=True)
        candidate.write_bytes(committed)
        subprocess.run(["git", "-C", str(self.repository), "add", path], check=True)
        subprocess.run(["git", "-C", str(self.repository), "commit", "-qm", path], check=True)
        revision = subprocess.run(
            ["git", "-C", str(self.repository), "rev-parse", "HEAD"],
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout.strip()
        candidate.write_bytes(hostile)
        result = subprocess.run(
            [
                sys.executable,
                str(HELPER),
                "--repository",
                str(self.repository),
                "--revision",
                revision,
                "--path",
                path,
                "--expected-sha256",
                hashlib.sha256(committed).hexdigest(),
                "--maximum-input-bytes",
                "1048576",
                "--seconds",
                "10",
                "--bytes",
                "1024",
                "--",
                sys.executable,
                "-c",
                "import hashlib,sys; print(hashlib.sha256(sys.stdin.buffer.read()).hexdigest())",
            ],
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        )
        self.assertEqual(result.stdout.strip(), hashlib.sha256(committed).hexdigest())
        self.assertNotEqual(hashlib.sha256(candidate.read_bytes()).hexdigest(), result.stdout.strip())

    def test_source_dockerfile_ignores_post_sample_path_substitution(self) -> None:
        path = "apps/canary/pinned-source.Dockerfile"
        committed = b"FROM scratch\n"
        candidate = self.repository / path
        candidate.parent.mkdir(parents=True, exist_ok=True)
        candidate.write_bytes(committed)
        subprocess.run(["git", "-C", str(self.repository), "add", path], check=True)
        subprocess.run(["git", "-C", str(self.repository), "commit", "-qm", path], check=True)
        revision = subprocess.run(
            ["git", "-C", str(self.repository), "rev-parse", "HEAD"],
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout.strip()
        candidate.write_bytes(b"FROM busybox\n")
        manifest = self.repository / "source.manifest"
        manifest.write_bytes(b"manifest\n")
        result = self.repository / "context.tar"
        with result.open("wb") as output:
            subprocess.run(
                [
                    sys.executable,
                    str(HELPER),
                    "--repository",
                    str(self.repository),
                    "--revision",
                    revision,
                    "--kind",
                    "archive",
                    "--path",
                    path,
                    "--archive-prefix",
                    "source/",
                    "--dockerfile-name",
                    "Dockerfile",
                    "--expected-sha256",
                    hashlib.sha256(committed).hexdigest(),
                    "--archive-path",
                    "apps/canary",
                    "--extra-file",
                    str(manifest),
                    "--extra-name",
                    "control/source.manifest",
                    "--extra-sha256",
                    hashlib.sha256(manifest.read_bytes()).hexdigest(),
                    "--maximum-input-bytes",
                    "1048576",
                    "--seconds",
                    "10",
                    "--bytes",
                    "1048576",
                    "--",
                    "sh",
                    "-c",
                    "cat",
                ],
                check=True,
                stdout=output,
            )
        with tarfile.open(result) as archive:
            extracted = archive.extractfile(f"source/{path}")
            self.assertIsNotNone(extracted)
            assert extracted is not None
            self.assertEqual(extracted.read(), committed)
            self.assertEqual(archive.extractfile("Dockerfile").read(), committed)
            self.assertEqual(archive.extractfile("control/source.manifest").read(), b"manifest\n")
            self.assertEqual(archive.getmember(f"source/{path}").mode, 0o644)
            self.assertEqual(result.read_bytes()[257:262], b"ustar")

    def test_output_dockerfile_ignores_post_sample_path_substitution(self) -> None:
        self.assert_committed_input_wins(
            "apps/canary/pinned-output.Dockerfile", b"FROM scratch\n", b"FROM busybox\n"
        )

    def test_extractor_ignores_post_sample_path_substitution(self) -> None:
        self.assert_committed_input_wins(
            "apps/canary/tools/extract-pinned-image.py",
            b"print('committed extractor')\n",
            b"print('substituted extractor')\n",
        )


if __name__ == "__main__":
    unittest.main()
