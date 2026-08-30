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
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

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
    HarnessError,
    new_scratch,
    read_bounded_text,
    remove_scratch,
    check_retained_artifacts,
)
from relay_inspection import InspectionError, assert_event, inspect_until_eose
from harness_process import (
    ManagedProcess,
    Relay,
    materialize_commands,
    require_stopped,
    run_app,
    start_croissant,
    start_ordinary,
    wait_ready,
)
from scenario_contract import resolve, result_captures, validate_executable_scenario

LIVE = Path(__file__).resolve().parent
SCENARIOS = LIVE / "scenarios"
INSPECTION_SECONDS = 10.0
GROUP_ID_PATTERN = __import__("re").compile(r"^[a-z0-9][a-z0-9-]{0,62}$")

# Last-resort fallback when neither --nip29-bin nor FAVA_CROISSANT_BIN names a
# Croissant checkout. See FAVA_CROISSANT_BIN in README.md.
DEFAULT_CROISSANT_BINARY = "/Users/pablofernandez/Work/croissant/croissant"


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


def cleanup_before_retention(scratch: Path, artifacts: Path) -> None:
    """Erase all transient state and validate retained artifact bounds."""

    cleanup_error: BaseException | None = None
    scan_error: BaseException | None = None
    try:
        remove_scratch(scratch)
    except BaseException as error:
        cleanup_error = error
    try:
        check_retained_artifacts(artifacts)
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
    """Write a compact run result before the final retention scan."""

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
    validate_executable_scenario(scenario)
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
            cleanup_before_retention(scratch, arguments.artifacts)
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
            os.environ.get("FAVA_CROISSANT_BIN", DEFAULT_CROISSANT_BINARY),
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
        if arguments.subcommand == "discover":
            return discover(arguments)
        return run(arguments)
    except HarnessError as error:
        print(json_line({"action": "harness-failed", "reason": str(error)}), end="", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
