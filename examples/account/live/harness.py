#!/usr/bin/env python3
"""Bounded live proof for the account-reactive application."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SHARED = REPO / "examples" / "crates" / "e2e-support" / "live"
sys.path.insert(0, str(SHARED))

from harness_process import require_stopped, run_app, start_ordinary, wait_ready  # noqa: E402
from harness_safety import HarnessError, check_retained_artifacts  # noqa: E402
from relay_inspection import assert_event, inspect_until_eose  # noqa: E402


def parse() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--relay", type=Path, default=Path.home() / ".cargo/bin/nostr-rs-relay")
    parser.add_argument("--evidence", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse()
    evidence = args.evidence or Path(tempfile.mkdtemp(prefix="fava-account-live-"))
    evidence.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ)
    relay = start_ordinary(args.relay.resolve(), evidence, environment)
    rows: list[dict[str, Any]] = []
    try:
        wait_ready(relay)
        commands = (REPO / "examples/account/scenarios/account-reactivity.txt").read_text(
            encoding="utf-8"
        ).replace("ws://127.0.0.1:18080", relay.url)
        lines = [line for line in commands.splitlines() if line and not line.startswith("#")]
        status, stdout_path, stderr_path = run_app(
            [str(args.app.resolve()), "--jsonl"],
            evidence,
            environment,
            lambda row, _process: rows.append(row),
            lines,
        )
        if status != 0:
            raise HarnessError(f"account application exited with {status}")
        require_ok_rows(rows, len(lines))
        captured = capture_rows(rows)
        alice = inspect_until_eose(
            relay.url, "account-alice", {"ids": [captured["alice-event"]]}, 10.0
        )
        bob = inspect_until_eose(
            relay.url, "account-bob", {"ids": [captured["bob-event"]]}, 10.0
        )
        if len(alice.events) != 1 or len(bob.events) != 1:
            raise HarnessError("independent event-id REQ did not return one exact event")
        assert_event(
            alice.events[0],
            {
                "id": captured["alice-event"],
                "pubkey": captured["alice-pub"],
                "kind": 1,
                "content": "alice event",
            },
        )
        assert_event(
            bob.events[0],
            {
                "id": captured["bob-event"],
                "pubkey": captured["bob-pub"],
                "kind": 1,
                "content": "bob event",
            },
        )
        require_reactive_transitions(rows, captured)
        require_route_attribution(rows)
        (evidence / "run.jsonl").write_text(
            "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8"
        )
        (evidence / "relay-alice.json").write_text(
            json.dumps(alice.as_json(), indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        (evidence / "relay-bob.json").write_text(
            json.dumps(bob.as_json(), indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        result = {
            "app_stderr": str(stderr_path.relative_to(evidence)),
            "app_stdout": str(stdout_path.relative_to(evidence)),
            "fixtures": {
                "app": "account 0.1.0",
                "app_sha256": sha256(args.app.resolve()),
                "relay": relay_version(args.relay.resolve()),
                "relay_sha256": sha256(args.relay.resolve()),
                "scenario_sha256": sha256(
                    REPO / "examples/account/scenarios/account-reactivity.txt"
                ),
            },
            "assertions": {
                "accepted_authors_exact": True,
                "independent_req_eose": True,
                "one_observation_id": True,
                "reactive_account_transitions": True,
                "route_attribution": True,
            },
            "captures": captured,
            "relay": relay.url,
        }
        (evidence / "result.json").write_text(
            json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(json.dumps({"evidence": str(evidence), "result": "passed"}, sort_keys=True))
        return 0
    finally:
        teardown = relay.process.stop()
        require_stopped("ordinary relay", teardown)
        (evidence / "teardown.json").write_text(
            json.dumps(teardown, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        shutil.rmtree(evidence / "relays/ordinary/data", ignore_errors=True)
        write_manifest(evidence)
        check_retained_artifacts(evidence)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_manifest(evidence: Path) -> None:
    files: dict[str, dict[str, Any]] = {}
    for path in sorted(evidence.rglob("*")):
        if path.is_file() and path.name != "manifest.json":
            files[str(path.relative_to(evidence))] = {
                "bytes": path.stat().st_size,
                "sha256": sha256(path),
            }
    (evidence / "manifest.json").write_text(
        json.dumps(
            {"files": files, "format": 1, "scenario": "account-reactivity.txt"},
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def relay_version(binary: Path) -> str:
    completed = subprocess.run(
        [str(binary), "--version"], capture_output=True, text=True, timeout=5, check=False
    )
    value = (completed.stdout or completed.stderr).strip()
    if completed.returncode != 0 or not value or len(value.encode("utf-8")) > 256:
        raise HarnessError("ordinary relay did not report one bounded version")
    if value != "nostr-rs-relay 0.8.12":
        raise HarnessError(
            f"ordinary relay version must be nostr-rs-relay 0.8.12, got {value!r}"
        )
    return value


def require_ok_rows(rows: list[dict[str, Any]], expected: int) -> None:
    if len(rows) != expected:
        raise HarnessError(f"expected {expected} typed rows, got {len(rows)}")
    failures = [row for row in rows if row.get("status") != "ok"]
    if failures:
        raise HarnessError(f"application reported non-ok rows: {failures}")


def capture_rows(rows: list[dict[str, Any]]) -> dict[str, str]:
    captures: dict[str, str] = {}
    for row in rows:
        if row.get("kind") != "capture-set":
            continue
        fields = row.get("fields", {})
        name = fields.get("capture")
        value = fields.get("value")
        if not isinstance(name, str) or not isinstance(value, str):
            raise HarnessError(f"capture row omitted its scalar authority: {row}")
        if name in captures:
            raise HarnessError(f"capture {name!r} appeared more than once")
        captures[name] = value
    required = {"alice-pub", "bob-pub", "alice-event", "bob-event"}
    if set(captures) != required:
        raise HarnessError(f"missing app captures: expected {required}, got {set(captures)}")
    return captures


def require_route_attribution(rows: list[dict[str, Any]]) -> None:
    routes = [row for row in rows if row.get("kind") == "routes"]
    if len(routes) != 2:
        raise HarnessError(f"expected active and cleared route rows, got {len(routes)}")
    active = routes[0]["fields"]
    cleared = routes[1]["fields"]
    if not active.get("demand_relays") or set(active.get("demand_observations", [])) != {1}:
        raise HarnessError(f"active route did not attribute public observation 1: {active}")
    if cleared.get("demand_relays") or cleared.get("wire_subscriptions"):
        raise HarnessError(f"cleared selection retained route work: {cleared}")


def require_reactive_transitions(rows: list[dict[str, Any]], captured: dict[str, str]) -> None:
    snapshots = [row for row in rows if row.get("kind") == "query-snapshot"]
    if len(snapshots) < 6:
        raise HarnessError("scenario did not expose every reactive query transition")
    ids = {row["fields"]["observation_id"] for row in snapshots}
    if len(ids) != 1:
        raise HarnessError(f"query handle identity changed: {ids}")
    authors = [row["fields"].get("authors", []) for row in snapshots]
    expected = [
        [captured["alice-pub"]],
        [],
        [captured["bob-pub"]],
        [captured["alice-pub"]],
        [captured["bob-pub"]],
        [],
    ]
    if authors[-6:] != expected:
        raise HarnessError(f"reactive author transitions differed: {authors[-6:]} != {expected}")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (HarnessError, OSError, ValueError, KeyError) as error:
        print(f"harness failed: {error}", file=sys.stderr)
        raise SystemExit(1)
