"""Private bounded child-process and disposable-relay lifecycle for the harness."""

from __future__ import annotations

import errno
import json
import os
import pty
import re
import select
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
INTERACTIVE_SECONDS = 60.0
MAX_INTERACTIVE_RESULTS = 8
_CLASSIC_HUMAN_RESULT = re.compile(
    rb"\[(?:Ok|Refused|Failed)\] ([a-z][a-z-]*): [^\r\n]*\r?\n"
    rb"((?:  [a-z_]+=[^\r\n]*\r?\n)*)"
)
_POLISHED_HUMAN_RESULT = re.compile(
    "(?:✓|!|×)  ([a-z][a-z ]*)  [^\r\n]*\r?\n"
    "((?:   [^\r\n]*\r?\n)*)".encode()
)
_POLISHED_FIELD_NAMES = {"public key": "public_key", "event": "event_id"}
_POLISHED_CAPTURE_FIELDS = {
    "account",
    "public_key",
    "author",
    "event_id",
    "kind",
    "group",
    "content",
}
_POLISHED_RESULT_KINDS = {
    "event acknowledged": "group-event-published",
    "relay state": "group-state",
}
_CURSOR_POSITION_QUERY = b"\x1b[6n"
_CURSOR_POSITION_RESPONSE = b"\x1b[1;1R"
_POLISHED_PROMPT_SUFFIX = "› ".encode()


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


def _write_pty(descriptor: int, value: bytes | bytearray) -> None:
    view = memoryview(value)
    while view:
        try:
            written = os.write(descriptor, view)
        except OSError as error:
            raise HarnessError("interactive REPL terminal input failed") from error
        view = view[written:]


def _read_pty_until(
    descriptor: int,
    transcript: bytearray,
    marker: bytes,
    offset: int,
    deadline: float,
    cursor_scan: int,
) -> int:
    while marker not in transcript[offset:]:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise HarnessError("interactive REPL did not produce its bounded expected terminal output")
        readable, _, _ = select.select([descriptor], [], [], remaining)
        if not readable:
            continue
        try:
            chunk = os.read(descriptor, 16_384)
        except OSError as error:
            if error.errno == errno.EIO:
                chunk = b""
            else:
                raise HarnessError("interactive REPL terminal read failed") from error
        if not chunk:
            raise HarnessError("interactive REPL ended before its expected terminal output")
        transcript.extend(chunk)
        if len(transcript) > MAX_LOG_BYTES:
            raise HarnessError(f"interactive REPL output exceeded {MAX_LOG_BYTES} bytes")
        while True:
            query = transcript.find(_CURSOR_POSITION_QUERY, cursor_scan)
            if query == -1:
                cursor_scan = max(cursor_scan, len(transcript) - len(_CURSOR_POSITION_QUERY) + 1)
                break
            _write_pty(descriptor, _CURSOR_POSITION_RESPONSE)
            cursor_scan = query + len(_CURSOR_POSITION_QUERY)
    return cursor_scan


def _drain_pty(descriptor: int, transcript: bytearray, deadline: float) -> None:
    while time.monotonic() < deadline:
        readable, _, _ = select.select([descriptor], [], [], max(0.0, deadline - time.monotonic()))
        if not readable:
            return
        try:
            chunk = os.read(descriptor, 16_384)
        except OSError as error:
            if error.errno == errno.EIO:
                return
            raise HarnessError("interactive REPL terminal drain failed") from error
        if not chunk:
            return
        transcript.extend(chunk)
        if len(transcript) > MAX_LOG_BYTES:
            raise HarnessError(f"interactive REPL output exceeded {MAX_LOG_BYTES} bytes")


def _stop_pty_process(process: subprocess.Popen[bytes]) -> None:
    try:
        process.wait(timeout=0.1)
        return
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        except PermissionError as error:
            raise HarnessError("interactive REPL process group could not be terminated") from error
        try:
            process.wait(timeout=TEARDOWN_SECONDS)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            except PermissionError as error:
                raise HarnessError("interactive REPL process group could not be killed") from error
            try:
                process.wait(timeout=TEARDOWN_SECONDS)
            except subprocess.TimeoutExpired as error:
                raise HarnessError("interactive REPL process group survived teardown") from error
    try:
        os.killpg(process.pid, 0)
    except ProcessLookupError:
        return
    except PermissionError as error:
        raise HarnessError("interactive REPL process group teardown could not be verified") from error
    raise HarnessError("interactive REPL descendants survived teardown")


def _human_results(transcript: bytearray) -> dict[str, dict[str, str]]:
    """Extract bounded public fields from classic or polished terminal results."""

    results: dict[str, dict[str, str]] = {}
    for match in _CLASSIC_HUMAN_RESULT.finditer(transcript):
        fields: dict[str, str] = {}
        for line in match.group(2).splitlines():
            name, separator, value = line[2:].partition(b"=")
            if not separator:
                raise HarnessError("interactive REPL rendered an invalid classic result field")
            _insert_human_field(fields, name.decode("ascii"), value)
        _insert_human_result(results, match.group(1).decode("ascii"), fields)
    for match in _POLISHED_HUMAN_RESULT.finditer(transcript):
        fields = {}
        for line in match.group(2).splitlines():
            value = line[3:]
            split = re.match(rb"([A-Za-z_ ]+?)\s{2,}(.+)$", value)
            if split is None:
                continue
            label = split.group(1).decode("ascii").strip()
            name = _POLISHED_FIELD_NAMES.get(label, label.replace(" ", "_"))
            if name not in _POLISHED_CAPTURE_FIELDS:
                continue
            _insert_human_field(fields, name, split.group(2))
        heading = match.group(1).decode("ascii").strip()
        kind = _POLISHED_RESULT_KINDS.get(heading, heading.replace(" ", "-"))
        _insert_human_result(results, kind, fields)
    return results


def _insert_human_field(fields: dict[str, str], name: str, value: bytes) -> None:
    if not name or name in fields or len(value) > 4_096:
        raise HarnessError("interactive REPL rendered an invalid bounded result field")
    try:
        fields[name] = value.decode("ascii")
    except UnicodeDecodeError as error:
        raise HarnessError("interactive REPL rendered a non-ASCII public result field") from error


def _insert_human_result(
    results: dict[str, dict[str, str]], kind: str, fields: dict[str, str]
) -> None:
    if len(results) == MAX_INTERACTIVE_RESULTS:
        raise HarnessError(f"interactive REPL exceeded {MAX_INTERACTIVE_RESULTS} result records")
    if kind in results:
        raise HarnessError(f"interactive REPL repeated result kind {kind!r}")
    results[kind] = fields


def _require_no_secret_echo(transcript: bytearray, secret: bytearray) -> None:
    if transcript.find(secret) != -1:
        raise HarnessError("protected account import echoed its secret into the PTY")


def run_interactive_import(
    command: list[str],
    environment: dict[str, str],
    secret: bytearray,
    commands: tuple[tuple[str, str], ...],
) -> dict[str, dict[str, str]]:
    """Run one no-echo account import and return only its public typed result fields.

    The disposable PTY is never logged. The caller retains the mutable secret only
    until its final retained-artifact scan; this function rejects any terminal echo.
    """

    if not command or not secret or len(secret) > 1_024:
        raise HarnessError("interactive import input was outside its explicit bound")
    master, slave = pty.openpty()
    process: subprocess.Popen[bytes] | None = None
    transcript = bytearray()
    phase = "startup"
    try:
        process = subprocess.Popen(
            command,
            cwd=ROOT,
            env=environment,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            close_fds=True,
            start_new_session=True,
        )
        os.close(slave)
        slave = -1
        deadline = time.monotonic() + INTERACTIVE_SECONDS
        cursor_scan = 0
        phase = "initial prompt"
        cursor_scan = _read_pty_until(
            master, transcript, _POLISHED_PROMPT_SUFFIX, 0, deadline, cursor_scan
        )
        phase = "protected secret prompt"
        _write_pty(master, b"account import imported\n")
        prompt_offset = len(transcript)
        try:
            cursor_scan = _read_pty_until(
                master,
                transcript,
                b"account private key: ",
                prompt_offset,
                deadline,
                cursor_scan,
            )
        except HarnessError as error:
            preview = bytes(transcript[-4_096:]).decode("utf-8", "replace")
            raise HarnessError(
                f"{error}; protected pre-input terminal tail={preview!r}"
            ) from error
        phase = "account import result"
        _write_pty(master, secret)
        _write_pty(master, b"\n")
        import_offset = len(transcript)
        cursor_scan = _read_pty_until(
            master,
            transcript,
            "✓  account imported".encode(),
            import_offset,
            deadline,
            cursor_scan,
        )
        for text, result_kind in commands:
            phase = f"{result_kind} result"
            _write_pty(master, text.encode("ascii") + b"\n")
            result_offset = len(transcript)
            heading = next(
                (heading for heading, kind in _POLISHED_RESULT_KINDS.items() if kind == result_kind),
                result_kind.replace("-", " "),
            )
            marker = f"✓  {heading}".encode()
            cursor_scan = _read_pty_until(
                master, transcript, marker, result_offset, deadline, cursor_scan
            )
        phase = "successful exit"
        if process.wait(timeout=max(0.1, deadline - time.monotonic())) != 0:
            raise HarnessError("interactive REPL exited unsuccessfully after account import")
        _drain_pty(master, transcript, deadline)
        _require_no_secret_echo(transcript, secret)
        return _human_results(transcript)
    except (OSError, UnicodeEncodeError) as error:
        raise HarnessError("interactive REPL setup failed") from error
    except HarnessError as error:
        raise HarnessError(f"interactive REPL {phase} failed: {error}") from error
    finally:
        try:
            if process is not None:
                _stop_pty_process(process)
        finally:
            if slave != -1:
                os.close(slave)
            os.close(master)
            transcript[:] = b"\0" * len(transcript)
