"""Private bounds and cleanup rules for the simple-groups live harness."""

from __future__ import annotations

import os
import shutil
import stat
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


MAX_LOG_BYTES = 1_048_576
MAX_SCENARIO_BYTES = 65_536
MAX_COMMAND_BYTES = 65_536
MAX_JSONL_ROWS = 128
MAX_JSONL_LINE_BYTES = 65_536
MAX_ASSERTIONS = 24
MAX_FILTER_BYTES = 16_384
MAX_SECRET_SENTINELS = 16
MAX_SECRET_SENTINEL_BYTES = 1_024
MAX_ARTIFACT_ENTRIES = 256
MAX_ARTIFACT_FILES = 128
MAX_ARTIFACT_FILE_BYTES = 2_097_152
MAX_ARTIFACT_TOTAL_BYTES = 40 * 1_048_576
MAX_BINARY_BYTES = 64 * 1_048_576


class HarnessError(RuntimeError):
    """A bounded harness responsibility could not meet its contract."""


@dataclass(frozen=True)
class ArtifactScan:
    """The bounded, retained artifact surface examined for secret material."""

    file_count: int
    total_bytes: int


def read_bounded_text(path: Path, byte_limit: int, label: str) -> str:
    """Read one UTF-8 input only after its explicit on-disk bound is known."""

    try:
        size = path.stat().st_size
    except OSError as error:
        raise HarnessError(f"{label} was unavailable") from error
    if size > byte_limit:
        raise HarnessError(f"{label} exceeded {byte_limit} bytes")
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise HarnessError(f"{label} was not UTF-8 text") from error


def _bounded_artifact_files(root: Path) -> Iterable[tuple[Path, int]]:
    """Walk retained artifacts with explicit entry, file, and byte bounds."""

    entries_seen = 0
    files_seen = 0
    total_bytes = 0
    directories = [root]
    while directories:
        directory = directories.pop()
        try:
            with os.scandir(directory) as scanned:
                entries = sorted(scanned, key=lambda entry: entry.name)
        except OSError as error:
            raise HarnessError("retained artifact directory could not be scanned") from error
        for entry in entries:
            entries_seen += 1
            if entries_seen > MAX_ARTIFACT_ENTRIES:
                raise HarnessError(f"retained artifacts exceeded {MAX_ARTIFACT_ENTRIES} entries")
            path = Path(entry.path)
            try:
                mode = entry.stat(follow_symlinks=False).st_mode
            except OSError as error:
                raise HarnessError("retained artifact metadata could not be read") from error
            if stat.S_ISLNK(mode):
                raise HarnessError(f"retained artifact may not be a symlink: {path.relative_to(root)}")
            if stat.S_ISDIR(mode):
                directories.append(path)
                continue
            if not stat.S_ISREG(mode):
                raise HarnessError(f"retained artifact was not a regular file: {path.relative_to(root)}")
            size = entry.stat(follow_symlinks=False).st_size
            files_seen += 1
            if files_seen > MAX_ARTIFACT_FILES:
                raise HarnessError(f"retained artifacts exceeded {MAX_ARTIFACT_FILES} files")
            if size > MAX_ARTIFACT_FILE_BYTES:
                raise HarnessError(
                    f"retained artifact exceeded {MAX_ARTIFACT_FILE_BYTES} bytes: {path.relative_to(root)}"
                )
            total_bytes += size
            if total_bytes > MAX_ARTIFACT_TOTAL_BYTES:
                raise HarnessError(f"retained artifacts exceeded {MAX_ARTIFACT_TOTAL_BYTES} bytes")
            yield path, size


def scan_secret_absence(root: Path, needles: Iterable[str]) -> ArtifactScan:
    """Scan every retained regular file without unbounded reads or traversal."""

    encoded_needles = [needle.encode("utf-8") for needle in needles]
    overlap_size = max((len(needle) - 1 for needle in encoded_needles), default=0)
    file_count = 0
    total_bytes = 0
    for path, size in _bounded_artifact_files(root):
        file_count += 1
        total_bytes += size
        overlap = b""
        try:
            with path.open("rb") as source:
                while chunk := source.read(65_536):
                    examined = overlap + chunk
                    if any(needle in examined for needle in encoded_needles):
                        raise HarnessError(
                            f"secret sentinel was retained in artifact {path.relative_to(root)}"
                        )
                    overlap = examined[-overlap_size:] if overlap_size else b""
        except OSError as error:
            raise HarnessError(f"retained artifact could not be read: {path.relative_to(root)}") from error
    return ArtifactScan(file_count=file_count, total_bytes=total_bytes)


def new_scratch(live: Path) -> Path:
    """Create private, ignored transient state outside retained user artifacts."""

    scratch_parent = live / ".scratch"
    scratch_parent.mkdir(mode=0o700, exist_ok=True)
    return Path(tempfile.mkdtemp(prefix="run-", dir=scratch_parent))


def remove_scratch(scratch: Path) -> None:
    """Remove all command, relay, and TMPDIR state before artifact retention."""

    if scratch.exists():
        shutil.rmtree(scratch)
