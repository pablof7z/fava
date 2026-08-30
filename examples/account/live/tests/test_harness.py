import importlib.util
import sys
import unittest
from pathlib import Path

LIVE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIVE))
spec = importlib.util.spec_from_file_location("account_harness", LIVE / "harness.py")
harness = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(harness)


class HarnessContract(unittest.TestCase):
    def test_captures_and_reactive_sequence_are_exact(self):
        alice = "a" * 64
        bob = "b" * 64
        rows = [
            row("account-imported", {"account": "alice", "public_key": alice}),
            row("published", {"event_id": "1" * 64}),
            snapshot(1, [alice]),
            row("account-imported", {"account": "bob", "public_key": bob}),
            snapshot(1, []),
            row("published", {"event_id": "2" * 64}),
            snapshot(1, [bob]),
            snapshot(1, [alice]),
            snapshot(1, [bob]),
            snapshot(1, []),
        ]
        captured = harness.capture_rows(rows)
        self.assertEqual(captured["alice-pub"], alice)
        self.assertEqual(captured["bob-event"], "2" * 64)
        harness.require_reactive_transitions(rows, captured)

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
