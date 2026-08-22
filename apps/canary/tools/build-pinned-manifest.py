#!/usr/bin/env python3
"""Create the bounded canonical compiler-input manifest from exact Git objects."""

import hashlib
import os
import re
import subprocess
import sys
import tempfile

SCOPES = ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "apps/canary", "crates")


def main() -> None:
    repository, revision, tree, destination, tree_digest_destination = sys.argv[1:]
    listing = subprocess.Popen(
        ["git", "-C", repository, "ls-tree", "-r", "-z", "--full-tree", revision, "--", *SCOPES],
        stdout=subprocess.PIPE,
    )
    assert listing.stdout is not None
    row_spool = tempfile.TemporaryFile()
    total = 0
    file_count = 0
    previous_path = None
    buffer = b""
    while True:
        chunk = listing.stdout.read(4_096)
        if not chunk:
            break
        buffer += chunk
        while b"\0" in buffer:
            record, buffer = buffer.split(b"\0", 1)
            if not record or len(record) > 1_024:
                raise SystemExit("pinned Git tree row exceeded its bound")
            file_count += 1
            if file_count > 4_096:
                raise SystemExit("pinned compiler-input inventory exceeded 4096 files")
            try:
                identity, raw_path = record.split(b"\t", 1)
                mode, kind, object_id = identity.decode("ascii").split(" ")
                path = raw_path.decode("ascii")
            except (UnicodeDecodeError, ValueError) as error:
                raise SystemExit(f"noncanonical pinned Git tree row: {error}")
            if kind != "blob" or mode not in {"100644", "100755"}:
                raise SystemExit(f"unsupported pinned compiler input mode/type: {path}")
            if not re.fullmatch(r"[0-9a-f]{40}", object_id):
                raise SystemExit(f"noncanonical pinned Git object identity: {path}")
            if len(path) > 512 or not re.fullmatch(r"[A-Za-z0-9._/+@=-]+", path):
                raise SystemExit(f"unsafe pinned compiler input path: {path!r}")
            if any(part in {"", ".", ".."} for part in path.split("/")):
                raise SystemExit(f"noncanonical pinned compiler input path: {path}")
            if not (
                path in SCOPES[:3]
                or path.startswith(".cargo/")
                or path.startswith("apps/canary/")
                or path.startswith("crates/")
            ):
                raise SystemExit(f"out-of-scope pinned compiler input: {path}")
            encoded_path = path.encode("ascii")
            if previous_path is not None and encoded_path <= previous_path:
                raise SystemExit("pinned compiler-input paths were not strictly ordered")
            previous_path = encoded_path
            size_output = subprocess.run(
                ["git", "-C", repository, "cat-file", "-s", object_id],
                check=True,
                stdout=subprocess.PIPE,
            ).stdout
            if len(size_output) > 32 or not re.fullmatch(rb"[0-9]+\n", size_output):
                raise SystemExit(f"noncanonical pinned compiler input size: {path}")
            size = int(size_output)
            if size > 8_388_608:
                raise SystemExit(f"pinned compiler input exceeded 8 MiB: {path}")
            total += size
            if total > 67_108_864:
                raise SystemExit("pinned compiler inputs exceeded 64 MiB")
            blob = subprocess.Popen(
                ["git", "-C", repository, "cat-file", "blob", object_id],
                stdout=subprocess.PIPE,
            )
            assert blob.stdout is not None
            digest = hashlib.sha256()
            observed_size = 0
            while True:
                content = blob.stdout.read(65_536)
                if not content:
                    break
                observed_size += len(content)
                if observed_size > size:
                    blob.kill()
                    raise SystemExit(f"pinned compiler input exceeded declared size: {path}")
                digest.update(content)
            if blob.wait() != 0 or observed_size != size:
                raise SystemExit(f"pinned compiler input blob read failed: {path}")
            row_spool.write(f"file={mode}\t{digest.hexdigest()}\t{size}\t{path}\n".encode("ascii"))
            if row_spool.tell() > 1_048_576:
                raise SystemExit("pinned source manifest rows exceeded 1 MiB")
        if len(buffer) > 1_024:
            listing.kill()
            raise SystemExit("unterminated pinned Git tree row exceeded its bound")
    if buffer or listing.wait() != 0:
        raise SystemExit("pinned Git tree listing was incomplete")
    if file_count == 0:
        raise SystemExit("pinned compiler-input inventory was empty")
    header = (
        "format=fava-pinned-source-v1\n"
        f"revision={revision}\n"
        f"tree={tree}\n"
        f"file_count={file_count}\n"
        f"total_bytes={total}\n"
    ).encode("ascii")
    if len(header) + row_spool.tell() > 1_048_576:
        raise SystemExit("pinned source manifest exceeded 1 MiB")
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as output:
        output.write(header)
        row_spool.seek(0)
        while chunk := row_spool.read(65_536):
            output.write(chunk)
        output.flush()
        os.fsync(output.fileno())
    tree_listing = subprocess.Popen(
        ["git", "-C", repository, "ls-tree", "-r", "--full-tree", revision, "--", *SCOPES],
        stdout=subprocess.PIPE,
    )
    assert tree_listing.stdout is not None
    tree_digest = hashlib.sha256()
    tree_listing_bytes = 0
    while True:
        chunk = tree_listing.stdout.read(65_536)
        if not chunk:
            break
        tree_listing_bytes += len(chunk)
        if tree_listing_bytes > 1_048_576:
            tree_listing.kill()
            raise SystemExit("pinned source tree listing exceeded 1 MiB")
        tree_digest.update(chunk)
    if tree_listing.wait() != 0:
        raise SystemExit("pinned source tree listing failed")
    descriptor = os.open(tree_digest_destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="ascii", newline="\n") as output:
        output.write(tree_digest.hexdigest() + "\n")
        output.flush()
        os.fsync(output.fileno())


if __name__ == "__main__":
    if len(sys.argv) != 6:
        raise SystemExit("usage: build-pinned-manifest.py REPOSITORY REVISION TREE MANIFEST TREE_DIGEST")
    main()
