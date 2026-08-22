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


def canonical_leaf(path: pathlib.Path) -> str:
    if (
        not path.is_absolute()
        or pathlib.Path(os.path.abspath(path)) != path
        or path.name in {"", ".", ".."}
        or path.parent == path
    ):
        raise RuntimeError("pinned output path was not one absolute canonical leaf")
    return path.name


def open_directory(name: str, parent: int) -> int:
    return os.open(
        name,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0),
        dir_fd=parent,
    )


def verify_directory(descriptor: int) -> tuple[int, int]:
    metadata = os.fstat(descriptor)
    if not stat.S_ISDIR(metadata.st_mode) or set(os.listdir(descriptor)) != set(EXPECTED):
        raise RuntimeError("pinned output staging inventory was not exact")
    for name, mode in EXPECTED.items():
        item = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
        if not stat.S_ISREG(item.st_mode) or stat.S_IMODE(item.st_mode) != mode:
            raise RuntimeError("pinned output staging member was not canonical")
        opened = os.open(
            name,
            os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=descriptor,
        )
        try:
            os.fsync(opened)
        finally:
            os.close(opened)
    os.fsync(descriptor)
    return metadata.st_dev, metadata.st_ino


def require_empty_directory(name: str, parent: int) -> None:
    descriptor = open_directory(name, parent)
    try:
        if os.listdir(descriptor):
            raise RuntimeError("pinned output destination was not one empty directory")
    finally:
        os.close(descriptor)


def restore_empty(name: str, parent: int) -> None:
    try:
        metadata = os.stat(name, dir_fd=parent, follow_symlinks=False)
    except FileNotFoundError:
        metadata = None
    if metadata is not None and stat.S_ISLNK(metadata.st_mode):
        os.unlink(name, dir_fd=parent)
        metadata = None
    if metadata is not None:
        if not stat.S_ISDIR(metadata.st_mode):
            raise RuntimeError("refusing unsafe pinned output rollback target")
        descriptor = open_directory(name, parent)
        try:
            members = os.listdir(descriptor)
            if set(members) - set(EXPECTED):
                raise RuntimeError("refusing unexpected pinned output rollback member")
            for member in members:
                item = os.stat(member, dir_fd=descriptor, follow_symlinks=False)
                if not stat.S_ISREG(item.st_mode):
                    raise RuntimeError("refusing unexpected pinned output rollback member")
                os.unlink(member, dir_fd=descriptor)
        finally:
            os.close(descriptor)
        os.rmdir(name, dir_fd=parent)
    os.mkdir(name, mode=0o700, dir_fd=parent)
    os.fsync(parent)


def promote(
    staging: pathlib.Path,
    destination: pathlib.Path,
    after_rename: Callable[[], None] | None = None,
    before_rename: Callable[[], None] | None = None,
) -> None:
    if staging.parent != destination.parent:
        raise RuntimeError("pinned output promotion crossed filesystems")
    staging_name = canonical_leaf(staging)
    destination_name = canonical_leaf(destination)
    parent = os.open(
        staging.parent,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
    renamed = False
    try:
        staging_descriptor = open_directory(staging_name, parent)
        try:
            staging_identity = verify_directory(staging_descriptor)
        finally:
            os.close(staging_descriptor)
        require_empty_directory(destination_name, parent)
        os.rmdir(destination_name, dir_fd=parent)
        if before_rename is not None:
            before_rename()
        current = os.stat(staging_name, dir_fd=parent, follow_symlinks=False)
        if not stat.S_ISDIR(current.st_mode) or (current.st_dev, current.st_ino) != staging_identity:
            raise RuntimeError("pinned output staging changed before promotion")
        try:
            os.stat(destination_name, dir_fd=parent, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise RuntimeError("pinned output destination reappeared before promotion")
        os.rename(staging_name, destination_name, src_dir_fd=parent, dst_dir_fd=parent)
        renamed = True
        destination_descriptor = open_directory(destination_name, parent)
        try:
            if verify_directory(destination_descriptor) != staging_identity:
                raise RuntimeError("pinned output promotion changed directory identity")
            if after_rename is not None:
                after_rename()
            os.fsync(destination_descriptor)
        finally:
            os.close(destination_descriptor)
        os.fsync(parent)
    except BaseException:
        try:
            destination_metadata = os.stat(
                destination_name,
                dir_fd=parent,
                follow_symlinks=False,
            )
            destination_absent = False
        except FileNotFoundError:
            destination_metadata = None
            destination_absent = True
        if renamed or destination_absent or (
            destination_metadata is not None and stat.S_ISLNK(destination_metadata.st_mode)
        ):
            restore_empty(destination_name, parent)
        raise
    finally:
        os.close(parent)


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: promote-pinned-output.py STAGING EMPTY_DESTINATION")
    promote(pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]))


if __name__ == "__main__":
    main()
