#!/usr/bin/env python3
"""Run one owned command with exact elapsed-time and combined-output bounds."""

import argparse
import os
import selectors
import signal
import subprocess
import sys
import time


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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seconds", type=int, required=True)
    parser.add_argument("--bytes", type=int, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    if (
        arguments.seconds <= 0
        or arguments.seconds > 3_600
        or arguments.bytes <= 0
        or arguments.bytes > 16_777_216
        or not arguments.command
        or arguments.command[0] != "--"
        or len(arguments.command) == 1
    ):
        parser.error("bounded command arguments exceeded their limits")
    command = arguments.command[1:]
    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    assert process.stdout is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    deadline = time.monotonic() + arguments.seconds
    observed = 0
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                stop_group(process)
                print("bounded command exceeded its deadline", file=sys.stderr)
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
                    print("bounded command exceeded its output limit", file=sys.stderr)
                    return 125
                sys.stdout.buffer.write(chunk)
                sys.stdout.buffer.flush()
        return process.wait()
    except BaseException:
        stop_group(process)
        raise


if __name__ == "__main__":
    raise SystemExit(main())
