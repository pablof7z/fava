#!/usr/bin/env python3
"""Independent, bounded live harness for the simple-groups REPL.

The harness supervises disposable relays, invokes ordinary REPL command files,
keeps bounded artifacts, and independently reads relay state via REQ/EOSE. It
does not construct, sign, route, or publish Nostr events.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from harness_safety import (
    MAX_ARTIFACT_FILE_BYTES,
    MAX_ASSERTIONS,
    MAX_BINARY_BYTES,
    MAX_COMMAND_BYTES,
    MAX_FILTER_BYTES,
    MAX_JSONL_LINE_BYTES,
    MAX_JSONL_ROWS,
    MAX_LOG_BYTES,
    MAX_SCENARIO_BYTES,
    MAX_SECRET_SENTINEL_BYTES,
    MAX_SECRET_SENTINELS,
    HarnessError,
    new_scratch,
    read_bounded_text,
    remove_scratch,
    scan_secret_absence,
)
from relay_inspection import InspectionError, assert_event, inspect_until_eose

ROOT = Path(__file__).resolve().parents[3]
LIVE = Path(__file__).resolve().parent
SCENARIOS = LIVE / "scenarios"
READINESS_SECONDS = 10.0
TEARDOWN_SECONDS = 5.0
APP_SECONDS = 60.0
INSPECTION_SECONDS = 10.0
GROUP_ID_PATTERN = re.compile(r"^[a-z0-9][a-z0-9-]{0,62}$")

def json_line(value: dict[str, Any]) -> str:
    """Render deterministic, one-object-per-line evidence."""

    return json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True) + "\n"
class RunLog:
    """Append-only, deterministic JSONL artifact writer."""
    def __init__(self, path: Path) -> None:
        self.path = path
        self._sequence = 0

    def record(self, action: str, **facts: Any) -> None:
        self._sequence += 1
        record = {"action": action, "sequence": self._sequence, **facts}
        with self.path.open("a", encoding="utf-8") as output:
            output.write(json_line(record))
        print(json_line(record), end="")


class BoundedLog:
    """Drain one child stream while retaining at most its declared byte bound."""
    def __init__(
        self,
        stream: Any,
        path: Path,
        byte_limit: int = MAX_LOG_BYTES,
        on_line: Callable[[bytes], None] | None = None,
    ) -> None:
        self.path = path
        self.byte_limit = byte_limit
        self._stream = stream
        self._on_line = on_line
        self._partial = bytearray()
        self._overflow = threading.Event()
        self._thread = threading.Thread(target=self._drain, daemon=True)

    def start(self) -> None:
        self._thread.start()

    def join(self) -> bool:
        self._thread.join(timeout=TEARDOWN_SECONDS)
        finished = not self._thread.is_alive()
        if finished:
            self._stream.close()
        return finished

    @property
    def overflowed(self) -> bool:
        return self._overflow.is_set()

    def _drain(self) -> None:
        retained = 0
        with self.path.open("wb") as destination:
            while True:
                chunk = self._stream.read1(16_384)
                if not chunk:
                    return
                available = max(0, self.byte_limit - retained)
                if len(chunk) > available:
                    self._overflow.set()
                if available:
                    accepted = chunk[:available]
                    destination.write(accepted)
                    destination.flush()
                    retained += len(accepted)
                    if self._on_line is not None:
                        self._partial.extend(accepted)
                        while b"\n" in self._partial:
                            line, _, remainder = self._partial.partition(b"\n")
                            self._partial = bytearray(remainder)
                            self._on_line(bytes(line))


@dataclass
class ManagedProcess:
    """One process with process-group teardown and bounded retained output."""
    label: str
    process: subprocess.Popen[bytes]
    stdout: BoundedLog
    stderr: BoundedLog

    @classmethod
    def start(
        cls,
        label: str,
        command: list[str],
        directory: Path,
        environment: dict[str, str],
        on_stdout_line: Callable[[bytes, subprocess.Popen[bytes]], None] | None = None,
        stdin_pipe: bool = False,
    ) -> "ManagedProcess":
        if not command:
            raise HarnessError(f"{label} command was empty")
        process = subprocess.Popen(
            command,
            cwd=ROOT,
            env=environment,
            stdin=subprocess.PIPE if stdin_pipe else subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        if process.stdout is None or process.stderr is None:
            raise HarnessError(f"{label} did not expose bounded output streams")
        stdout = BoundedLog(
            process.stdout,
            directory / "stdout.log",
            on_line=(lambda line: on_stdout_line(line, process)) if on_stdout_line else None,
        )
        stderr = BoundedLog(process.stderr, directory / "stderr.log")
        stdout.start()
        stderr.start()
        return cls(label, process, stdout, stderr)
    def require_healthy(self) -> None:
        if self.stdout.overflowed or self.stderr.overflowed:
            raise HarnessError(f"{self.label} output exceeded {MAX_LOG_BYTES} bytes")
        status = self.process.poll()
        if status is not None:
            raise HarnessError(f"{self.label} exited before readiness with status {status}")

    def stop(self) -> dict[str, Any]:
        """Terminate the whole original process group, even after parent exit."""
        parent_exited_before_teardown = self.process.poll() is not None
        group_alive_before_teardown = self._group_alive()
        term_sent = False
        kill_sent = False
        teardown_error = False
        permission_denied = False
        if group_alive_before_teardown:
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
                term_sent = True
            except PermissionError:
                teardown_error = permission_denied = True
            except ProcessLookupError:
                pass
        term_deadline = time.monotonic() + TEARDOWN_SECONDS
        while self._group_alive() and time.monotonic() < term_deadline:
            time.sleep(0.02)
        if self._group_alive():
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
                kill_sent = True
            except PermissionError:
                teardown_error = permission_denied = True
            except ProcessLookupError:
                pass
            kill_deadline = time.monotonic() + TEARDOWN_SECONDS
            while self._group_alive() and time.monotonic() < kill_deadline:
                time.sleep(0.02)
        group_gone = not self._group_alive()
        if self.process.poll() is None:
            try:
                self.process.wait(timeout=max(0.1, TEARDOWN_SECONDS))
            except subprocess.TimeoutExpired:
                teardown_error = True
        stdout_joined = self.stdout.join()
        stderr_joined = self.stderr.join()
        return {
            "descendants_survived_parent": parent_exited_before_teardown and group_alive_before_teardown,
            "group_gone": group_gone,
            "group_alive_before_teardown": group_alive_before_teardown,
            "kill_sent": kill_sent,
            "output_overflowed": self.stdout.overflowed or self.stderr.overflowed,
            "output_threads_joined": stdout_joined and stderr_joined,
            "parent_exited_before_teardown": parent_exited_before_teardown,
            "permission_denied": permission_denied,
            "pid": self.process.pid,
            "returncode": self.process.returncode,
            "stderr_bytes": self.stderr.path.stat().st_size,
            "stdout_bytes": self.stdout.path.stat().st_size,
            "teardown_error": teardown_error,
            "term_sent": term_sent,
        }

    def _group_alive(self) -> bool:
        try:
            os.killpg(self.process.pid, 0)
        except ProcessLookupError:
            return False
        except PermissionError:
            # A reused foreign process group cannot be one of our descendants.
            return False
        return True
@dataclass(frozen=True)
class Relay:
    """One supervised relay role and exact loopback WebSocket endpoint."""
    label: str
    url: str
    process: ManagedProcess


def reserve_port() -> int:
    """Reserve an ephemeral loopback port only long enough to select it."""
    import socket

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def ordinary_config(port: int) -> str:
    """The documented nostr-rs-relay 0.8.12 loopback configuration."""

    return f'''[info]
relay_url = "ws://127.0.0.1:{port}/"
name = "Fava simple-groups live harness"
description = "Disposable ordinary relay for kind-10009 evidence"

[database]
engine = "sqlite"
in_memory = false

[network]
address = "127.0.0.1"
port = {port}
ping_interval = 30

[options]
reject_future_seconds = 1800

[limits]
max_event_bytes = 131072
max_ws_message_bytes = 131072
max_ws_frame_bytes = 131072
broadcast_buffer = 1024
event_persist_buffer = 1024

[authorization]
nip42_auth = false
'''


def start_croissant(binary: Path, root: Path, environment: dict[str, str]) -> Relay:
    port = reserve_port()
    relay_root = root / "relays" / "nip29"
    relay_root.mkdir(parents=True)
    data = relay_root / "data"
    data.mkdir()
    relay_environment = {
        **environment,
        "HOST": "127.0.0.1",
        "PORT": str(port),
        "DATAPATH": str(data),
    }
    process = ManagedProcess.start("NIP-29 relay", [str(binary)], relay_root, relay_environment)
    return Relay("group", f"ws://127.0.0.1:{port}", process)


def start_ordinary(binary: Path, root: Path, environment: dict[str, str]) -> Relay:
    port = reserve_port()
    relay_root = root / "relays" / "ordinary"
    relay_root.mkdir(parents=True)
    config = relay_root / "config.toml"
    data = relay_root / "data"
    data.mkdir()
    config.write_text(ordinary_config(port), encoding="utf-8")
    process = ManagedProcess.start(
        "ordinary relay",
        [str(binary), "--config", str(config), "--db", str(data)],
        relay_root,
        {**environment, "RUST_LOG": "info"},
    )
    return Relay("state", f"ws://127.0.0.1:{port}", process)


def wait_ready(relay: Relay) -> None:
    """Require a successful bounded REQ/EOSE, not merely an open TCP port."""

    deadline = time.monotonic() + READINESS_SECONDS
    while time.monotonic() < deadline:
        relay.process.require_healthy()
        try:
            inspect_until_eose(
                relay.url,
                f"ready-{relay.label}",
                {"limit": 1},
                min(1.0, max(0.1, deadline - time.monotonic())),
            )
            return
        except (InspectionError, OSError):
            time.sleep(0.05)
    relay.process.require_healthy()
    raise HarnessError(f"{relay.label} relay did not complete REQ/EOSE readiness in {READINESS_SECONDS}s")


def materialize_commands(source: Path, destination: Path, replacements: dict[str, str]) -> None:
    value = read_bounded_text(source, MAX_COMMAND_BYTES, "command source")
    for name, replacement in replacements.items():
        value = value.replace("{{" + name + "}}", replacement)
    if "{{" in value or "}}" in value:
        raise HarnessError(f"unresolved command placeholder in {source}")
    if len(value.encode("utf-8")) > MAX_COMMAND_BYTES:
        raise HarnessError(f"materialized command exceeded {MAX_COMMAND_BYTES} bytes")
    destination.write_text(value, encoding="utf-8")


def run_app(
    command: list[str],
    root: Path,
    environment: dict[str, str],
    on_row: Callable[[dict[str, Any], subprocess.Popen[bytes]], None] | None = None,
    input_lines: list[str] | None = None,
) -> tuple[int, Path, Path]:
    app_root = root / "app"
    app_root.mkdir()
    callback_errors: list[BaseException] = []
    received_rows = 0
    row_condition = threading.Condition()

    def on_stdout_line(line: bytes, process: subprocess.Popen[bytes]) -> None:
        nonlocal received_rows
        if on_row is None or callback_errors:
            return
        try:
            value = json.loads(line.decode("utf-8"))
            if not isinstance(value, dict):
                raise HarnessError("application stdout line was not a JSON object")
            on_row(value, process)
            with row_condition:
                received_rows += 1
                row_condition.notify_all()
        except BaseException as error:
            with row_condition:
                callback_errors.append(error)
                row_condition.notify_all()

    process = ManagedProcess.start(
        "REPL application",
        command,
        app_root,
        environment,
        on_stdout_line=on_stdout_line,
        stdin_pipe=input_lines is not None,
    )
    try:
        if input_lines is not None:
            if process.process.stdin is None:
                raise HarnessError("REPL application did not expose script input")
            for expected_rows, line in enumerate(input_lines, start=1):
                process.process.stdin.write((line + "\n").encode("utf-8"))
                process.process.stdin.flush()
                deadline = time.monotonic() + APP_SECONDS
                with row_condition:
                    while received_rows < expected_rows and not callback_errors:
                        remaining = deadline - time.monotonic()
                        if remaining <= 0:
                            raise HarnessError("REPL application did not render one typed result per command")
                        row_condition.wait(timeout=remaining)
                    if callback_errors:
                        raise callback_errors[0]
            process.process.stdin.close()
        process.process.wait(timeout=APP_SECONDS)
    except subprocess.TimeoutExpired as error:
        facts = process.stop()
        require_stopped("REPL application", facts)
        raise HarnessError(f"REPL application exceeded its {APP_SECONDS}s deadline") from error
    facts = process.stop()
    require_stopped("REPL application", facts)
    if facts["output_overflowed"]:
        raise HarnessError("REPL application exceeded the retained output bound")
    if callback_errors:
        raise callback_errors[0]
    return int(facts["returncode"]), process.stdout.path, process.stderr.path


def parse_jsonl(path: Path) -> list[dict[str, Any]]:
    value = read_bounded_text(path, MAX_LOG_BYTES, "application stdout")
    rows: list[dict[str, Any]] = []
    for number, line in enumerate(value.splitlines(), start=1):
        if len(line.encode("utf-8")) > MAX_JSONL_LINE_BYTES:
            raise HarnessError(f"application stdout line {number} exceeded {MAX_JSONL_LINE_BYTES} bytes")
        if number > MAX_JSONL_ROWS:
            raise HarnessError(f"application stdout exceeded {MAX_JSONL_ROWS} JSONL rows")
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            raise HarnessError(f"application stdout line {number} was not JSONL") from error
        if not isinstance(value, dict):
            raise HarnessError(f"application stdout line {number} was not a JSON object")
        rows.append(value)
    return rows


def scenario_path(name: str) -> Path:
    candidate = SCENARIOS / f"{name}.json"
    if not candidate.is_file():
        raise HarnessError(f"unknown scenario {name!r}")
    return candidate


def load_scenario(name: str) -> dict[str, Any]:
    try:
        value = json.loads(read_bounded_text(scenario_path(name), MAX_SCENARIO_BYTES, "scenario"))
    except json.JSONDecodeError as error:
        raise HarnessError(f"scenario {name!r} is invalid JSON") from error
    if not isinstance(value, dict) or value.get("id") != name:
        raise HarnessError(f"scenario {name!r} did not have its exact id")
    base_name = value.get("bounded_gap_control_of")
    if base_name is not None:
        if base_name != "full-nip29-contract":
            raise HarnessError("bounded gap control named an unsupported base scenario")
        base = load_scenario(base_name)
        result = json.loads(json.dumps(base))
        result["id"] = name
        result["bounded_gap_control_of"] = base_name
        final_assertion = result["stages"][-1]["assertions"][0]
        if final_assertion.get("event", {}).get("kind") != 9008:
            raise HarnessError("bounded gap control base omitted exact kind-9008 assertion")
        result["stages"][-1]["assertions"][0] = {
            "filter": final_assertion["filter"],
            "present": False,
            "relay": "group",
        }
        return result
    return value


def validate_executable_scenario(scenario: dict[str, Any]) -> tuple[str, ...]:
    """Refuse a scenario that could claim live proof without wire assertions."""

    if "required_facts" in scenario:
        raise HarnessError("executable scenario must convert required_facts into concrete assertions")
    command_file = scenario.get("command_file")
    if not isinstance(command_file, str) or not command_file:
        raise HarnessError("executable scenario requires a command_file")
    expected_exit = scenario.get("app_exit", "zero")
    if expected_exit not in {"zero", "nonzero"}:
        raise HarnessError("executable scenario app_exit must be 'zero' or 'nonzero'")
    assertions = scenario.get("assertions")
    stages = scenario.get("stages")
    if assertions is not None and stages is not None:
        raise HarnessError("executable scenario may use assertions or stages, not both")
    if stages is not None:
        if not isinstance(stages, list) or not stages:
            raise HarnessError("executable scenario requires nonempty bounded assertion stages")
        previous_line = 0
        flattened: list[Any] = []
        for stage_number, stage in enumerate(stages, start=1):
            if not isinstance(stage, dict):
                raise HarnessError(f"scenario stage {stage_number} was not an object")
            after_line = stage.get("after_line")
            if not isinstance(after_line, int) or not 0 < after_line <= MAX_JSONL_ROWS:
                raise HarnessError(f"scenario stage {stage_number} after_line was outside JSONL bounds")
            if after_line <= previous_line:
                raise HarnessError("scenario stages must have strictly increasing after_line values")
            previous_line = after_line
            staged_assertions = stage.get("assertions")
            if not isinstance(staged_assertions, list) or not staged_assertions:
                raise HarnessError(f"scenario stage {stage_number} requires concrete assertions")
            flattened.extend(staged_assertions)
        assertions = flattened
    if not isinstance(assertions, list) or not assertions:
        raise HarnessError("executable scenario requires at least one concrete assertion")
    if len(assertions) > MAX_ASSERTIONS:
        raise HarnessError(f"executable scenario exceeded {MAX_ASSERTIONS} assertions")
    for number, assertion in enumerate(assertions, start=1):
        if not isinstance(assertion, dict):
            raise HarnessError(f"scenario assertion {number} was not an object")
        if assertion.get("relay") not in {"group", "state"}:
            raise HarnessError(f"scenario assertion {number} named an unsupported relay")
        if not isinstance(assertion.get("present"), bool):
            raise HarnessError(f"scenario assertion {number} must state present explicitly")
        event_filter = assertion.get("filter")
        if not isinstance(event_filter, dict) or not event_filter:
            raise HarnessError(f"scenario assertion {number} requires a nonempty relay filter")
        try:
            filter_bytes = len(json.dumps(event_filter, ensure_ascii=True, separators=(",", ":")).encode("utf-8"))
        except (TypeError, ValueError) as error:
            raise HarnessError(f"scenario assertion {number} filter was not JSON data") from error
        if filter_bytes > MAX_FILTER_BYTES:
            raise HarnessError(f"scenario assertion {number} filter exceeded {MAX_FILTER_BYTES} bytes")
        if assertion["present"]:
            if assertion.get("collection") == "relay-state":
                if assertion.get("required_kinds") != [39000, 39001, 39002, 39003]:
                    raise HarnessError(f"scenario assertion {number} has invalid relay-state kinds")
                continue
            event = assertion.get("event")
            if not isinstance(event, dict) or not {"id", "pubkey", "kind", "content", "tags"}.issubset(event):
                raise HarnessError(
                    f"scenario assertion {number} requires exact id, pubkey, kind, content, and tags"
                )
        elif "event" in assertion:
            raise HarnessError(f"negative scenario assertion {number} must not carry an ignored event")
    raw_sentinels = scenario.get("secret_sentinels", [])
    if not isinstance(raw_sentinels, list) or len(raw_sentinels) > MAX_SECRET_SENTINELS:
        raise HarnessError("scenario secret_sentinels exceeded its count bound")
    sentinels: list[str] = []
    for sentinel in raw_sentinels:
        if not isinstance(sentinel, str) or not sentinel:
            raise HarnessError("scenario secret sentinel was not a nonempty string")
        if len(sentinel.encode("utf-8")) > MAX_SECRET_SENTINEL_BYTES:
            raise HarnessError("scenario secret sentinel exceeded its byte bound")
        sentinels.append(sentinel)
    return tuple(sentinels)


def result_captures(rows: list[dict[str, Any]], definitions: dict[str, Any]) -> dict[str, dict[str, Any]]:
    captures: dict[str, dict[str, Any]] = {}
    for name, selector in definitions.items():
        matches = []
        for row in rows:
            if row.get("kind") != selector.get("kind"):
                continue
            fields = row.get("fields")
            required = selector.get("fields", {})
            if isinstance(fields, dict) and all(fields.get(key) == value for key, value in required.items()):
                matches.append(row)
        if len(matches) != 1:
            raise HarnessError(f"capture {name!r} matched {len(matches)} application result rows")
        fields = matches[0].get("fields")
        if not isinstance(fields, dict):
            raise HarnessError(f"capture {name!r} result omitted fields")
        captures[name] = fields
    return captures


def resolve(value: Any, context: dict[str, Any]) -> Any:
    if isinstance(value, str) and value.startswith("$"):
        current: Any = context
        for segment in value[1:].split("."):
            if not isinstance(current, dict) or segment not in current:
                raise HarnessError(f"scenario reference {value!r} was unavailable")
            current = current[segment]
        return current
    if isinstance(value, list):
        return [resolve(item, context) for item in value]
    if isinstance(value, dict):
        return {key: resolve(item, context) for key, item in value.items()}
    return value


def inspect_assertions(
    scenario: dict[str, Any],
    relays: dict[str, Relay],
    rows: list[dict[str, Any]],
    context: dict[str, Any],
    artifacts: Path,
    assertion_offset: int = 0,
) -> dict[str, Any]:
    captures = result_captures(rows, scenario.get("captures", {}))
    context = {**context, **captures}
    for number, assertion in enumerate(scenario.get("assertions", []), start=1):
        if not isinstance(assertion, dict):
            raise HarnessError(f"scenario assertion {number} was not an object")
        relay_name = assertion.get("relay")
        if relay_name not in relays:
            raise HarnessError(f"scenario assertion {number} named unavailable relay {relay_name!r}")
        event_filter = resolve(assertion.get("filter", {}), context)
        if not isinstance(event_filter, dict):
            raise HarnessError(f"scenario assertion {number} filter was not an object")
        try:
            filter_bytes = len(
                json.dumps(event_filter, ensure_ascii=True, separators=(",", ":")).encode("utf-8")
            )
        except (TypeError, ValueError) as error:
            raise HarnessError(f"scenario assertion {number} resolved filter was not JSON data") from error
        if filter_bytes > MAX_FILTER_BYTES:
            raise HarnessError(
                f"scenario assertion {number} resolved filter exceeded {MAX_FILTER_BYTES} bytes"
            )
        inspection = inspect_until_eose(
            relays[relay_name].url,
            f"assert-{assertion_offset + number}",
            event_filter,
            INSPECTION_SECONDS,
        )
        inspection_path = artifacts / "inspections"
        inspection_path.mkdir(exist_ok=True)
        rendered = json.dumps(
            inspection.as_json(), ensure_ascii=True, separators=(",", ":"), sort_keys=True
        ) + "\n"
        if len(rendered.encode("utf-8")) > MAX_ARTIFACT_FILE_BYTES:
            raise HarnessError(f"inspection {number} exceeded the retained file byte bound")
        (inspection_path / f"{assertion_offset + number:02d}.json").write_text(rendered, encoding="utf-8")
        expected_present = bool(assertion.get("present", True))
        if expected_present:
            if assertion.get("collection") == "relay-state":
                required_kinds = assertion["required_kinds"]
                if len(inspection.events) != len(required_kinds):
                    raise HarnessError(f"assertion {number} expected one record for each relay state kind")
                kinds = [event.get("kind") for event in inspection.events]
                if sorted(kinds) != required_kinds:
                    raise HarnessError(f"assertion {number} did not return exactly the required relay state kinds")
                authors = {event.get("pubkey") for event in inspection.events}
                if len(authors) != 1 or not all(isinstance(author, str) for author in authors):
                    raise HarnessError(f"assertion {number} state records did not share one relay author")
                app_authors = {context.get("alice", {}).get("author"), context.get("bob", {}).get("author")}
                if authors & app_authors:
                    raise HarnessError(f"assertion {number} state records used an application author")
                for event in inspection.events:
                    if ["d", context["group_id"]] not in event.get("tags", []):
                        raise HarnessError(f"assertion {number} state record omitted the exact group tag")
                continue
            if len(inspection.events) != 1:
                raise HarnessError(f"assertion {number} expected one event, found {len(inspection.events)}")
            expected = resolve(assertion.get("event", {}), context)
            if not isinstance(expected, dict):
                raise HarnessError(f"scenario assertion {number} event was not an object")
            assert_event(inspection.events[0], expected)
        elif inspection.events:
            raise HarnessError(f"assertion {number} expected no unauthorized effect, found {len(inspection.events)} events")
    return context


def require_stopped(label: str, facts: dict[str, Any]) -> None:
    """Turn process-group teardown facts into an attributable harness failure."""

    if not facts["group_gone"]:
        raise HarnessError(f"{label} process group survived teardown")
    if facts["teardown_error"]:
        raise HarnessError(f"{label} process-group teardown reported an operating-system error")
    if not facts["output_threads_joined"]:
        raise HarnessError(f"{label} bounded-output drain did not finish after teardown")


def cleanup_before_retention(scratch: Path, artifacts: Path, sentinels: tuple[str, ...]) -> None:
    """Erase all transient state, then scan every retained artifact on every outcome."""

    cleanup_error: BaseException | None = None
    scan_error: BaseException | None = None
    try:
        remove_scratch(scratch)
    except BaseException as error:
        cleanup_error = error
    try:
        scan_secret_absence(artifacts, sentinels)
    except BaseException as error:
        scan_error = error
    if cleanup_error is not None and scan_error is not None:
        raise HarnessError("transient cleanup and retained-artifact scan both failed") from cleanup_error
    if cleanup_error is not None:
        raise HarnessError("transient scratch cleanup failed") from cleanup_error
    if scan_error is not None:
        raise scan_error


def command_for(arguments: argparse.Namespace, commands: Path, streamed: bool = False) -> list[str]:
    if arguments.app_command:
        if streamed:
            raise HarnessError("staged scenarios require the ordinary REPL command")
        return [part.format(commands=str(commands)) for part in arguments.app_command]
    command = [
        "cargo",
        "run",
        "--quiet",
        "--locked",
        "--manifest-path",
        "examples/simple-groups/Cargo.toml",
        "--",
        "--jsonl",
    ]
    if not streamed:
        command[command.index("--jsonl"):command.index("--jsonl")] = ["--script", str(commands)]
    return command


def require_binary(value: str, label: str) -> Path:
    path = Path(value)
    if path.is_file() and os.access(path, os.X_OK):
        return path.resolve()
    located = shutil.which(value)
    if located:
        return Path(located).resolve()
    raise HarnessError(f"{label} executable is unavailable: {value}")


def ordinary_fixture_version(binary: Path) -> str:
    """Accept only the nostr-rs-relay release whose generated config we use."""

    try:
        completed = subprocess.run(
            [str(binary), "--version"],
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=5,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise HarnessError("ordinary relay version command was unavailable") from error
    if len(completed.stdout) > 8_192 or len(completed.stderr) > 8_192:
        raise HarnessError("ordinary relay version output exceeded its bound")
    version = completed.stdout.decode("utf-8", "replace").strip()
    if completed.returncode != 0 or version != "nostr-rs-relay 0.8.12":
        raise HarnessError(
            f"ordinary relay must be nostr-rs-relay 0.8.12, got {version!r}"
        )
    return version


def binary_sha256(binary: Path) -> str:
    """Return a bounded-memory identity for a selected local fixture."""

    if binary.stat().st_size > MAX_BINARY_BYTES:
        raise HarnessError(f"relay executable exceeded {MAX_BINARY_BYTES} bytes")
    digest = hashlib.sha256()
    with binary.open("rb") as source:
        while chunk := source.read(1_048_576):
            digest.update(chunk)
    return digest.hexdigest()

def write_result(
    artifacts: Path,
    scenario: str,
    fixture_hashes: dict[str, str],
    ordinary_version: str,
    result_rows: int | None,
    outcome: str,
) -> None:
    """Write a compact, secret-free run result before the final retention scan."""

    value: dict[str, Any] = {
        "fixtures": {
            "group": {"sha256": fixture_hashes["group"], "source": "explicit-path"},
            "state": {
                "sha256": fixture_hashes["state"],
                "source": "explicit-path",
                "version": ordinary_version,
            },
        },
        "outcome": outcome,
        "scenario": scenario,
        "schema": "fava-simple-groups-live-proof-v1",
    }
    if result_rows is not None:
        value["app_result_rows"] = result_rows
    rendered = json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True) + "\n"
    (artifacts / "result.json").write_text(rendered, encoding="utf-8")


def run(arguments: argparse.Namespace) -> int:
    scenario = load_scenario(arguments.scenario)
    if scenario.get("status") != "executable":
        reason = scenario.get("blocked_by", "scenario is not executable")
        print(json_line({"action": "scenario-unavailable", "reason": reason, "scenario": arguments.scenario}), end="")
        return 2
    secret_sentinels = validate_executable_scenario(scenario)
    arguments.artifacts = arguments.artifacts.resolve()
    if not GROUP_ID_PATTERN.fullmatch(arguments.group_id):
        raise HarnessError("group id must be lowercase ASCII and at most 63 characters")
    if arguments.artifacts.exists():
        raise HarnessError(f"artifact directory must be new: {arguments.artifacts}")
    nip29_binary = require_binary(arguments.nip29_bin, "NIP-29 relay")
    ordinary_binary = require_binary(arguments.ordinary_bin, "ordinary relay")
    ordinary_version = ordinary_fixture_version(ordinary_binary)
    arguments.artifacts.mkdir(parents=True, mode=0o700)
    log = RunLog(arguments.artifacts / "run.jsonl")
    scratch = new_scratch(LIVE)
    scratch_tmp = scratch / "tmp"
    scratch_tmp.mkdir(mode=0o700)
    environment = {**os.environ, "TMPDIR": str(scratch_tmp)}
    relays: list[Relay] = []
    run_error: BaseException | None = None
    teardown_errors: list[str] = []
    finalization_error: BaseException | None = None
    scan_error: BaseException | None = None
    result_rows: int | None = None
    fixture_hashes: dict[str, str] = {}
    try:
        fixture_hashes = {
            "group": binary_sha256(nip29_binary),
            "state": binary_sha256(ordinary_binary),
        }
        log.record(
            "relay-fixture-selected",
            relay="group",
            sha256=fixture_hashes["group"],
            source="explicit-path",
        )
        log.record(
            "relay-fixture-selected",
            relay="state",
            sha256=fixture_hashes["state"],
            source="explicit-path",
            version=ordinary_version,
        )
        group = start_croissant(nip29_binary, scratch, environment)
        relays.append(group)
        state = start_ordinary(ordinary_binary, scratch, environment)
        relays.append(state)
        for relay in relays:
            wait_ready(relay)
            log.record("relay-ready", relay=relay.label, url=relay.url)
        commands_source = LIVE / scenario["command_file"]
        if not commands_source.is_file():
            raise HarnessError(f"scenario command file was absent: {commands_source}")
        commands = scratch / "commands.txt"
        materialize_commands(
            commands_source,
            commands,
            {"GROUP_RELAY": group.url, "STATE_RELAY": state.url, "GROUP_ID": arguments.group_id},
        )
        stage_context: dict[str, Any] = {"group_id": arguments.group_id, "group_relay": group.url}
        stages = scenario.get("stages", [])
        stage_index = 0
        assertion_offset = 0
        streamed_rows: list[dict[str, Any]] = []

        def inspect_stage(row: dict[str, Any], process: subprocess.Popen[bytes]) -> None:
            nonlocal assertion_offset, stage_index, stage_context
            streamed_rows.append(row)
            if stage_index >= len(stages):
                return
            stage = stages[stage_index]
            if len(streamed_rows) != stage["after_line"]:
                return
            stage_context = inspect_assertions(
                stage,
                {relay.label: relay for relay in relays},
                streamed_rows,
                stage_context,
                arguments.artifacts,
                assertion_offset,
            )
            assertion_offset += len(stage["assertions"])
            stage_index += 1

        status, stdout, stderr = run_app(
            command_for(arguments, commands, streamed=bool(stages)),
            arguments.artifacts,
            environment,
            inspect_stage if stages else None,
            commands.read_text(encoding="utf-8").splitlines() if stages else None,
        )
        expected_exit = scenario.get("app_exit", "zero")
        if (expected_exit == "zero" and status != 0) or (expected_exit == "nonzero" and status == 0):
            raise HarnessError(f"application exit {status} did not satisfy {expected_exit!r}")
        rows = parse_jsonl(stdout)
        (arguments.artifacts / "app-results.jsonl").write_text(
            "".join(json_line(row) for row in rows), encoding="utf-8"
        )
        result_rows = len(rows)
        if stages:
            captures = {key: value for key, value in stage_context.items() if key not in {"group_id", "group_relay"}}
            rendered_captures = json.dumps(
                {"captures": captures, "schema": "fava-simple-groups-live-captures-v1"},
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            ) + "\n"
            if len(rendered_captures.encode("utf-8")) > MAX_ARTIFACT_FILE_BYTES:
                raise HarnessError("application captures exceeded the retained file byte bound")
            (arguments.artifacts / "app-captures.json").write_text(rendered_captures, encoding="utf-8")
        log.record("application-finished", exit=status, result_rows=result_rows)
        if stages:
            if stage_index != len(stages):
                raise HarnessError("application ended before every declared assertion stage")
            if rows != streamed_rows:
                raise HarnessError("streamed REPL evidence differed from retained JSONL")
        else:
            inspect_assertions(
                scenario,
                {relay.label: relay for relay in relays},
                rows,
                {"group_id": arguments.group_id},
                arguments.artifacts,
            )
    except BaseException as error:
        run_error = error
    finally:
        for relay in reversed(relays):
            try:
                facts = relay.process.stop()
                log.record("relay-stopped", relay=relay.label, **facts)
                require_stopped(f"{relay.label} relay", facts)
            except BaseException as error:
                teardown_errors.append(str(error))
        try:
            if fixture_hashes:
                outcome = "passed" if run_error is None and not teardown_errors else "failed"
                write_result(
                    arguments.artifacts,
                    arguments.scenario,
                    fixture_hashes,
                    ordinary_version,
                    result_rows,
                    outcome,
                )
            log.record("retained-artifact-scan", scenario=arguments.scenario)
        except BaseException as error:
            finalization_error = error
        try:
            cleanup_before_retention(scratch, arguments.artifacts, secret_sentinels)
        except BaseException as error:
            scan_error = error
    errors: list[BaseException] = []
    if run_error is not None:
        errors.append(run_error)
    errors.extend(HarnessError(message) for message in teardown_errors)
    if finalization_error is not None:
        errors.append(HarnessError("retained-artifact finalization failed"))
    if scan_error is not None:
        errors.append(scan_error)
    if errors:
        if len(errors) == 1:
            raise errors[0]
        raise HarnessError("harness failure plus " + "; ".join(str(error) for error in errors)) from errors[0]
    return 0


def discover(_: argparse.Namespace) -> int:
    candidates = {
        "nip29": [
            os.environ.get("SIMPLE_GROUPS_NIP29_RELAY"),
            shutil.which("croissant"),
            "/Users/pablofernandez/Work/croissant/croissant",
        ],
        "ordinary": [os.environ.get("SIMPLE_GROUPS_ORDINARY_RELAY"), shutil.which("nostr-rs-relay")],
    }
    for role, values in candidates.items():
        available = []
        for value in values:
            if value and Path(value).is_file() and os.access(value, os.X_OK) and value not in available:
                available.append(value)
        print(json_line({"available": available, "role": role}), end="")
    return 0


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    subcommands = result.add_subparsers(dest="subcommand", required=True)
    subcommands.add_parser("discover", help="report usable local relay executables")
    run_parser = subcommands.add_parser("run", help="run one scenario against disposable real relays")
    run_parser.add_argument("--scenario", default="smoke-create-content")
    run_parser.add_argument("--nip29-bin", required=True, help="Croissant-compatible NIP-29 relay executable")
    run_parser.add_argument("--ordinary-bin", required=True, help="nostr-rs-relay 0.8.12 executable")
    run_parser.add_argument("--artifacts", required=True, type=Path, help="new output directory")
    run_parser.add_argument("--group-id", default="fava-e2e-group")
    run_parser.add_argument(
        "--app-command",
        nargs="+",
        help="argv template; each argument may contain {commands}, never executed by a shell",
    )
    return result


def main() -> int:
    arguments = parser().parse_args()
    try:
        return discover(arguments) if arguments.subcommand == "discover" else run(arguments)
    except HarnessError as error:
        print(json_line({"action": "harness-failed", "reason": str(error)}), end="", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
