#!/usr/bin/env python3
"""Atomically publish one complete pinned-canary output directory."""

import os
import pathlib
import stat
import sys
from collections.abc import Callable


EXPECTED = {
    "canary": 0o500,
    "pinned-build.json": 0o400,
    "pinned-source.manifest": 0o400,
}


def sync_directory(path: pathlib.Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def verify_staging(path: pathlib.Path) -> None:
    if path.is_symlink() or not path.is_dir() or {item.name for item in path.iterdir()} != set(EXPECTED):
        raise RuntimeError("pinned output staging inventory was not exact")
    for name, mode in EXPECTED.items():
        item = path / name
        metadata = item.stat(follow_symlinks=False)
        if not stat.S_ISREG(metadata.st_mode) or stat.S_IMODE(metadata.st_mode) != mode:
            raise RuntimeError("pinned output staging member was not canonical")
        descriptor = os.open(item, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    sync_directory(path)


def restore_empty(destination: pathlib.Path) -> None:
    if destination.exists():
        if destination.is_symlink() or not destination.is_dir():
            raise RuntimeError("refusing unsafe pinned output rollback target")
        for item in destination.iterdir():
            if item.name not in EXPECTED or item.is_symlink() or not item.is_file():
                raise RuntimeError("refusing unexpected pinned output rollback member")
            item.unlink()
        destination.rmdir()
    destination.mkdir(mode=0o700)
    sync_directory(destination.parent)


def promote(
    staging: pathlib.Path,
    destination: pathlib.Path,
    after_rename: Callable[[], None] | None = None,
) -> None:
    if staging.parent != destination.parent:
        raise RuntimeError("pinned output promotion crossed filesystems")
    if destination.is_symlink() or not destination.is_dir() or any(destination.iterdir()):
        raise RuntimeError("pinned output destination was not one empty directory")
    staging_metadata = staging.stat(follow_symlinks=False)
    staging_identity = (staging_metadata.st_dev, staging_metadata.st_ino)
    verify_staging(staging)
    destination.rmdir()
    renamed = False
    try:
        if os.path.lexists(destination):
            raise RuntimeError("pinned output destination reappeared before promotion")
        os.rename(staging, destination)
        renamed = True
        destination_metadata = destination.stat(follow_symlinks=False)
        if (destination_metadata.st_dev, destination_metadata.st_ino) != staging_identity:
            raise RuntimeError("pinned output promotion changed directory identity")
        verify_staging(destination)
        if after_rename is not None:
            after_rename()
        sync_directory(destination)
        sync_directory(destination.parent)
    except BaseException:
        if renamed or not destination.exists():
            restore_empty(destination)
        raise


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: promote-pinned-output.py STAGING EMPTY_DESTINATION")
    promote(pathlib.Path(sys.argv[1]).resolve(), pathlib.Path(sys.argv[2]).resolve())


if __name__ == "__main__":
    main()
