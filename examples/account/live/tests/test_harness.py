import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

LIVE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIVE))
spec = importlib.util.spec_from_file_location("account_harness", LIVE / "harness.py")
harness = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(harness)


class HarnessContract(unittest.TestCase):
    def test_unpinned_relay_version_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "relay"
            binary.write_text("#!/bin/sh\necho 'nostr-rs-relay 9.9.9'\n", encoding="utf-8")
            binary.chmod(0o700)
            with self.assertRaises(harness.HarnessError):
                harness.relay_version(binary)

    def test_canonical_manifest_matches_every_retained_artifact(self):
        root = LIVE / "evidence/2026-08-30-account-reactivity"
        manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
        actual = {
            str(path.relative_to(root))
            for path in root.rglob("*")
            if path.is_file() and path.name != "manifest.json"
        }
        self.assertEqual(set(manifest["files"]), actual)
        for name, claim in manifest["files"].items():
            data = (root / name).read_bytes()
            self.assertEqual(claim["bytes"], len(data), name)
            self.assertEqual(claim["sha256"], hashlib.sha256(data).hexdigest(), name)

    def test_captures_and_reactive_sequence_are_exact(self):
        alice = "a" * 64
        bob = "b" * 64
        rows = [
            row("account-imported", {"account": "alice", "public_key": alice}),
            row("capture-set", {"capture": "alice-pub", "value": alice}),
            row("published", {"event_id": "1" * 64}),
            row("capture-set", {"capture": "alice-event", "value": "1" * 64}),
            snapshot(1, [alice]),
            row("account-imported", {"account": "bob", "public_key": bob}),
            row("capture-set", {"capture": "bob-pub", "value": bob}),
            snapshot(1, []),
            row("published", {"event_id": "2" * 64}),
            row("capture-set", {"capture": "bob-event", "value": "2" * 64}),
            snapshot(1, [bob]),
            snapshot(1, [alice]),
            snapshot(1, [bob]),
            row(
                "routes",
                {
                    "demand_relays": ["ws://relay.example"],
                    "demand_observations": [1],
                    "wire_subscriptions": ["subscription"],
                },
            ),
            snapshot(1, []),
            row(
                "routes",
                {
                    "demand_relays": [],
                    "demand_observations": [],
                    "wire_subscriptions": [],
                },
            ),
        ]
        captured = harness.capture_rows(rows)
        self.assertEqual(captured["alice-pub"], alice)
        self.assertEqual(captured["bob-event"], "2" * 64)
        harness.require_reactive_transitions(rows, captured)
        harness.require_route_attribution(rows)

    def test_changed_observation_id_is_rejected(self):
        captured = {
            "alice-pub": "a" * 64,
            "bob-pub": "b" * 64,
            "alice-event": "1" * 64,
            "bob-event": "2" * 64,
        }
        rows = [
            snapshot(1, [captured["alice-pub"]]),
            snapshot(1, []),
            snapshot(2, [captured["bob-pub"]]),
            snapshot(2, [captured["alice-pub"]]),
            snapshot(2, [captured["bob-pub"]]),
            snapshot(2, []),
        ]
        with self.assertRaises(harness.HarnessError):
            harness.require_reactive_transitions(rows, captured)


def row(kind, fields):
    return {"status": "ok", "kind": kind, "summary": "", "fields": fields}


def snapshot(observation, authors):
    return row("query-snapshot", {"observation_id": observation, "authors": authors})


if __name__ == "__main__":
    unittest.main()
