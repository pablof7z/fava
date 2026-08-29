"""Private bounded child-process and disposable-relay lifecycle for the harness."""

from __future__ import annotations

import json
import os
import signal
import subprocess
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from harness_safety import HarnessError, MAX_COMMAND_BYTES, MAX_LOG_BYTES, read_bounded_text
from relay_inspection import InspectionError, inspect_until_eose

ROOT = Path(__file__).resolve().parents[3]
READINESS_SECONDS = 10.0
TEARDOWN_SECONDS = 5.0
APP_SECONDS = 60.0


class BoundedLog:
    def __init__(self, stream: Any, path: Path, on_line: Callable[[bytes], None] | None = None) -> None:
        self.path, self._stream, self._on_line = path, stream, on_line
        self._partial, self._overflow = bytearray(), threading.Event()
        self._thread = threading.Thread(target=self._drain, daemon=True)

    def start(self) -> None:
        self._thread.start()

    def join(self) -> bool:
        self._thread.join(timeout=TEARDOWN_SECONDS)
        if not self._thread.is_alive():
            self._stream.close()
            return True
        return False

    @property
    def overflowed(self) -> bool:
        return self._overflow.is_set()

    def _drain(self) -> None:
        retained = 0
        with self.path.open("wb") as destination:
            while chunk := self._stream.read1(16_384):
                available = max(0, MAX_LOG_BYTES - retained)
                if len(chunk) > available:
                    self._overflow.set()
                if not available:
                    continue
                accepted = chunk[:available]
                destination.write(accepted)
                destination.flush()
                retained += len(accepted)
                if self._on_line is not None:
                    self._partial.extend(accepted)
                    while b"\n" in self._partial:
                        line, _, rest = self._partial.partition(b"\n")
                        self._partial = bytearray(rest)
                        self._on_line(bytes(line))


@dataclass
class ManagedProcess:
    label: str
    process: subprocess.Popen[bytes]
    stdout: BoundedLog
    stderr: BoundedLog

    @classmethod
    def start(
        cls, label: str, command: list[str], directory: Path, environment: dict[str, str],
        on_stdout_line: Callable[[bytes, subprocess.Popen[bytes]], None] | None = None,
        stdin_pipe: bool = False,
    ) -> "ManagedProcess":
        if not command:
            raise HarnessError(f"{label} command was empty")
        process = subprocess.Popen(command, cwd=ROOT, env=environment,
            stdin=subprocess.PIPE if stdin_pipe else subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        if process.stdout is None or process.stderr is None:
            raise HarnessError(f"{label} did not expose bounded output streams")
        stdout = BoundedLog(process.stdout, directory / "stdout.log",
            (lambda line: on_stdout_line(line, process)) if on_stdout_line else None)
        stderr = BoundedLog(process.stderr, directory / "stderr.log")
        stdout.start(); stderr.start()
        return cls(label, process, stdout, stderr)

    def require_healthy(self) -> None:
        if self.stdout.overflowed or self.stderr.overflowed:
            raise HarnessError(f"{self.label} output exceeded {MAX_LOG_BYTES} bytes")
        if (status := self.process.poll()) is not None:
            raise HarnessError(f"{self.label} exited before readiness with status {status}")

    def _group_alive(self) -> bool:
        try:
            os.killpg(self.process.pid, 0)
        except (ProcessLookupError, PermissionError):
            return False
        return True

    def stop(self) -> dict[str, Any]:
        parent_exited = self.process.poll() is not None
        alive = self._group_alive(); term_sent = kill_sent = teardown_error = permission_denied = False
        if alive:
            try: os.killpg(self.process.pid, signal.SIGTERM); term_sent = True
            except PermissionError: teardown_error = permission_denied = True
            except ProcessLookupError: pass
        deadline = time.monotonic() + TEARDOWN_SECONDS
        while self._group_alive() and time.monotonic() < deadline: time.sleep(0.02)
        if self._group_alive():
            try: os.killpg(self.process.pid, signal.SIGKILL); kill_sent = True
            except PermissionError: teardown_error = permission_denied = True
            except ProcessLookupError: pass
            deadline = time.monotonic() + TEARDOWN_SECONDS
            while self._group_alive() and time.monotonic() < deadline: time.sleep(0.02)
        gone = not self._group_alive()
        if self.process.poll() is None:
            try: self.process.wait(timeout=max(0.1, TEARDOWN_SECONDS))
            except subprocess.TimeoutExpired: teardown_error = True
        joined = self.stdout.join() and self.stderr.join()
        return {"descendants_survived_parent": parent_exited and alive, "group_gone": gone,
            "group_alive_before_teardown": alive, "kill_sent": kill_sent,
            "output_overflowed": self.stdout.overflowed or self.stderr.overflowed,
            "output_threads_joined": joined, "parent_exited_before_teardown": parent_exited,
            "permission_denied": permission_denied, "pid": self.process.pid,
            "returncode": self.process.returncode, "stderr_bytes": self.stderr.path.stat().st_size,
            "stdout_bytes": self.stdout.path.stat().st_size, "teardown_error": teardown_error,
            "term_sent": term_sent}


@dataclass(frozen=True)
class Relay:
    label: str
    url: str
    process: ManagedProcess


def require_stopped(label: str, facts: dict[str, Any]) -> None:
    if not facts["group_gone"]: raise HarnessError(f"{label} process group survived teardown")
    if facts["teardown_error"]: raise HarnessError(f"{label} process-group teardown reported an operating-system error")
    if not facts["output_threads_joined"]: raise HarnessError(f"{label} bounded-output drain did not finish after teardown")


def materialize_commands(source: Path, destination: Path, replacements: dict[str, str]) -> None:
    value = read_bounded_text(source, MAX_COMMAND_BYTES, "command source")
    for name, replacement in replacements.items():
        value = value.replace("{{" + name + "}}", replacement)
    if "{{" in value or "}}" in value:
        raise HarnessError(f"unresolved command placeholder in {source}")
    if len(value.encode("utf-8")) > MAX_COMMAND_BYTES:
        raise HarnessError(f"materialized command exceeded {MAX_COMMAND_BYTES} bytes")
    destination.write_text(value, encoding="utf-8")


def _port() -> int:
    import socket
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0)); return int(listener.getsockname()[1])


def start_croissant(binary: Path, root: Path, environment: dict[str, str]) -> Relay:
    port = _port(); relay_root = root / "relays" / "nip29"; relay_root.mkdir(parents=True)
    data = relay_root / "data"; data.mkdir()
    process = ManagedProcess.start("NIP-29 relay", [str(binary)], relay_root,
        {**environment, "HOST": "127.0.0.1", "PORT": str(port), "DATAPATH": str(data)})
    return Relay("group", f"ws://127.0.0.1:{port}", process)


def start_ordinary(binary: Path, root: Path, environment: dict[str, str]) -> Relay:
    port = _port(); relay_root = root / "relays" / "ordinary"; relay_root.mkdir(parents=True)
    data = relay_root / "data"; data.mkdir(); config = relay_root / "config.toml"
    config.write_text(f'''[info]\nrelay_url = "ws://127.0.0.1:{port}/"\n[database]\nengine = "sqlite"\nin_memory = false\n[network]\naddress = "127.0.0.1"\nport = {port}\nping_interval = 30\n[options]\nreject_future_seconds = 1800\n[limits]\nmax_event_bytes = 131072\nmax_ws_message_bytes = 131072\nmax_ws_frame_bytes = 131072\nbroadcast_buffer = 1024\nevent_persist_buffer = 1024\n[authorization]\nnip42_auth = false\n''', encoding="utf-8")
    process = ManagedProcess.start("ordinary relay", [str(binary), "--config", str(config), "--db", str(data)], relay_root, {**environment, "RUST_LOG": "info"})
    return Relay("state", f"ws://127.0.0.1:{port}", process)


def wait_ready(relay: Relay) -> None:
    deadline = time.monotonic() + READINESS_SECONDS
    while time.monotonic() < deadline:
        relay.process.require_healthy()
        try:
            inspect_until_eose(relay.url, f"ready-{relay.label}", {"limit": 1}, min(1.0, max(0.1, deadline - time.monotonic())))
            return
        except (InspectionError, OSError): time.sleep(0.05)
    relay.process.require_healthy()
    raise HarnessError(f"{relay.label} relay did not complete REQ/EOSE readiness in {READINESS_SECONDS}s")


def run_app(command: list[str], root: Path, environment: dict[str, str], on_row: Callable[[dict[str, Any], subprocess.Popen[bytes]], None] | None = None, input_lines: list[str] | None = None) -> tuple[int, Path, Path]:
    app_root = root / "app"; app_root.mkdir(); errors: list[BaseException] = []; received = 0; condition = threading.Condition()
    def line(value: bytes, process: subprocess.Popen[bytes]) -> None:
        nonlocal received
        if on_row is None or errors: return
        try:
            row = json.loads(value.decode("utf-8"))
            if not isinstance(row, dict): raise HarnessError("application stdout line was not a JSON object")
            on_row(row, process)
            with condition: received += 1; condition.notify_all()
        except BaseException as error:
            with condition: errors.append(error); condition.notify_all()
    managed = ManagedProcess.start("REPL application", command, app_root, environment, line, input_lines is not None)
    try:
        if input_lines is not None:
            if managed.process.stdin is None: raise HarnessError("REPL application did not expose script input")
            for expected, text in enumerate(input_lines, 1):
                managed.process.stdin.write((text + "\n").encode()); managed.process.stdin.flush()
                with condition:
                    if not condition.wait_for(lambda: received >= expected or errors, APP_SECONDS):
                        raise HarnessError("REPL application did not render one typed result per command")
                    if errors: raise errors[0]
            managed.process.stdin.close()
        managed.process.wait(timeout=APP_SECONDS)
    except subprocess.TimeoutExpired as error:
        facts = managed.stop(); require_stopped("REPL application", facts)
        raise HarnessError(f"REPL application exceeded its {APP_SECONDS}s deadline") from error
    facts = managed.stop(); require_stopped("REPL application", facts)
    if facts["output_overflowed"]: raise HarnessError("REPL application exceeded the retained output bound")
    if errors: raise errors[0]
    return int(facts["returncode"]), managed.stdout.path, managed.stderr.path
