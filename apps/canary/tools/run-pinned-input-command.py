#!/usr/bin/env python3
"""Feed exact committed Git input to one bounded owned command."""

import argparse
import hashlib
import io
import os
import selectors
import signal
import subprocess
import sys
import threading
import time
import tarfile


def stop_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except (ProcessLookupError, PermissionError):
        if process.poll() is None:
            process.wait()
        return
    time.sleep(0.1)
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
    if process.poll() is None:
        process.wait()


def run_git(arguments: argparse.Namespace, command: list[str]) -> bytes:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        stdout, stderr = process.communicate(timeout=min(arguments.seconds, 120))
    except subprocess.TimeoutExpired:
        process.kill()
        process.communicate()
        raise SystemExit("committed input producer exceeded its deadline")
    if process.returncode != 0:
        raise SystemExit(f"committed input producer failed: {stderr[:1024]!r}")
    if len(stdout) == 0 or len(stdout) > arguments.maximum_input_bytes:
        raise SystemExit("committed input exceeded its byte bound")
    return stdout


def committed_blob(arguments: argparse.Namespace) -> bytes:
    object_name = f"{arguments.revision}:{arguments.path}"
    size_result = subprocess.run(
        ["git", "-C", arguments.repository, "cat-file", "-s", object_name],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=min(arguments.seconds, 30),
    )
    try:
        size = int(size_result.stdout)
    except ValueError as error:
        raise SystemExit(f"committed input size was invalid: {error}") from error
    if size <= 0 or size > arguments.maximum_input_bytes:
        raise SystemExit("committed input exceeded its byte bound")
    blob = run_git(
        arguments,
        ["git", "-C", arguments.repository, "cat-file", "blob", object_name],
    )
    if len(blob) != size or hashlib.sha256(blob).hexdigest() != arguments.expected_sha256:
        raise SystemExit("committed input disagreed with its exact manifest claim")
    return blob


def committed_archive(arguments: argparse.Namespace) -> bytes:
    if not arguments.archive_path or not arguments.archive_prefix.endswith("/"):
        raise SystemExit("committed archive arguments were incomplete")
    archive = run_git(
        arguments,
        [
            "git",
            "-C",
            arguments.repository,
            "archive",
            "--format=tar",
            f"--prefix={arguments.archive_prefix}",
            arguments.revision,
            "--",
            *arguments.archive_path,
        ],
    )
    if not arguments.extra_file or not arguments.extra_name or not arguments.extra_sha256:
        raise SystemExit("committed archive extra input was incomplete")
    with open(arguments.extra_file, "rb") as source:
        extra = source.read(1_048_577)
    if len(extra) == 0 or len(extra) > 1_048_576 \
            or hashlib.sha256(extra).hexdigest() != arguments.extra_sha256:
        raise SystemExit("committed archive extra input disagreed with its exact claim")
    result = io.BytesIO()
    count = 0
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as source, \
            tarfile.open(fileobj=result, mode="w:") as destination:
        for member in source:
            count += 1
            if count > 4_096 or member.name == arguments.extra_name:
                raise SystemExit("committed archive inventory was invalid")
            stream = source.extractfile(member) if member.isfile() else None
            destination.addfile(member, stream)
        extra_member = tarfile.TarInfo(arguments.extra_name)
        extra_member.mode = 0o400
        extra_member.size = len(extra)
        destination.addfile(extra_member, io.BytesIO(extra))
    value = result.getvalue()
    if len(value) > arguments.maximum_input_bytes:
        raise SystemExit("committed archive context exceeded its byte bound")
    return value


def write_input(stream, value: bytes, failures: list[BaseException]) -> None:
    try:
        stream.write(value)
        stream.close()
    except BrokenPipeError as error:
        failures.append(error)


def run_consumer(arguments: argparse.Namespace, value: bytes) -> int:
    process = subprocess.Popen(
        arguments.command[1:],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    assert process.stdin is not None
    assert process.stdout is not None
    failures: list[BaseException] = []
    writer = threading.Thread(target=write_input, args=(process.stdin, value, failures), daemon=True)
    writer.start()
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    deadline = time.monotonic() + arguments.seconds
    observed = 0
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                stop_group(process)
                print("pinned-input command exceeded its deadline", file=sys.stderr)
                return 124
            events = selector.select(min(remaining, 0.25))
            if not events and process.poll() is not None:
                events = selector.select(0)
            for key, _ in events:
                chunk = os.read(key.fd, 16_384)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                observed += len(chunk)
                if observed > arguments.bytes:
                    stop_group(process)
                    print("pinned-input command exceeded its output limit", file=sys.stderr)
                    return 125
                sys.stdout.buffer.write(chunk)
                sys.stdout.buffer.flush()
        writer.join(timeout=max(0.0, deadline - time.monotonic()))
        if writer.is_alive():
            stop_group(process)
            print("pinned-input command did not consume its exact input", file=sys.stderr)
            return 124
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            stop_group(process)
            print("pinned-input command exceeded its deadline", file=sys.stderr)
            return 124
        try:
            status = process.wait(timeout=remaining)
        except subprocess.TimeoutExpired:
            stop_group(process)
            print("pinned-input command exceeded its deadline", file=sys.stderr)
            return 124
        if failures and status == 0:
            print("pinned-input command closed its exact input early", file=sys.stderr)
            return 126
        return status
    except BaseException:
        stop_group(process)
        raise


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--path", required=True)
    parser.add_argument("--expected-sha256")
    parser.add_argument("--kind", choices=("blob", "archive"), default="blob")
    parser.add_argument("--archive-path", nargs="+")
    parser.add_argument("--archive-prefix", default="")
    parser.add_argument("--extra-file")
    parser.add_argument("--extra-name")
    parser.add_argument("--extra-sha256")
    parser.add_argument("--maximum-input-bytes", type=int, required=True)
    parser.add_argument("--seconds", type=int, required=True)
    parser.add_argument("--bytes", type=int, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    if (
        not arguments.command
        or arguments.command[0] != "--"
        or len(arguments.command) == 1
        or arguments.maximum_input_bytes <= 0
        or arguments.maximum_input_bytes > 83_886_080
        or arguments.seconds <= 0
        or arguments.seconds > 3_600
        or arguments.bytes <= 0
        or arguments.bytes > 16_777_216
        or len(arguments.path) > 512
        or arguments.path.startswith("/")
        or ".." in arguments.path.split("/")
    ):
        parser.error("pinned-input command arguments exceeded their limits")
    if arguments.kind == "blob" and (
        arguments.expected_sha256 is None
        or len(arguments.expected_sha256) != 64
        or any(byte not in "0123456789abcdef" for byte in arguments.expected_sha256)
    ):
        parser.error("pinned blob identity was invalid")
    value = committed_blob(arguments) if arguments.kind == "blob" else committed_archive(arguments)
    return run_consumer(arguments, value)


if __name__ == "__main__":
    raise SystemExit(main())
