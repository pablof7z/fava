"""Behaviour tests for vocabulary approval parsing, rendering, and serving."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

_TOOLS = str(Path(__file__).parents[1])
if _TOOLS not in sys.path:
    sys.path.insert(0, _TOOLS)
import vocabulary_approval as approval

# Real nostr-crate-generated fixtures (throwaway keys only; owner key is
# unavailable here — owner approval is the Rust governance test's gate).
#
# Both events were produced by /private/tmp/claude-501/sigvec/src/main.rs and
# pass nostr::Event::verify().  They are used to exercise the *structural*
# checks that Python still owns (pubkey, kind, name tag) without re-implementing
# cryptography.

# secret key = scalar 1, canonical markdown of the "Event" term
THROWAWAY_EVENT = json.loads(
    '{"id":"bbc6bc2bb03fcff13f3b465c8edda0269e51c844c2c1d067c77f02962a4d8ac4",'
    '"pubkey":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",'
    '"created_at":1700000000,"kind":9999,'
    '"tags":[["name","Event"]],'
    '"content":"# Event\\n\\n**source**: nostr\\n\\n**protocol**: NIP-01\\n\\n'
    '**owner**: nostr\\n\\n**meaning**: A signed Nostr event.\\n",'
    '"sig":"f218801e16e03833d7c9d6a8bc179a68e0747c15960fb19487be2326bf687acd'
    '3187b1a58a3f6b9a07647e6298d42c5c22a6f8ac1123417ad7e62e17356bd2a2"}'
)

# Like the owner-signed path but with the throwaway pubkey swapped in; used
# to test that OWNER equality is enforced before content is accepted.
OWNER_SHAPED_EVENT = dict(THROWAWAY_EVENT)
OWNER_SHAPED_EVENT["pubkey"] = approval.OWNER

EVENT_TERM = {
    "name": "Event",
    "source": "nostr",
    "protocol": "NIP-01",
    "meaning": "A signed Nostr event.",
    "owner": "nostr",
    "symbols": [],
    "crates": [],
}


class CanonicalMarkdownTest(unittest.TestCase):
    def test_heading_is_term_name(self) -> None:
        md = approval.canonical_markdown({"name": "Foo"})
        self.assertTrue(md.startswith("# Foo\n"))

    def test_prose_fields_appear_in_order(self) -> None:
        term = {
            "name": "T",
            "source": "fava",
            "meaning": "A thing.",
            "falsifier": "test must fail.",
        }
        md = approval.canonical_markdown(term)
        src = md.index("**source**")
        mean = md.index("**meaning**")
        fals = md.index("**falsifier**")
        self.assertLess(src, mean)
        self.assertLess(mean, fals)

    def test_empty_prose_field_is_omitted(self) -> None:
        term = {"name": "T", "source": "nostr", "meaning": ""}
        md = approval.canonical_markdown(term)
        self.assertIn("**source**", md)
        self.assertNotIn("**meaning**", md)

    def test_whitespace_only_prose_field_is_omitted(self) -> None:
        term = {"name": "T", "meaning": "   "}
        md = approval.canonical_markdown(term)
        self.assertNotIn("**meaning**", md)

    def test_empty_list_field_is_omitted(self) -> None:
        term = {"name": "T", "symbols": [], "crates": ["fava-foo"]}
        md = approval.canonical_markdown(term)
        self.assertNotIn("**symbols**", md)
        self.assertIn("**crates**", md)

    def test_list_items_are_sorted(self) -> None:
        term = {"name": "T", "symbols": ["b::B", "a::A"]}
        md = approval.canonical_markdown(term)
        self.assertLess(md.index("a::A"), md.index("b::B"))

    def test_extra_field_is_included(self) -> None:
        term = {"name": "T", "source": "fava", "custom_field": "custom_value"}
        md = approval.canonical_markdown(term)
        self.assertIn("**custom_field**: custom_value", md)

    def test_output_ends_with_single_newline(self) -> None:
        md = approval.canonical_markdown({"name": "T", "source": "nostr"})
        self.assertTrue(md.endswith("\n"))
        self.assertFalse(md.endswith("\n\n"))

    def test_deterministic_across_calls(self) -> None:
        term = {
            "name": "Query",
            "source": "fava",
            "meaning": "A request.",
            "symbols": ["fava_query::Query", "fava_query::QueryBounds"],
        }
        self.assertEqual(
            approval.canonical_markdown(term), approval.canonical_markdown(term)
        )

    def test_non_ascii_content_is_preserved(self) -> None:
        term = {"name": "T", "meaning": "Ünïcödé and emoji 🐍."}
        md = approval.canonical_markdown(term)
        self.assertIn("Ünïcödé and emoji 🐍", md)

    def test_backslash_and_quotes_are_preserved(self) -> None:
        term = {"name": "T", "meaning": 'back\\slash and "quotes"'}
        md = approval.canonical_markdown(term)
        self.assertIn('back\\slash and "quotes"', md)

    def test_event_term_matches_fixture_content(self) -> None:
        """The fixture content in THROWAWAY_EVENT must match current rendering."""
        expected = (
            "# Event\n\n"
            "**source**: nostr\n\n"
            "**protocol**: NIP-01\n\n"
            "**owner**: nostr\n\n"
            "**meaning**: A signed Nostr event.\n"
        )
        self.assertEqual(approval.canonical_markdown(EVENT_TERM), expected)
        self.assertEqual(THROWAWAY_EVENT["content"], expected)


class ApprovedNameTest(unittest.TestCase):
    def test_returns_name_for_single_name_tag(self) -> None:
        event = {"tags": [["name", "Query"]]}
        self.assertEqual(approval.approved_name(event), "Query")

    def test_returns_none_for_no_name_tag(self) -> None:
        event = {"tags": [["p", "abc"]]}
        self.assertIsNone(approval.approved_name(event))

    def test_returns_none_for_multiple_name_tags(self) -> None:
        event = {"tags": [["name", "A"], ["name", "B"]]}
        self.assertIsNone(approval.approved_name(event))

    def test_returns_none_for_empty_tags(self) -> None:
        self.assertIsNone(approval.approved_name({"tags": []}))


class VerifyEventTest(unittest.TestCase):
    """Python structural verification — no crypto."""

    def _base(self) -> dict:
        return {
            "id": "aabbcc",
            "pubkey": approval.OWNER,
            "created_at": 1700000000,
            "kind": 9999,
            "tags": [["name", "Query"]],
            "content": "...",
            "sig": "ddeeff",
        }

    def test_accepts_structurally_valid_owner_event(self) -> None:
        self.assertEqual(approval.verify_event(self._base()), [])

    def test_rejects_missing_fields(self) -> None:
        event = {"id": "x"}
        problems = approval.verify_event(event)
        self.assertEqual(len(problems), 1)
        self.assertIn("missing", problems[0])

    def test_rejects_wrong_pubkey(self) -> None:
        event = self._base()
        event["pubkey"] = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        problems = approval.verify_event(event)
        self.assertTrue(any("not signed by the owner" in p for p in problems))

    def test_rejects_wrong_kind(self) -> None:
        event = self._base()
        event["kind"] = 1
        problems = approval.verify_event(event)
        self.assertTrue(any("kind must be 9999" in p for p in problems))

    def test_rejects_no_name_tag(self) -> None:
        event = self._base()
        event["tags"] = []
        problems = approval.verify_event(event)
        self.assertTrue(any("exactly one name tag" in p for p in problems))

    def test_rejects_multiple_name_tags(self) -> None:
        event = self._base()
        event["tags"] = [["name", "A"], ["name", "B"]]
        problems = approval.verify_event(event)
        self.assertTrue(any("exactly one name tag" in p for p in problems))

    def test_throwaway_event_is_rejected_on_pubkey(self) -> None:
        """The nostr-crate fixture with a throwaway pubkey fails Python's
        owner check even though it would pass crypto verification."""
        problems = approval.verify_event(THROWAWAY_EVENT)
        self.assertTrue(any("not signed by the owner" in p for p in problems))

    def test_does_not_reject_id_or_sig_values(self) -> None:
        """Python no longer checks id hash or Schnorr validity; those are
        Rust's responsibility.  A structurally complete event must not be
        rejected for a bad id or sig here."""
        event = self._base()
        event["id"] = "0" * 64
        event["sig"] = "0" * 128
        self.assertEqual(approval.verify_event(event), [])


class LoadApprovalsTest(unittest.TestCase):
    def _write(self, tmp: Path, *events: dict) -> Path:
        path = tmp / "approvals.jsonl"
        path.write_text(
            "\n".join(json.dumps(e) for e in events) + "\n", encoding="utf-8"
        )
        return path

    def test_empty_file_returns_empty(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "approvals.jsonl"
            path.write_text("", encoding="utf-8")
            approvals, problems = approval.load_approvals(path)
        self.assertEqual(approvals, {})
        self.assertEqual(problems, [])

    def test_absent_file_returns_empty(self) -> None:
        approvals, problems = approval.load_approvals(Path("/nonexistent/approvals.jsonl"))
        self.assertEqual(approvals, {})
        self.assertEqual(problems, [])

    def test_valid_owner_event_is_loaded(self) -> None:
        event = {
            "id": "x", "pubkey": approval.OWNER, "created_at": 1700000000,
            "kind": 9999, "tags": [["name", "Query"]], "content": "md",
            "sig": "y",
        }
        with tempfile.TemporaryDirectory() as d:
            path = self._write(Path(d), event)
            approvals, problems = approval.load_approvals(path)
        self.assertIn("Query", approvals)
        self.assertEqual(problems, [])

    def test_wrong_pubkey_event_is_rejected(self) -> None:
        event = {
            "id": "x", "pubkey": "deadbeef" * 8, "created_at": 1700000000,
            "kind": 9999, "tags": [["name", "Query"]], "content": "md",
            "sig": "y",
        }
        with tempfile.TemporaryDirectory() as d:
            path = self._write(Path(d), event)
            approvals, problems = approval.load_approvals(path)
        self.assertNotIn("Query", approvals)
        self.assertTrue(len(problems) > 0)

    def test_later_timestamp_wins(self) -> None:
        base = {
            "id": "x", "pubkey": approval.OWNER, "kind": 9999,
            "tags": [["name", "Query"]], "content": "v1", "sig": "y",
        }
        old = dict(base, created_at=1_000_000)
        new = dict(base, created_at=2_000_000, content="v2")
        with tempfile.TemporaryDirectory() as d:
            path = self._write(Path(d), old, new)
            approvals, _ = approval.load_approvals(path)
        self.assertEqual(approvals["Query"]["content"], "v2")

    def test_malformed_json_line_reported(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "approvals.jsonl"
            path.write_text("not json\n", encoding="utf-8")
            _, problems = approval.load_approvals(path)
        self.assertTrue(any("unreadable" in p for p in problems))


class UnapprovedTermsTest(unittest.TestCase):
    def test_no_approval_reported(self) -> None:
        term = {"name": "Foo", "meaning": "x"}
        problems = approval.unapproved_terms((term,), {})
        self.assertEqual(problems, ["Foo: no signed approval"])

    def test_matching_approval_is_silent(self) -> None:
        term = {"name": "Foo", "meaning": "x"}
        md = approval.canonical_markdown(term)
        evt = {"content": md}
        problems = approval.unapproved_terms((term,), {"Foo": evt})
        self.assertEqual(problems, [])

    def test_stale_approval_detected(self) -> None:
        """Editing a term after approval must be caught by content mismatch."""
        original = {"name": "Foo", "meaning": "original meaning"}
        md = approval.canonical_markdown(original)
        evt = {"content": md}

        modified = {"name": "Foo", "meaning": "CHANGED meaning"}
        problems = approval.unapproved_terms((modified,), {"Foo": evt})
        self.assertEqual(problems, ["Foo: changed since its approval was signed"])

    def test_stale_detected_after_list_field_change(self) -> None:
        original = {"name": "Foo", "symbols": ["foo::Bar"]}
        md = approval.canonical_markdown(original)
        evt = {"content": md}

        modified = {"name": "Foo", "symbols": ["foo::Bar", "foo::Baz"]}
        problems = approval.unapproved_terms((modified,), {"Foo": evt})
        self.assertIn("Foo: changed since its approval was signed", problems)

    def test_stale_detected_after_field_removed(self) -> None:
        original = {"name": "Foo", "meaning": "something", "distinction": "key detail"}
        md = approval.canonical_markdown(original)
        evt = {"content": md}

        without_distinction = {"name": "Foo", "meaning": "something"}
        problems = approval.unapproved_terms((without_distinction,), {"Foo": evt})
        self.assertIn("Foo: changed since its approval was signed", problems)


def _network_available() -> bool:
    import socket as _sock
    try:
        with _sock.socket() as s:
            s.bind(("127.0.0.1", 0))
        return True
    except OSError:
        return False


# Mock verifier script written to the temp project dir in setUp.
# OWNER = throwaway pubkey (sk=0x01).  Treats sig == "0"*128 as a crypto
# failure so test_bad_crypto_event_rejected can exercise that path.
_MOCK_VERIFIER_SRC = """\
#!/usr/bin/env python3
import json, sys
OWNER = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
event = json.loads(sys.stdin.read().strip())
if event.get("pubkey") != OWNER:
    print(f"pubkey is not the owner: {event.get('pubkey')}", file=sys.stderr)
    sys.exit(1)
if event.get("kind") != 9999:
    print(f"wrong kind: {event.get('kind')}", file=sys.stderr)
    sys.exit(1)
names = [t[1] for t in event.get("tags", [])
         if isinstance(t, list) and len(t) >= 2 and t[0] == "name"]
if len(names) != 1:
    print(f"must have exactly one name tag, got {len(names)}", file=sys.stderr)
    sys.exit(1)
if event.get("sig") == "0" * 128:
    print("signature verification failed (simulated bad sig)", file=sys.stderr)
    sys.exit(1)
print(names[0])
"""


@unittest.skipUnless(_network_available(), "socket binding not available in this environment")
class ServerTest(unittest.TestCase):
    """End-to-end HTTP server tests.

    The server runs in a subprocess so socket binding is outside the test
    sandbox.  OWNER is patched to the throwaway pubkey (sk=0x01) in both
    the Python module and the mock verifier so the success path can be
    exercised without Pablo's private key.

    A mock verifier script replaces the Rust binary.  It performs structural
    checks (pubkey, kind, name tag) and treats sig == "0"*128 as a crypto
    failure.  This proves the server calls the verifier and honours its exit
    code; Schnorr correctness is the Rust governance test's responsibility.
    """

    _VOCAB = (
        'version = 1\n\n'
        '[[term]]\n'
        'name = "Event"\n'
        'source = "nostr"\n'
        'protocol = "NIP-01"\n'
        'meaning = "A signed Nostr event."\n'
        'owner = "nostr"\n'
        'symbols = []\n'
        'crates = []\n'
    )
    _THROWAWAY_OWNER = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"

    def setUp(self) -> None:
        import socket as _sock
        self._tmp = tempfile.TemporaryDirectory()
        self._root = Path(self._tmp.name)
        internals = self._root / "docs" / "internals"
        internals.mkdir(parents=True)
        (internals / "vocabulary.toml").write_text(self._VOCAB, encoding="utf-8")

        # Write the mock verifier and make it executable.
        self._verifier = self._root / "vocab-verify-mock"
        self._verifier.write_text(_MOCK_VERIFIER_SRC, encoding="utf-8")
        os.chmod(self._verifier, 0o755)

        # Patch OWNER in a copy of vocabulary_approval so the subprocess
        # uses the throwaway key without touching the shipped file.
        patched_va = self._root / "vocabulary_approval.py"
        src = (Path(_TOOLS) / "vocabulary_approval.py").read_text(encoding="utf-8")
        patched_va.write_text(
            src.replace(
                f'OWNER = "{approval.OWNER}"',
                f'OWNER = "{self._THROWAWAY_OWNER}"',
            ),
            encoding="utf-8",
        )
        shutil.copy(
            Path(_TOOLS) / "approve_vocabulary.py",
            self._root / "approve_vocabulary.py",
        )

        with _sock.socket() as s:
            s.bind(("127.0.0.1", 0))
            self._port = s.getsockname()[1]

        self._proc = subprocess.Popen(
            [
                sys.executable,
                str(self._root / "approve_vocabulary.py"),
                "--root", str(self._root),
                "--port", str(self._port),
                "--no-open",
                "--verifier", str(self._verifier),
            ],
            cwd=str(self._root),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        import urllib.request
        deadline = time.monotonic() + 5.0
        while time.monotonic() < deadline:
            try:
                with urllib.request.urlopen(  # noqa: S310
                    f"http://127.0.0.1:{self._port}/api/terms"
                ):
                    break
            except Exception:
                time.sleep(0.05)
        else:
            self._proc.terminate()
            self._proc.wait()
            self._tmp.cleanup()
            raise RuntimeError("server did not start in time")

    def tearDown(self) -> None:
        self._proc.terminate()
        self._proc.wait(timeout=5)
        self._tmp.cleanup()

    def _get(self, path: str):
        import urllib.request
        return urllib.request.urlopen(  # noqa: S310
            f"http://127.0.0.1:{self._port}{path}"
        )

    def _post(self, path: str, body: dict) -> tuple[int, dict]:
        import urllib.error
        import urllib.request
        data = json.dumps(body).encode("utf-8")
        req = urllib.request.Request(
            f"http://127.0.0.1:{self._port}{path}",
            data=data,
            headers={
                "Content-Type": "application/json",
                "Content-Length": str(len(data)),
            },
            method="POST",
        )
        try:
            with urllib.request.urlopen(req) as resp:  # noqa: S310
                return resp.status, json.loads(resp.read())
        except urllib.error.HTTPError as exc:
            with exc:
                body_bytes = exc.read()
            return exc.code, json.loads(body_bytes)

    def test_get_terms_returns_event_term(self) -> None:
        with self._get("/api/terms") as resp:
            payload = json.loads(resp.read())
        names = [t["name"] for t in payload["terms"]]
        self.assertIn("Event", names)

    def test_wrong_path_returns_404(self) -> None:
        import urllib.error
        with self.assertRaises(urllib.error.HTTPError) as ctx:
            with self._get("/api/nonexistent"):
                pass
        self.assertEqual(ctx.exception.code, 404)
        ctx.exception.close()

    def test_throwaway_event_rejected_as_wrong_pubkey(self) -> None:
        """An event signed by a non-OWNER key is rejected by Python's structural
        check before the verifier is called.  We submit the sk=0x02 event."""
        wrong_key_event = json.loads(
            '{"id":"a5da16e5cf91a2fa5ca407fcf31808092dfb917d4fdee7b3ad9375f9d8487ccb",'
            '"pubkey":"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",'
            '"created_at":1700000000,"kind":9999,'
            '"tags":[["name","Event"]],'
            '"content":"# Event\\n\\n**source**: nostr\\n\\n**protocol**: NIP-01\\n\\n'
            '**owner**: nostr\\n\\n**meaning**: A signed Nostr event.\\n",'
            '"sig":"05f4b05e4a2c0842a17b4f91daf4f5e8916a9d9328e07f86bdf1e7602c74ec09'
            'aba2102211cf647ec90f4638e36a0a732a273e543faf4130c1b625987feff491"}'
        )
        status, body = self._post("/api/approvals", wrong_key_event)
        self.assertEqual(status, 400)
        self.assertIn("not signed by the owner", body["error"])

    def test_correct_owner_event_accepted_and_persisted(self) -> None:
        """OWNER is the throwaway key (sk=0x01); THROWAWAY_EVENT is signed by
        that key and must be accepted by the mock verifier and written to disk."""
        status, body = self._post("/api/approvals", THROWAWAY_EVENT)
        self.assertEqual(status, 200, body)
        self.assertEqual(body["stored"], "Event")
        path = self._root / "docs" / "internals" / "approvals.jsonl"
        self.assertTrue(path.exists())
        stored = json.loads(path.read_text(encoding="utf-8").strip())
        self.assertEqual(stored["id"], THROWAWAY_EVENT["id"])

    def test_replay_returns_already_stored_and_file_unchanged(self) -> None:
        """Replaying an identical event returns 200 'already stored' and does
        not append a second line to approvals.jsonl."""
        status1, body1 = self._post("/api/approvals", THROWAWAY_EVENT)
        self.assertEqual(status1, 200, body1)

        status2, body2 = self._post("/api/approvals", THROWAWAY_EVENT)
        self.assertEqual(status2, 200, body2)
        self.assertEqual(body2.get("note"), "already stored")

        path = self._root / "docs" / "internals" / "approvals.jsonl"
        lines = [ln for ln in path.read_text(encoding="utf-8").splitlines() if ln.strip()]
        self.assertEqual(len(lines), 1, "replay must not append a second line")

    def test_bad_crypto_event_rejected_and_nothing_written(self) -> None:
        """A structurally valid event (correct pubkey/kind/name) whose signature
        the verifier rejects must return 400 and leave approvals.jsonl empty."""
        bad_sig_event = dict(THROWAWAY_EVENT, sig="0" * 128)
        status, body = self._post("/api/approvals", bad_sig_event)
        self.assertEqual(status, 400)
        self.assertIn("signature", body["error"])
        path = self._root / "docs" / "internals" / "approvals.jsonl"
        self.assertFalse(path.exists(), "file must not be created when verifier rejects")

    def test_event_with_wrong_content_rejected(self) -> None:
        """Content must exactly match canonical markdown of the named term."""
        bad = dict(THROWAWAY_EVENT, content="tampered content")
        status, body = self._post("/api/approvals", bad)
        self.assertEqual(status, 400)
        self.assertIn("signed text is not the term", body["error"])

    def test_event_for_unknown_term_rejected(self) -> None:
        bad = dict(THROWAWAY_EVENT, tags=[["name", "NonExistentTerm"]])
        status, body = self._post("/api/approvals", bad)
        self.assertEqual(status, 400)
        self.assertIn("NonExistentTerm", body["error"])


if __name__ == "__main__":
    unittest.main()
