#!/usr/bin/env python3
"""Bounded live proof for the NIP-42 relay-authentication application.

Runs the real `relay-auth` binary's ordinary REPL command lines against four
disposable relays: `nostr-rs-relay` (a genuine third-party relay that gives a
real `AUTH` challenge but does not enforce it), and three instances of the
harness-owned enforcing relay (`accept`, `reject`, `accept-refuse` -- see
`examples/crates/e2e-support/live/nip42_relay.py`). It then independently
inspects each relay over its own fresh WebSocket connection -- never reusing
the application's connection or trusting its self-report -- to confirm what
actually happened on the wire.
"""

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

from harness_process import (  # noqa: E402
    require_stopped,
    run_app,
    start_authenticating,
    start_nip42_enforcing,
    wait_ready,
    wait_ready_enforcing,
)
from harness_safety import HarnessError, check_retained_artifacts  # noqa: E402
from relay_inspection import WebSocket, assert_event, inspect_until_eose  # noqa: E402

ALICE_SECRET = "0000000000000000000000000000000000000000000000000000000000000001"
ALICE_PUBKEY = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
BOB_SECRET = "0000000000000000000000000000000000000000000000000000000000000002"
CAROL_SECRET = "0000000000000000000000000000000000000000000000000000000000000003"
# `account import` also selects, so the last import (carol) is the current
# account when the scenario's first, implicit-author "publish public" runs.
CAROL_PUBKEY = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
INSPECTION_TIMEOUT = 10.0


def parse() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--relay", type=Path, default=Path.home() / ".cargo/bin/nostr-rs-relay")
    parser.add_argument(
        "--verify-bin",
        type=Path,
        default=REPO / "examples/crates/e2e-support/target/debug/nip01_wire",
    )
    parser.add_argument("--evidence", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse()
    verify_bin = args.verify_bin.resolve()
    if not verify_bin.exists():
        raise HarnessError(
            f"nip01_wire helper not found at {verify_bin}; build it with "
            "`cargo build -p e2e-support --bin nip01_wire`"
        )
    evidence = args.evidence or Path(tempfile.mkdtemp(prefix="fava-relay-auth-live-"))
    evidence.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ)

    nostr = start_authenticating(args.relay.resolve(), evidence, environment)
    accept = start_nip42_enforcing(verify_bin, evidence, environment, "accept", "accept")
    reject = start_nip42_enforcing(verify_bin, evidence, environment, "reject", "reject")
    refuse = start_nip42_enforcing(verify_bin, evidence, environment, "accept-refuse", "refuse")
    rows: list[dict[str, Any]] = []
    try:
        wait_ready(nostr)
        wait_ready_enforcing(accept)
        wait_ready_enforcing(reject)
        wait_ready_enforcing(refuse)

        commands = (
            (REPO / "examples/relay-auth/scenarios/live-nip42.txt")
            .read_text(encoding="utf-8")
            .replace("ws://127.0.0.1:18090", nostr.url)
            .replace("ws://127.0.0.1:18091", accept.url)
            .replace("ws://127.0.0.1:18092", reject.url)
            .replace("ws://127.0.0.1:18093", refuse.url)
        )
        lines = [line for line in commands.splitlines() if line and not line.startswith("#")]
        status, stdout_path, stderr_path = run_app(
            [str(args.app.resolve()), "--jsonl"],
            evidence,
            environment,
            lambda row, _process: rows.append(row),
            lines,
        )
        if status != 0:
            raise HarnessError(f"relay-auth application exited with {status}")
        require_ok_rows(rows, len(lines))
        captured = capture_rows(rows)
        auth_states = auth_state_rows(rows)

        # Independent inspection: the harness never reuses the application's
        # connection. Every claim below is re-derived from a fresh socket.
        inspect_public_event(nostr.url, captured["pub-event"], CAROL_PUBKEY, verify_bin)
        inspect_public_event(nostr.url, captured["defer-event"], ALICE_PUBKEY, verify_bin)
        inspect_authenticated_event(
            accept.url, ALICE_SECRET, ALICE_PUBKEY, captured["accept-event"], ALICE_PUBKEY, verify_bin
        )
        inspect_authenticated_event(
            accept.url, ALICE_SECRET, ALICE_PUBKEY, captured["cross-event"],
            "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5", verify_bin,
        )
        inspect_rejected_auth(reject.url, BOB_SECRET, "error:", verify_bin)
        inspect_rejected_auth(refuse.url, ALICE_SECRET, "restricted:", verify_bin)
        require_auth_states(auth_states)

        (evidence / "run.jsonl").write_text(
            "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8"
        )
        result = {
            "app_stdout": str(stdout_path.relative_to(evidence)),
            "app_stderr": str(stderr_path.relative_to(evidence)),
            "fixtures": {
                "app": "relay-auth 0.1.0",
                "app_sha256": sha256(args.app.resolve()),
                "authenticating_relay": relay_version(args.relay.resolve()),
                "authenticating_relay_sha256": sha256(args.relay.resolve()),
                "enforcing_relay": "examples/crates/e2e-support/live/nip42_relay.py (harness-owned)",
                "nip01_wire_sha256": sha256(verify_bin),
                "scenario_sha256": sha256(REPO / "examples/relay-auth/scenarios/live-nip42.txt"),
            },
            "assertions": {
                "public_events_independently_read_and_signature_verified": True,
                "authenticated_accept_events_independently_read_and_signature_verified": True,
                "reject_and_refuse_wire_replies_independently_reproduced": True,
                "every_authentication_state_but_challenge_received_observed": True,
                "auth_denied_publications_completed_command_execution": True,
            },
            "captures": captured,
            "auth_states": auth_states,
            "relays": {
                "nostr": nostr.url,
                "accept": accept.url,
                "reject": reject.url,
                "refuse": refuse.url,
            },
        }
        (evidence / "result.json").write_text(
            json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(json.dumps({"evidence": str(evidence), "result": "passed"}, sort_keys=True))
        return 0
    finally:
        for label, relay in (("nostr", nostr), ("accept", accept), ("reject", reject), ("refuse", refuse)):
            teardown = relay.process.stop()
            require_stopped(f"{label} relay", teardown)
            (evidence / f"teardown-{label}.json").write_text(
                json.dumps(teardown, indent=2, sort_keys=True) + "\n", encoding="utf-8"
            )
        shutil.rmtree(evidence / "relays" / "authenticating" / "data", ignore_errors=True)
        require_no_secrets(evidence)
        write_manifest(evidence)
        check_retained_artifacts(evidence)


def require_no_secrets(evidence: Path) -> None:
    """The scenario's own imported nsecs must never appear in retained evidence.

    Not a redaction/refusal policy inside the application -- issue 0053
    deliberately removed those from every E2E testing surface, and this app
    carries none. This is a bounded evidence check: given a scenario that has
    no reason to echo a private key, confirm it never accidentally did.
    """

    secrets = [ALICE_SECRET, BOB_SECRET, CAROL_SECRET]
    for path in evidence.rglob("*"):
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for secret in secrets:
            if secret in text:
                raise HarnessError(f"retained evidence file {path} contains an imported secret key")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_manifest(evidence: Path) -> None:
    files: dict[str, dict[str, Any]] = {}
    for path in sorted(evidence.rglob("*")):
        if path.is_file() and path.name != "manifest.json":
            files[str(path.relative_to(evidence))] = {"bytes": path.stat().st_size, "sha256": sha256(path)}
    (evidence / "manifest.json").write_text(
        json.dumps({"files": files, "format": 1, "scenario": "live-nip42.txt"}, indent=2, sort_keys=True)
        + "\n",
        encoding="utf-8",
    )


def relay_version(binary: Path) -> str:
    completed = subprocess.run(
        [str(binary), "--version"], capture_output=True, text=True, timeout=5, check=False
    )
    value = (completed.stdout or completed.stderr).strip()
    if completed.returncode != 0 or not value or len(value.encode("utf-8")) > 256:
        raise HarnessError("authenticating relay did not report one bounded version")
    if value != "nostr-rs-relay 0.8.12":
        raise HarnessError(f"authenticating relay must be nostr-rs-relay 0.8.12, got {value!r}")
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
        name, value = fields.get("capture"), fields.get("value")
        if not isinstance(name, str) or not isinstance(value, str):
            raise HarnessError(f"capture row omitted its scalar authority: {row}")
        if name in captures:
            raise HarnessError(f"capture {name!r} appeared more than once")
        captures[name] = value
    required = {
        "pub-event", "accept-event", "failed-write", "decline-write", "reject-write",
        "refuse-write", "cross-event", "defer-event", "pending-relay", "pending-connection",
    }
    if not required.issubset(captures):
        raise HarnessError(f"missing app captures: expected at least {required}, got {set(captures)}")
    return captures


def auth_state_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    states = [row["fields"] for row in rows if row.get("kind") == "auth-state"]
    if len(states) != 7:
        raise HarnessError(f"expected 7 auth-state checks, got {len(states)}")
    return states


def require_auth_states(states: list[dict[str, Any]]) -> None:
    expected = [
        ("accept", "as:alice", "authenticated"),
        ("refuse", "as:dave", "unanswerable"),
        ("reject", "as:carol", "declined"),
        ("reject", "as:bob", "refused"),
        ("refuse", "as:alice", "refused"),
        ("nostr", "as:alice", "requested"),
        ("nostr", "as:alice", "authenticating"),
    ]
    actual = [(state["relay"], state["access"], state["state"]) for state in states]
    if actual != expected:
        raise HarnessError(f"authentication state sequence differed: {actual} != {expected}")
    # Two relays refuse the same proof for different reasons, and the state
    # name no longer separates them -- a refusal is a refusal. The relay's own
    # words are what carry the difference, so they are what gets asserted.
    reasons = [
        (3, "error:", "reject answers a valid AUTH with a plain error"),
        (4, "restricted:", "accept-refuse answers a valid AUTH with restricted"),
    ]
    for index, prefix, why in reasons:
        message = states[index].get("message", "")
        if not message.startswith(prefix):
            raise HarnessError(f"{why}: expected {prefix!r}, got {message!r}")


def sign_auth(verify_bin: Path, secret_key: str, relay: str, challenge: str) -> dict[str, Any]:
    request = json.dumps({"secret_key": secret_key, "relay": relay, "challenge": challenge})
    completed = subprocess.run(
        [str(verify_bin), "sign-auth"], input=request, capture_output=True, text=True, timeout=5,
    )
    if completed.returncode != 0:
        raise HarnessError(f"nip01_wire sign-auth failed: {completed.stderr}")
    return json.loads(completed.stdout)


def verify_event(verify_bin: Path, event: dict[str, Any]) -> None:
    completed = subprocess.run(
        [str(verify_bin), "verify"], input=json.dumps(event), capture_output=True, text=True, timeout=5,
    )
    if completed.returncode != 0:
        raise HarnessError(f"independently fetched event failed real signature verification: {completed.stderr}")


def inspect_public_event(url: str, event_id: str, expected_pubkey: str, verify_bin: Path) -> None:
    inspection = inspect_until_eose(url, "relay-auth-public", {"ids": [event_id]}, INSPECTION_TIMEOUT)
    if len(inspection.events) != 1:
        raise HarnessError(f"independent REQ for {event_id} did not return exactly one event")
    assert_event(inspection.events[0], {"id": event_id, "pubkey": expected_pubkey})
    verify_event(verify_bin, inspection.events[0])


def inspect_authenticated_event(
    url: str, secret_key: str, connection_pubkey: str, event_id: str, expected_author: str, verify_bin: Path
) -> None:
    """Independently authenticate, over a fresh connection, then read one event."""

    socket_client = WebSocket(url, INSPECTION_TIMEOUT)
    try:
        challenge_frame = socket_client.receive_json()
        if challenge_frame[:1] != ["AUTH"]:
            raise HarnessError(f"enforcing relay did not open with AUTH: {challenge_frame}")
        challenge = challenge_frame[1]
        auth_event = sign_auth(verify_bin, secret_key, url, challenge)
        verify_event(verify_bin, auth_event)
        socket_client.send_json(["AUTH", auth_event])
        reply = socket_client.receive_json()
        if not (reply[:1] == ["OK"] and reply[2] is True):
            raise HarnessError(f"independent inspection AUTH as {connection_pubkey} was refused: {reply}")
        socket_client.send_json(["REQ", "relay-auth-inspect", {"ids": [event_id]}])
        events: list[dict[str, Any]] = []
        while True:
            frame = socket_client.receive_json()
            if frame[:2] == ["EVENT", "relay-auth-inspect"]:
                events.append(frame[2])
            elif frame[:2] == ["EOSE", "relay-auth-inspect"]:
                break
            else:
                raise HarnessError(f"unexpected frame during independent REQ: {frame}")
        if len(events) != 1:
            raise HarnessError(f"independent authenticated REQ for {event_id} returned {len(events)} events")
        assert_event(events[0], {"id": event_id, "pubkey": expected_author})
        verify_event(verify_bin, events[0])
    finally:
        socket_client.close()


def inspect_rejected_auth(url: str, secret_key: str, expected_prefix: str, verify_bin: Path) -> None:
    """Independently reproduce a relay's refusal of a valid AUTH on a fresh connection."""

    socket_client = WebSocket(url, INSPECTION_TIMEOUT)
    try:
        challenge_frame = socket_client.receive_json()
        if challenge_frame[:1] != ["AUTH"]:
            raise HarnessError(f"enforcing relay did not open with AUTH: {challenge_frame}")
        challenge = challenge_frame[1]
        auth_event = sign_auth(verify_bin, secret_key, url, challenge)
        verify_event(verify_bin, auth_event)
        socket_client.send_json(["AUTH", auth_event])
        reply = socket_client.receive_json()
        if not (reply[:1] == ["OK"] and reply[2] is False and reply[3].startswith(expected_prefix)):
            raise HarnessError(f"expected OK false {expected_prefix!r} for a valid AUTH, got {reply}")
        socket_client.send_json(["REQ", "relay-auth-inspect-denied", {"limit": 1}])
        closed = socket_client.receive_json()
        if not (closed[:1] == ["CLOSED"] and "auth-required" in closed[2]):
            raise HarnessError(f"a still-unauthenticated connection must keep refusing reads: {closed}")
    finally:
        socket_client.close()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (HarnessError, OSError, ValueError, KeyError) as error:
        print(f"harness failed: {error}", file=sys.stderr)
        raise SystemExit(1)
