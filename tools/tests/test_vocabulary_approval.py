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


class HiddenVocabularyTest(unittest.TestCase):
    def test_exposes_every_differently_named_symbol_occurrence(self) -> None:
        terms = [
            {
                "name": "Planner",
                "symbols": ["fava::Planner", "fava::ShortfallReason"],
                "spec_symbols": ["Planner", "PlanResult"],
            }
        ]
        hidden = approval.hidden_vocabulary(terms)
        self.assertEqual(set(hidden), {"ShortfallReason", "PlanResult"})
        self.assertEqual(hidden["ShortfallReason"][0]["parent"], "Planner")
        self.assertEqual(hidden["ShortfallReason"][0]["field"], "symbols")
        self.assertEqual(hidden["PlanResult"][0]["field"], "spec_symbols")

    def test_same_terminal_name_is_not_hidden(self) -> None:
        terms = [
            {
                "name": "Query",
                "symbols": ["fava::Query", "other::Query"],
                "spec_symbols": ["Query"],
            }
        ]
        self.assertEqual(approval.hidden_vocabulary(terms), {})

    def test_parent_structural_problems_name_exact_locations(self) -> None:
        term = {
            "name": "SubscriptionPlanner",
            "symbols": ["fava::ShortfallReason"],
        }
        hidden = approval.hidden_vocabulary([term])
        self.assertEqual(
            approval.structural_problems_for_term(term, hidden),
            ["symbols: fava::ShortfallReason"],
        )


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


class ReviewFieldsTest(unittest.TestCase):
    def test_returns_canonical_fields_without_markdown_syntax(self) -> None:
        term = {
            "name": "T",
            "source": "fava",
            "meaning": "A thing.",
            "symbols": ["z::Z", "a::A"],
        }
        self.assertEqual(
            approval.review_fields(term),
            [
                {"name": "source", "value": "fava"},
                {"name": "meaning", "value": "A thing."},
                {"name": "symbols", "value": ["a::A", "z::Z"]},
            ],
        )

    def test_rejects_invalid_values_like_canonical_markdown(self) -> None:
        with self.assertRaisesRegex(ValueError, "meaning.*str"):
            approval.review_fields({"name": "T", "meaning": ["not text"]})


class RowPresentationTest(unittest.TestCase):
    def test_item_kind_prefers_real_rust_symbols(self) -> None:
        term = {
            "name": "Follow",
            "symbols": ["fava_simple_groups::SimpleGroup", "fava_nip02::Follow"],
            "meaning": "A row result from NIP-02.",
        }
        self.assertEqual(
            approval.item_kind_for_term(term),
            "Struct",
        )

    def test_item_kind_falls_back_to_spec_symbol(self) -> None:
        term = {
            "name": "KindHint",
            "spec_symbols": ["SpecKind"],
            "meaning": "placeholder",
        }
        self.assertEqual(approval.item_kind_for_term(term), "non-Rust Concept")

    def test_item_kind_falls_back_to_name_when_no_symbol(self) -> None:
        term = {"name": "Event", "meaning": "A signed event."}
        self.assertEqual(approval.item_kind_for_term(term), "non-Rust Concept")

    def test_symbol_for_term_prefers_real_rust_symbols(self) -> None:
        term = {
            "name": "SimpleGroup",
            "symbols": ["fava_simple_groups::SimpleGroup"],
            "meaning": "A relay-based Nostr group.",
        }
        self.assertEqual(
            approval.symbol_for_term(term),
            "fava_simple_groups::SimpleGroup",
        )

    def test_row_purpose_is_single_line(self) -> None:
        term = {"meaning": "A line.\n with extra   spacing."}
        self.assertEqual(approval.row_purpose(term), "A line. with extra spacing.")


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
        self.assertEqual(approvals["Query"], [event])
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

    def test_signed_history_is_preserved_in_timestamp_order(self) -> None:
        base = {
            "id": "x", "pubkey": approval.OWNER, "kind": 9999,
            "tags": [["name", "Query"]], "content": "v1", "sig": "y",
        }
        old = dict(base, created_at=1_000_000)
        new = dict(base, id="z", created_at=2_000_000, content="v2")
        with tempfile.TemporaryDirectory() as d:
            path = self._write(Path(d), old, new)
            approvals, _ = approval.load_approvals(path)
        self.assertEqual([event["content"] for event in approvals["Query"]], ["v1", "v2"])

    def test_authoritative_approval_requires_exact_current_markdown(self) -> None:
        events = [
            {"created_at": 1, "id": "old", "content": "old"},
            {"created_at": 2, "id": "final", "content": "final"},
        ]
        self.assertEqual(
            approval.authoritative_approval(events, "final")["id"], "final"
        )
        self.assertIsNone(approval.authoritative_approval(events, "different"))

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
        problems = approval.unapproved_terms((term,), {"Foo": [evt]})
        self.assertEqual(problems, [])

    def test_stale_approval_detected(self) -> None:
        """Editing a term after approval must be caught by content mismatch."""
        original = {"name": "Foo", "meaning": "original meaning"}
        md = approval.canonical_markdown(original)
        evt = {"content": md}

        modified = {"name": "Foo", "meaning": "CHANGED meaning"}
        problems = approval.unapproved_terms((modified,), {"Foo": [evt]})
        self.assertEqual(problems, ["Foo: changed since its approval was signed"])

    def test_stale_detected_after_list_field_change(self) -> None:
        original = {"name": "Foo", "symbols": ["foo::Bar"]}
        md = approval.canonical_markdown(original)
        evt = {"content": md}

        modified = {"name": "Foo", "symbols": ["foo::Bar", "foo::Baz"]}
        problems = approval.unapproved_terms((modified,), {"Foo": [evt]})
        self.assertIn("Foo: changed since its approval was signed", problems)

    def test_stale_detected_after_field_removed(self) -> None:
        original = {"name": "Foo", "meaning": "something", "distinction": "key detail"}
        md = approval.canonical_markdown(original)
        evt = {"content": md}

        without_distinction = {"name": "Foo", "meaning": "something"}
        problems = approval.unapproved_terms((without_distinction,), {"Foo": [evt]})
        self.assertIn("Foo: changed since its approval was signed", problems)


class CandidateCoverageTest(unittest.TestCase):
    """Requested names have explicit, independently reviewable research."""

    REQUESTED_NAMES = {
        "DesiredPlanEvidence",
        "QueryEvidence",
        "RelayEvidence",
        "RelayQueryEvidence",
        "SourceEvidence",
    }

    @classmethod
    def setUpClass(cls) -> None:
        import tomllib

        cls.root = Path(__file__).parents[2]
        registry = tomllib.loads(
            (cls.root / "docs/internals/vocabulary.toml").read_text(encoding="utf-8")
        )
        cls.terms = list(registry["term"])
        cls.hidden = approval.hidden_vocabulary(cls.terms)
        cls.research, research_problems = approval.load_candidate_research(
            cls.root / approval.CANDIDATES_PATH
        )
        cls.candidates, candidate_problems = approval.candidate_terms(
            cls.terms, cls.research, cls.root
        )
        cls.problems = [*research_problems, *candidate_problems]

    def test_every_hidden_name_has_one_researched_candidate(self) -> None:
        self.assertEqual(self.problems, [])
        self.assertTrue(
            self.REQUESTED_NAMES.issubset(
                {term["name"] for term in self.candidates}
            )
        )

    def test_requested_old_names_are_blocked_with_independent_proposals(self) -> None:
        self.assertEqual(
            {
                term["name"]: term["disposition"]
                for term in self.candidates
                if term["name"] in self.REQUESTED_NAMES
            },
            {
                "DesiredPlanEvidence": "blocked",
                "QueryEvidence": "blocked",
                "RelayEvidence": "blocked",
                "RelayQueryEvidence": "blocked",
                "SourceEvidence": "blocked",
            },
        )
        self.assertEqual(
            {
                term["name"]: term["proposed_disposition"]
                for term in self.candidates
                if term["name"] in self.REQUESTED_NAMES
            },
            {
                "DesiredPlanEvidence": "remove",
                "QueryEvidence": "retain as QueryResultStatus",
                "RelayEvidence": "retain as EventRelayObservations",
                "RelayQueryEvidence": "retain as QueryRelayStatus",
                "SourceEvidence": "retain as SourceContributionState",
            },
        )

    def test_no_candidate_has_a_missing_or_placeholder_field(self) -> None:
        for term in self.candidates:
            for field in approval.REQUIRED_CANDIDATE_FIELDS:
                with self.subTest(term=term["name"], field=field):
                    self.assertIsInstance(term[field], str)
                    self.assertTrue(term[field].strip())
                    self.assertNotIn("No independently described term", term[field])

    def test_every_candidate_markdown_contains_the_complete_review_packet(self) -> None:
        for term in self.candidates:
            markdown = approval.canonical_markdown(term)
            for field in approval.EXPLICIT_CANDIDATE_FIELDS:
                with self.subTest(term=term["name"], field=field):
                    self.assertIn(f"**{field}**:", markdown)

    def test_parent_signature_cannot_approve_a_candidate(self) -> None:
        candidate = next(term for term in self.candidates if term["name"] == "QueryEvidence")
        parent = next(term for term in self.terms if term["name"] == "QuerySnapshot")
        approvals = {
            "QuerySnapshot": [{"content": approval.canonical_markdown(parent)}]
        }
        self.assertEqual(
            approval.unapproved_terms((candidate,), approvals),
            ["QueryEvidence: blocked candidate cannot be approved"],
        )

    def test_signature_must_match_the_exact_final_candidate(self) -> None:
        candidate = next(term for term in self.candidates if term["name"] == "QueryEvidence")
        stale = {"content": approval.canonical_markdown(candidate) + "changed\n"}
        self.assertEqual(
            approval.unapproved_terms((candidate,), {"QueryEvidence": [stale]}),
            ["QueryEvidence: blocked candidate cannot be approved"],
        )

    def test_approval_app_dump_contains_every_complete_candidate(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(self.root / "tools/approve_vocabulary.py"),
                "--root",
                str(self.root),
                "--dump-candidates-json",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        self.assertTrue(
            self.REQUESTED_NAMES.issubset({row["name"] for row in payload})
        )
        self.assertFalse(
            any("No independently described term" in row["markdown"] for row in payload)
        )

    def test_missing_research_cannot_quiet_discovery(self) -> None:
        terms = [{
            "name": "Planner",
            "source": "fava",
            "owner": "fava-planner",
            "nearest_nostr": "Filter",
            "meaning": "Plans filters.",
            "symbols": ["fava_planner::ShortfallReason"],
            "crates": ["fava-planner"],
        }]
        candidates, problems = approval.candidate_terms(terms, {}, self.root)
        self.assertEqual(candidates, [])
        self.assertEqual(
            problems, ["ShortfallReason: missing researched vocabulary candidate"]
        )

    def test_unrelated_evidence_cannot_create_a_signable_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/fava-planner/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("pub struct DifferentName;\n", encoding="utf-8")
            terms = [{
                "name": "Planner",
                "source": "fava",
                "owner": "fava-planner",
                "nearest_nostr": "Filter",
                "meaning": "Plans filters.",
                "symbols": ["fava_planner::ShortfallReason"],
                "crates": ["fava-planner"],
            }]
            research = {
                "ShortfallReason": {
                    "name": "ShortfallReason",
                    "disposition": "candidate",
                    "category": "state",
                    "disposition": "candidate",
                    "proposed_disposition": "retain as DemandShortfall",
                    "meaning": "Why demand was not planned.",
                    "evidence": "crates/fava-planner/src/lib.rs:1",
                    "owner": "The planner creates it for one demand and retains it in that plan.",
                    "nearest_nostr": "REQ filter",
                    "distinction": "It explains omitted logical demand without failing the rest of the plan.",
                    "counterexample": "A malformed duplicate demand refuses the whole planning call instead.",
                    "lifecycle": "Created during one plan revision, carried with that revision, then superseded.",
                    "forcing_requirement": "RELAY-004 forbids claiming omitted demand completed.",
                    "falsifier": "Remove typed omission and run `cargo test -p fava-subscriptions`; the shortfall test must fail.",
                }
            }
            candidates, problems = approval.candidate_terms(terms, research, root)
        self.assertEqual(candidates, [])
        self.assertEqual(
            problems,
            [
                "ShortfallReason: first evidence line does not name the candidate: "
                "crates/fava-planner/src/lib.rs:1"
            ],
        )


class CandidateResearchQualityTest(unittest.TestCase):
    def _record(self, name: str = "QueryFacts") -> dict[str, str]:
        return {
            "name": name,
            "category": "value",
            "disposition": "candidate",
            "proposed_disposition": f"retain as {name}",
            "evidence": "crates/fava-query/src/evidence.rs:353",
            "owner": "The query evaluator builds it for one snapshot revision; the observation retains it until the next revision.",
            "nearest_nostr": "REQ, EOSE, CLOSED, and relay provenance",
            "meaning": "Scoped evidence explaining the acquisition state behind one query result.",
            "distinction": "It combines per-source revisions, per-relay terminal facts, desired-plan coverage, and shortfalls without asserting global completeness.",
            "counterexample": "An event's relay URL proves where that event was seen but says nothing about another requested relay reaching EOSE.",
            "lifecycle": "The evaluator constructs a fresh immutable value from one coherent source and relay input revision; the next QuerySnapshot supersedes it.",
            "forcing_requirement": "QUERY-008 through QUERY-010 require scoped attribution while forbidding global completeness claims.",
            "falsifier": "Merge every relay into one completion bit, then run `cargo test -p fava-query --test query_evidence`; relay-scoping assertions must fail.",
        }

    def _load(self, records: list[dict[str, str]]) -> tuple[dict[str, dict[str, str]], list[str]]:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "candidates.jsonl"
            path.write_text(
                "".join(json.dumps(record) + "\n" for record in records),
                encoding="utf-8",
            )
            return approval.load_candidate_research(path)

    def test_rejects_the_old_generated_prose(self) -> None:
        record = self._record()
        record["counterexample"] = "For QueryFacts: the underlying event is not evidence."
        _, problems = self._load([record])
        self.assertTrue(any("contains generated prose" in problem for problem in problems))

    def test_rejects_existence_as_the_forcing_requirement(self) -> None:
        record = self._record()
        record["forcing_requirement"] = "The authoritative surface names QueryEvidence independently."
        _, problems = self._load([record])
        self.assertTrue(any("behavioral authority" in problem for problem in problems))

    def test_rejects_a_non_executable_falsifier(self) -> None:
        record = self._record()
        record["falsifier"] = "Delete the research record and expect a review problem."
        _, problems = self._load([record])
        self.assertTrue(any("executable test command" in problem for problem in problems))

    def test_rejects_duplicate_candidate_prose(self) -> None:
        first = self._record()
        second = self._record("OtherFacts")
        second["evidence"] = "crates/fava-query/src/evidence.rs:353"
        _, problems = self._load([first, second])
        self.assertTrue(any("duplicates QueryFacts" in problem for problem in problems))

    def test_generic_evidence_name_must_remain_blocked(self) -> None:
        record = self._record("RelayQueryEvidence")
        _, problems = self._load([record])
        self.assertTrue(any("generic Evidence name must remain blocked" in problem for problem in problems))

    def test_blocked_candidate_remains_unsigned_even_with_exact_content(self) -> None:
        term = self._record("RelayQueryEvidence")
        term["disposition"] = "blocked"
        event = {"content": approval.canonical_markdown(term)}
        self.assertEqual(
            approval.unapproved_terms((term,), {term["name"]: [event]}),
            ["RelayQueryEvidence: blocked candidate cannot be approved"],
        )


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
        (internals / "vocabulary-candidates.jsonl").write_text("", encoding="utf-8")

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
        event = next(term for term in payload["terms"] if term["name"] == "Event")
        self.assertEqual(event["review"], [
            {"name": "source", "value": "nostr"},
            {"name": "protocol", "value": "NIP-01"},
            {"name": "owner", "value": "nostr"},
            {"name": "meaning", "value": "A signed Nostr event."},
        ])
        self.assertEqual(event["rust_item"], "Event")
        self.assertEqual(event["rust_item_kind"], "non-Rust Concept")
        self.assertEqual(
            event["purpose"], "A signed Nostr event."
        )

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

    def test_final_signature_appends_beside_stale_signed_history(self) -> None:
        path = self._root / "docs" / "internals" / "approvals.jsonl"
        stale = dict(
            THROWAWAY_EVENT,
            id="1" * 64,
            created_at=1,
            content="older candidate markdown",
            sig="1" * 128,
        )
        path.write_text(json.dumps(stale) + "\n", encoding="utf-8")

        status, body = self._post("/api/approvals", THROWAWAY_EVENT)
        self.assertEqual(status, 200, body)
        lines = [line for line in path.read_text(encoding="utf-8").splitlines() if line]
        self.assertEqual(len(lines), 2)
        self.assertEqual(json.loads(lines[0])["content"], "older candidate markdown")
        self.assertEqual(json.loads(lines[1])["id"], THROWAWAY_EVENT["id"])

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
