"""Contract tests for the independent simple-groups live harness."""

from __future__ import annotations

import base64
import io
import hashlib
import json
import os
import socket
import sys
import threading
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
from contextlib import redirect_stdout


LIVE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIVE))

from harness import (
    HarnessError,
    ManagedProcess,
    cleanup_before_retention,
    inspect_assertions,
    json_line,
    materialize_commands,
    result_captures,
    resolve,
    run,
    validate_executable_scenario,
)
from harness_safety import MAX_ARTIFACT_FILE_BYTES, MAX_FILTER_BYTES, check_retained_artifacts
from relay_inspection import InspectionError, assert_event, inspect_until_eose


class ScriptedRelay:
    """One short-lived local WebSocket relay sufficient to falsify REQ/EOSE handling."""

    def __init__(self, frames: list[object]) -> None:
        self._listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._listener.bind(("127.0.0.1", 0))
        self._listener.listen(1)
        self._frames = frames
        self.request: object | None = None
        self._thread = threading.Thread(target=self._serve, daemon=True)

    @property
    def url(self) -> str:
        return f"ws://127.0.0.1:{self._listener.getsockname()[1]}"

    def start(self) -> None:
        self._thread.start()

    def join(self) -> None:
        self._thread.join(timeout=2)
        self._listener.close()

    def _serve(self) -> None:
        connection, _ = self._listener.accept()
        with connection:
            request = read_until(connection, b"\r\n\r\n")
            key = next(
                line.split(b":", 1)[1].strip()
                for line in request.split(b"\r\n")
                if line.lower().startswith(b"sec-websocket-key:")
            )
            accepted = base64.b64encode(
                hashlib.sha1(key + b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11").digest()
            )
            connection.sendall(
                b"HTTP/1.1 101 Switching Protocols\r\n"
                b"Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: "
                + accepted
                + b"\r\n\r\n"
            )
            self.request = read_client_text(connection)
            for frame in self._frames:
                send_server_text(connection, json.dumps(frame, separators=(",", ":")).encode())


def read_until(connection: socket.socket, marker: bytes) -> bytes:
    value = bytearray()
    while marker not in value:
        value.extend(connection.recv(4_096))
    return bytes(value)


def read_client_text(connection: socket.socket) -> object:
    first, second = connection.recv(2)
    assert first == 0x81
    length = second & 0x7F
    if length == 126:
        length = int.from_bytes(connection.recv(2), "big")
    mask = connection.recv(4)
    payload = bytearray()
    while len(payload) < length:
        payload.extend(connection.recv(length - len(payload)))
    return json.loads(bytes(value ^ mask[index % 4] for index, value in enumerate(payload)).decode())


def send_server_text(connection: socket.socket, payload: bytes) -> None:
    if len(payload) < 126:
        header = bytes([0x81, len(payload)])
    else:
        header = bytes([0x81, 126]) + len(payload).to_bytes(2, "big")
    connection.sendall(header + payload)


class InspectionTests(unittest.TestCase):
    def test_requires_matching_eose_and_preserves_exact_event(self) -> None:
        event = {
            "content": "arbitrary-kind content",
            "created_at": 1,
            "kind": 12345,
            "pubkey": "b" * 64,
            "tags": [["h", "room"]],
        }
        event["id"] = event_id(event)
        relay = ScriptedRelay([["EVENT", "other", event], ["EVENT", "proof", event], ["EOSE", "proof"]])
        relay.start()
        result = inspect_until_eose(relay.url, "proof", {"ids": [event["id"]]}, 1)
        relay.join()
        self.assertEqual(relay.request, ["REQ", "proof", {"ids": [event["id"]]}])
        self.assertEqual(result.events, [event])
        assert_event(
            result.events[0],
            {
                "content": "arbitrary-kind content",
                "id": event["id"],
                "kind": 12345,
                "pubkey": "b" * 64,
                "tags": [["h", "room"]],
            },
        )

    def test_exact_tag_mismatch_is_a_failure(self) -> None:
        event = {
            "content": "",
            "created_at": 1,
            "kind": 1,
            "pubkey": "b" * 64,
            "tags": [["h", "wrong"]],
        }
        event["id"] = event_id(event)
        with self.assertRaisesRegex(InspectionError, "wrong tags"):
            assert_event(event, {"tags": [["h", "room"]]})

    def test_rejects_relay_evidence_above_total_byte_bound(self) -> None:
        relay = ScriptedRelay(
            [["EVENT", "proof", {"content": "x" * 60_000}] for _ in range(18)]
        )
        relay.start()
        with self.assertRaisesRegex(InspectionError, "event evidence exceeded"):
            inspect_until_eose(relay.url, "proof", {"limit": 18}, 2)
        relay.join()


class FixtureTests(unittest.TestCase):
    def test_command_materialization_leaves_only_ordinary_repl_lines(self) -> None:
        source = LIVE / "commands" / "smoke-create-content.txt"
        destination = self.enterContext(self._temporary_file())
        materialize_commands(
            source,
            destination,
            {
                "GROUP_ID": "fava-e2e-group",
                "GROUP_RELAY": "ws://127.0.0.1:18101",
                "STATE_RELAY": "ws://127.0.0.1:18102",
            },
        )
        lines = destination.read_text(encoding="utf-8").splitlines()
        self.assertEqual(lines[0], "relay add group ws://127.0.0.1:18101")
        self.assertEqual(lines[3], "group create fava-e2e-group group")
        self.assertNotIn("{{", destination.read_text(encoding="utf-8"))

    def test_result_captures_refuse_ambiguous_application_evidence(self) -> None:
        rows = [
            {"kind": "group-created", "fields": {"event_id": "one"}},
            {"kind": "group-created", "fields": {"event_id": "two"}},
        ]
        with self.assertRaisesRegex(HarnessError, "matched 2"):
            result_captures(rows, {"create": {"kind": "group-created", "fields": {}}})

    def test_scenario_references_are_resolved_before_wire_assertion(self) -> None:
        context = {"create": {"author": "alice", "event_id": "id"}, "group_id": "room"}
        self.assertEqual(
            resolve({"ids": ["$create.event_id"], "#h": ["$group_id"]}, context),
            {"ids": ["id"], "#h": ["room"]},
        )

    def test_resolved_filter_is_bounded_before_direct_req(self) -> None:
        scenario = {
            "assertions": [
                {"filter": {"ids": ["$created.event_id"]}, "present": False, "relay": "group"}
            ],
            "captures": {"created": {"kind": "created"}},
        }
        rows = [{"fields": {"event_id": "a" * MAX_FILTER_BYTES}, "kind": "created"}]
        with self.assertRaisesRegex(HarnessError, "resolved filter exceeded"):
            inspect_assertions(
                scenario,
                {"group": SimpleNamespace(url="ws://127.0.0.1:1")},
                rows,
                {},
                Path("unused-after-admission"),
            )

    def test_jsonl_is_deterministic(self) -> None:
        self.assertEqual(json_line({"z": 1, "a": 2}), '{"a":2,"z":1}\n')

    def test_every_live_harness_code_file_stays_within_the_hard_line_limit(self) -> None:
        for source in LIVE.glob("*.py"):
            self.assertLessEqual(
                len(source.read_text(encoding="utf-8").splitlines()),
                800,
                source.name,
            )

    def test_full_contract_is_staged_with_only_the_four_croissant_state_kinds(self) -> None:
        contract = json.loads(
            (LIVE / "scenarios" / "full-nip29-contract.json").read_text(encoding="utf-8")
        )
        self.assertEqual(contract["status"], "executable")
        self.assertEqual([stage["after_line"] for stage in contract["stages"]], [24, 28, 32, 41])
        state = contract["stages"][0]["assertions"][-1]
        self.assertEqual(state["required_kinds"], [39000, 39001, 39002, 39003])
        self.assertNotIn(39004, state["required_kinds"])
        validate_executable_scenario(contract)

    def test_staged_contract_refuses_removed_stage_assertions(self) -> None:
        contract = json.loads(
            (LIVE / "scenarios" / "full-nip29-contract.json").read_text(encoding="utf-8")
        )
        contract["stages"][0]["assertions"] = []
        with self.assertRaisesRegex(HarnessError, "requires concrete assertions"):
            validate_executable_scenario(contract)

    def test_croissant_gap_control_keeps_the_9008_absence_explicit(self) -> None:
        contract = __import__("harness").load_scenario("croissant-9008-retention-gap")
        self.assertEqual(contract["bounded_gap_control_of"], "full-nip29-contract")
        self.assertFalse(contract["stages"][-1]["assertions"][0]["present"])
        self.assertEqual(len([item for stage in contract["stages"] for item in stage["assertions"]]), 18)
        validate_executable_scenario(contract)

    def test_canonical_croissant_gap_bundle_is_hashed_and_explicit(self) -> None:
        evidence = LIVE / "evidence" / "2026-08-29-croissant-9008-retention-gap"
        manifest = json.loads((evidence / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(len(manifest["artifact_sha256"]), 20)
        for relative, expected in manifest["artifact_sha256"].items():
            self.assertEqual(hashlib.sha256((evidence / relative).read_bytes()).hexdigest(), expected)
        captures = json.loads((evidence / "app-captures.json").read_text(encoding="utf-8"))
        self.assertEqual(captures["captures"]["group_deleted"]["kind"], 9008)
        inspection = json.loads((evidence / "inspections" / "15.json").read_text(encoding="utf-8"))
        self.assertEqual(inspection["events"], [])
        check_retained_artifacts(evidence)

    def test_runner_refuses_assertionless_executable_before_artifact_creation(self) -> None:
        forged = {"id": "forged", "status": "executable", "command_file": "commands/none.txt"}
        with patch("harness.load_scenario", return_value=forged):
            with self.assertRaisesRegex(HarnessError, "at least one concrete assertion"):
                run(SimpleNamespace(scenario="forged"))

    def test_retention_scan_runs_even_when_transient_cleanup_fails(self) -> None:
        import tempfile

        root = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(root, ignore_errors=True))
        scratch = root / "scratch"
        artifacts = root / "artifacts"
        scratch.mkdir()
        artifacts.mkdir()
        scanned: list[bool] = []
        with patch("harness.remove_scratch", side_effect=OSError("refused")):
            with patch("harness.check_retained_artifacts", side_effect=lambda *_: scanned.append(True)):
                with self.assertRaisesRegex(HarnessError, "scratch cleanup failed"):
                    cleanup_before_retention(scratch, artifacts)
        self.assertEqual(scanned, [True])

    def test_result_write_failure_still_cleans_materialized_command_and_scans(self) -> None:
        import tempfile

        root = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(root, ignore_errors=True))
        artifacts = root / "artifacts"
        scratch = root / "scratch"
        scratch.mkdir()
        facts = {
            "group_gone": True,
            "output_overflowed": False,
            "output_threads_joined": True,
            "teardown_error": False,
        }
        relays = [
            SimpleNamespace(label="group", url="ws://127.0.0.1:1", process=SimpleNamespace(stop=lambda: dict(facts))),
            SimpleNamespace(label="state", url="ws://127.0.0.1:2", process=SimpleNamespace(stop=lambda: dict(facts))),
        ]
        cleaned: list[bool] = []
        original_cleanup = cleanup_before_retention

        def verify_cleanup(*arguments: object) -> None:
            self.assertTrue((scratch / "commands.txt").exists())
            cleaned.append(True)
            original_cleanup(*arguments)

        arguments = SimpleNamespace(
            scenario="smoke-create-content",
            artifacts=artifacts,
            group_id="fava-e2e-group",
            nip29_bin=sys.executable,
            ordinary_bin=sys.executable,
            app_command=None,
        )
        with patch("harness.require_binary", return_value=Path(sys.executable)), \
             patch("harness.ordinary_fixture_version", return_value="nostr-rs-relay 0.8.12"), \
             patch("harness.binary_sha256", return_value="a" * 64), \
             patch("harness.new_scratch", return_value=scratch), \
             patch("harness.start_croissant", return_value=relays[0]), \
             patch("harness.start_ordinary", return_value=relays[1]), \
             patch("harness.wait_ready"), \
             patch("harness.run_app", side_effect=HarnessError("application failure")), \
             patch("harness.write_result", side_effect=OSError("disk failure")), \
             patch("harness.cleanup_before_retention", side_effect=verify_cleanup):
            with redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(HarnessError, "application failure"):
                    run(arguments)
        self.assertEqual(cleaned, [True])
        self.assertFalse(scratch.exists())

    def test_retention_scan_has_file_bounds(self) -> None:
        import tempfile

        root = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(root, ignore_errors=True))
        (root / "large").write_bytes(b"x" * (MAX_ARTIFACT_FILE_BYTES + 1))
        with self.assertRaisesRegex(HarnessError, "retained artifact exceeded"):
            check_retained_artifacts(root)
        (root / "large").unlink()
        (root / "one").write_bytes(b"a")
        (root / "two").write_bytes(b"b")
        with patch("harness_safety.MAX_ARTIFACT_FILES", 1):
            with self.assertRaisesRegex(HarnessError, "exceeded 1 files"):
                check_retained_artifacts(root)
        (root / "two").unlink()
        with patch("harness_safety.MAX_ARTIFACT_TOTAL_BYTES", 0):
            with self.assertRaisesRegex(HarnessError, "exceeded 0 bytes"):
                check_retained_artifacts(root)

    def test_process_group_teardown_kills_descendant_after_parent_exit(self) -> None:
        import tempfile

        directory = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(directory, ignore_errors=True))
        child_code = "import time; time.sleep(60)"
        parent_code = (
            "import subprocess,sys; "
            f"subprocess.Popen([sys.executable, '-c', {child_code!r}])"
        )
        process = ManagedProcess.start(
            "orphan-test",
            [sys.executable, "-c", parent_code],
            directory,
            dict(os.environ),
        )
        self.addCleanup(process.stop)
        process.process.wait(timeout=2)
        time.sleep(0.05)
        facts = process.stop()
        self.assertTrue(facts["parent_exited_before_teardown"])
        self.assertTrue(facts["descendants_survived_parent"])
        self.assertTrue(facts["group_gone"])
        self.assertTrue(facts["output_threads_joined"])

    def test_canonical_real_relay_bundle_is_hashed_and_bounded(self) -> None:
        evidence = LIVE / "evidence" / "2026-08-28-smoke"
        manifest = json.loads((evidence / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["scenario"], "smoke-create-content")
        for relative, expected_hash in manifest["artifact_sha256"].items():
            self.assertEqual(
                hashlib.sha256((evidence / relative).read_bytes()).hexdigest(), expected_hash,
                relative,
            )
        result = json.loads((evidence / "result.json").read_text(encoding="utf-8"))
        self.assertEqual(result["outcome"], "passed")
        self.assertEqual(result["fixtures"]["state"]["version"], "nostr-rs-relay 0.8.12")
        self.assertEqual(
            sorted(path.name for path in evidence.iterdir()),
            ["app-results.jsonl", "inspections", "manifest.json", "result.json"],
        )
        self.assertEqual(sorted(path.name for path in (evidence / "inspections").iterdir()), ["01.json", "02.json"])
        self.assertFalse((evidence / "run.jsonl").exists())
        self.assertFalse((evidence / "relays").exists())
        check_retained_artifacts(evidence)

    def _temporary_file(self):
        import tempfile

        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        return _PathContext(Path(directory.name) / "commands.txt")


class _PathContext:
    def __init__(self, path: Path) -> None:
        self.path = path

    def __enter__(self) -> Path:
        return self.path

    def __exit__(self, *unused: object) -> None:
        return None


def event_id(event: dict[str, object]) -> str:
    value = [
        0,
        event["pubkey"],
        event["created_at"],
        event["kind"],
        event["tags"],
        event["content"],
    ]
    return hashlib.sha256(
        json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    ).hexdigest()


if __name__ == "__main__":
    unittest.main()
