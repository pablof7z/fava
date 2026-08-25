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
from typing import Any

_TOOLS = str(Path(__file__).parents[1])
if _TOOLS not in sys.path:
    sys.path.insert(0, _TOOLS)
import vocabulary_approval as approval

EMPTY_STRUCTURE = {
    "private_architectural_state": [],
    "public_api": [],
    "reexports": [],
}

EMPTY_PACKET = {
    "interface": [],
    "review_problems": [],
    "structure": EMPTY_STRUCTURE,
}


def _markdown(term: dict, packet: dict = EMPTY_PACKET) -> str:
    if "structure" not in packet:
        packet = {"interface": [], "review_problems": [], "structure": packet}
    return approval.canonical_markdown(term, packet)

# Real nostr-crate-generated fixtures (throwaway keys only; owner key is
# unavailable here — owner approval is the Rust governance test's gate).
#
# Both events were produced by /private/tmp/claude-501/sigvec/src/main.rs and
# pass nostr::Event::verify().  They are used to exercise the *structural*
# checks that Python still owns (pubkey, kind, name tag) without re-implementing
# cryptography.

# secret key = scalar 1, canonical markdown of the "Event" term
THROWAWAY_EVENT = json.loads(
    '{"id":"89763d32a7e9bbc49e0a1b31d34a649c9df2f749772c737d73a1ae16e549f0a6",'
    '"pubkey":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",'
    '"created_at":1700000000,"kind":9999,'
    '"tags":[["name","Event"]],'
    '"content":"# Event\\n\\n**source**: nostr\\n\\n**protocol**: NIP-01\\n\\n'
    '**owner**: nostr\\n\\n**meaning**: A signed Nostr event.\\n\\n'
    '## Compiler-derived Rust structure\\n\\n```json\\n'
    '{\\"private_architectural_state\\":[],\\"public_api\\":[],\\"reexports\\":[]}'
    '\\n```\\n",'
    '"sig":"ea6db25a7ee8968d20fcd6e5e5f4a7259e12d477f469bf54c03cc9a0cacef495'
    '3af1739c90775c13a9fe329d0ef25a724c8450ff8210d3355b3fc2b3deacbf0f"}'
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
        md = _markdown({"name": "Foo"})
        self.assertTrue(md.startswith("# Foo\n"))

    def test_name_purpose_and_human_interface_lead_the_packet(self) -> None:
        packet = {
            "interface": [
                {
                    "kind": "Constructor",
                    "path": "probe::Thing::new",
                    "signature": "pub fn probe::Thing::new() -> Self",
                    "description": "Constructs one empty thing.",
                }
            ],
            "review_problems": [],
            "structure": EMPTY_STRUCTURE,
        }
        markdown = _markdown(
            {
                "name": "Thing",
                "meaning": "One useful thing.",
                "owner": "probe",
                "counterexample": "An unrelated value is not a Thing.",
            },
            packet,
        )
        self.assertTrue(markdown.startswith("# Thing\n\nOne useful thing.\n"))
        self.assertLess(markdown.index("## Human-readable interface"), markdown.index("## Edge and error semantics"))
        self.assertLess(markdown.index("## Edge and error semantics"), markdown.index("## Governance metadata"))
        self.assertLess(markdown.index("## Governance metadata"), markdown.index("## Deterministic compiler-derived structure"))
        self.assertIn("### Constructor `probe::Thing::new`", markdown)
        self.assertIn("Constructs one empty thing.", markdown)
        self.assertIn("pub fn probe::Thing::new() -> Self", markdown)

    def test_human_description_drift_invalidates_prior_content(self) -> None:
        term = {"name": "Thing", "meaning": "One thing."}
        prior = {
            "interface": [{
                "kind": "Method",
                "path": "probe::Thing::value",
                "signature": "pub fn probe::Thing::value(&self) -> usize",
                "description": "Returns the retained value.",
            }],
            "review_problems": [],
            "structure": EMPTY_STRUCTURE,
        }
        changed = json.loads(json.dumps(prior))
        changed["interface"][0]["description"] = "Returns the normalized value."
        event = {"id": "prior", "content": _markdown(term, prior)}
        self.assertIsNone(
            approval.authoritative_approval([event], _markdown(term, changed))
        )

    def test_prose_fields_appear_in_order(self) -> None:
        term = {
            "name": "T",
            "source": "fava",
            "meaning": "A thing.",
            "falsifier": "test must fail.",
        }
        md = _markdown(term)
        mean = md.index("A thing.")
        src = md.index("**source**")
        fals = md.index("**falsifier**")
        self.assertLess(mean, src)
        self.assertLess(src, fals)

    def test_empty_prose_field_is_omitted(self) -> None:
        term = {"name": "T", "source": "nostr", "meaning": ""}
        md = _markdown(term)
        self.assertIn("**source**", md)
        self.assertNotIn("**meaning**", md)

    def test_whitespace_only_prose_field_is_omitted(self) -> None:
        term = {"name": "T", "meaning": "   "}
        md = _markdown(term)
        self.assertNotIn("**meaning**", md)

    def test_empty_list_field_is_omitted(self) -> None:
        term = {"name": "T", "symbols": [], "crates": ["fava-foo"]}
        md = _markdown(term)
        self.assertNotIn("**symbols**", md)
        self.assertIn("**crates**", md)

    def test_list_items_are_sorted(self) -> None:
        term = {"name": "T", "symbols": ["b::B", "a::A"]}
        md = _markdown(term)
        self.assertLess(md.index("a::A"), md.index("b::B"))

    def test_extra_field_is_included(self) -> None:
        term = {"name": "T", "source": "fava", "custom_field": "custom_value"}
        md = _markdown(term)
        self.assertIn("**custom_field**: custom_value", md)

    def test_output_ends_with_single_newline(self) -> None:
        md = _markdown({"name": "T", "source": "nostr"})
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
            _markdown(term), _markdown(term)
        )

    def test_non_ascii_content_is_preserved(self) -> None:
        term = {"name": "T", "meaning": "Ünïcödé and emoji 🐍."}
        md = _markdown(term)
        self.assertIn("Ünïcödé and emoji 🐍", md)

    def test_backslash_and_quotes_are_preserved(self) -> None:
        term = {"name": "T", "meaning": 'back\\slash and "quotes"'}
        md = _markdown(term)
        self.assertIn('back\\slash and "quotes"', md)

    def test_event_term_includes_explicit_empty_compiler_structure(self) -> None:
        expected = (
            "# Event\n\n"
            "A signed Nostr event.\n\n"
            "## Human-readable interface\n\n"
            "No implemented Rust interface is bound to this term.\n\n"
            "## Governance metadata\n\n"
            "**source**: nostr\n\n"
            "**protocol**: NIP-01\n\n"
            "**owner**: nostr\n\n"
            "## Deterministic compiler-derived structure\n\n"
            "```json\n"
            '{"private_architectural_state":[],"public_api":[],"reexports":[]}\n'
            "```\n"
        )
        self.assertEqual(_markdown(EVENT_TERM), expected)

    def test_structural_drift_invalidates_prior_content(self) -> None:
        prior = _markdown(
            EVENT_TERM,
            {
                "private_architectural_state": [],
                "public_api": [
                    {"path": "fava::Event", "declaration": "pub struct fava::Event"}
                ],
                "reexports": [],
            },
        )
        changed = _markdown(
            EVENT_TERM,
            {
                "private_architectural_state": [],
                "public_api": [
                    {
                        "path": "fava::Event::id",
                        "declaration": "pub fava::Event::id: EventId",
                    },
                    {"path": "fava::Event", "declaration": "pub struct fava::Event"},
                ],
                "reexports": [],
            },
        )
        event = {"id": "prior", "content": prior}
        self.assertIsNone(approval.authoritative_approval([event], changed))


class ApprovalPageTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.html = (Path(_TOOLS) / "approve_vocabulary.html").read_text(
            encoding="utf-8"
        )

    def test_displays_the_exact_payload_submitted_to_signer(self) -> None:
        self.assertIn("payload.textContent = term.markdown;", self.html)
        self.assertIn("content: term.markdown", self.html)

    def test_renders_markdown_and_keeps_the_exact_raw_payload(self) -> None:
        self.assertIn("function renderMarkdown(markdown)", self.html)
        self.assertIn("splitReviewMarkdown(term.markdown)", self.html)
        self.assertIn("renderMarkdown(reviewSections.primary)", self.html)
        self.assertIn("Exact signed Markdown (raw)", self.html)
        self.assertNotIn("innerHTML", self.html)

    def test_has_no_multi_term_signing_path(self) -> None:
        self.assertNotIn("sign-all", self.html)
        self.assertNotIn("Sign all", self.html)

    def test_single_term_signing_connects_without_a_pause(self) -> None:
        self.assertNotIn("REVIEW_PAUSED", self.html)
        self.assertNotIn("Signing paused", self.html)
        self.assertIn("connect().then(load)", self.html)
        self.assertIn(
            'document.getElementById("connect").addEventListener("click", connect)',
            self.html,
        )

    def test_governance_and_machine_sections_are_secondary_details(self) -> None:
        self.assertIn("function splitReviewMarkdown(markdown)", self.html)
        self.assertIn("Governance and exact machine payload", self.html)

    def test_supports_owner_scoped_pending_review_links(self) -> None:
        self.assertIn("new URLSearchParams(window.location.search)", self.html)
        self.assertIn('term.owner === ownerFilter', self.html)
        self.assertIn('statusFilter === "pending"', self.html)
        self.assertIn(
            'term.status === "unapproved" || term.status === "stale"', self.html
        )


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


class LiveTermClassificationAuditTest(unittest.TestCase):
    """Regression: avoid classifying terms from unrelated symbol declarations."""

    @classmethod
    def setUpClass(cls) -> None:
        import tomllib

        root = Path(__file__).parents[2]
        registry = tomllib.loads(
            (root / "docs/internals/vocabulary.toml").read_text(encoding="utf-8")
        )
        cls.terms = {
            term["name"]: term
            for term in registry["term"]
            if isinstance(term, dict) and "name" in term
        }

    def _term(self, name: str) -> dict:
        return self.terms[name]

    def test_simple_group_uses_exact_symbol(self) -> None:
        term = self._term("SimpleGroup")
        self.assertEqual(approval.symbol_for_term(term), "fava_simple_groups::SimpleGroup")
        self.assertEqual(approval.item_kind_for_term(term, Path(__file__).parents[2]), "Struct")

    def test_simple_groups_crate_root_is_a_module(self) -> None:
        term = self._term("fava_simple_groups")
        self.assertEqual(approval.symbol_for_term(term), "fava_simple_groups")
        self.assertEqual(
            approval.item_kind_for_term(term, Path(__file__).parents[2]), "Module"
        )

    def test_saved_group_lists_is_a_function(self) -> None:
        term = self._term("saved_group_lists")
        self.assertEqual(
            approval.item_kind_for_term(term, Path(__file__).parents[2]), "Function"
        )

    def test_querysource_uses_trait_symbol(self) -> None:
        term = self._term("QuerySource")
        self.assertEqual(approval.symbol_for_term(term), "fava_query::QuerySource")
        self.assertEqual(approval.item_kind_for_term(term, Path(__file__).parents[2]), "Trait")

    def test_writestore_uses_trait_symbol(self) -> None:
        term = self._term("WriteStore")
        self.assertEqual(approval.symbol_for_term(term), "fava_write_store::WriteStore")
        self.assertEqual(approval.item_kind_for_term(term, Path(__file__).parents[2]), "Trait")

    def test_relayingest_does_not_borrow_relayingesterror(self) -> None:
        term = self._term("RelayIngest")
        self.assertEqual(approval.symbol_for_term(term), "RelayIngest")
        self.assertEqual(
            approval.item_kind_for_term(term, Path(__file__).parents[2]),
            "non-Rust Concept",
        )


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
        md = _markdown(term)
        evt = {"content": md}
        problems = approval.unapproved_terms((term,), {"Foo": [evt]})
        self.assertEqual(problems, [])

    def test_stale_approval_detected(self) -> None:
        """Editing a term after approval must be caught by content mismatch."""
        original = {"name": "Foo", "meaning": "original meaning"}
        md = _markdown(original)
        evt = {"content": md}

        modified = {"name": "Foo", "meaning": "CHANGED meaning"}
        problems = approval.unapproved_terms((modified,), {"Foo": [evt]})
        self.assertEqual(problems, ["Foo: changed since its approval was signed"])

    def test_stale_detected_after_list_field_change(self) -> None:
        original = {"name": "Foo", "symbols": ["foo::Bar"]}
        md = _markdown(original)
        evt = {"content": md}

        modified = {"name": "Foo", "symbols": ["foo::Bar", "foo::Baz"]}
        problems = approval.unapproved_terms((modified,), {"Foo": [evt]})
        self.assertIn("Foo: changed since its approval was signed", problems)

    def test_stale_detected_after_field_removed(self) -> None:
        original = {"name": "Foo", "meaning": "something", "distinction": "key detail"}
        md = _markdown(original)
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
            markdown = _markdown(term)
            for field in approval.EXPLICIT_CANDIDATE_FIELDS:
                with self.subTest(term=term["name"], field=field):
                    if field == "meaning":
                        self.assertTrue(
                            markdown.startswith(
                                f"# {term['name']}\n\n{approval.row_purpose(term)}\n"
                            )
                        )
                    else:
                        self.assertIn(f"**{field}**:", markdown)

    def test_parent_signature_cannot_approve_a_candidate(self) -> None:
        candidate = next(term for term in self.candidates if term["name"] == "QueryEvidence")
        parent = next(term for term in self.terms if term["name"] == "QuerySnapshot")
        approvals = {
            "QuerySnapshot": [{"content": _markdown(parent)}]
        }
        self.assertEqual(
            approval.unapproved_terms((candidate,), approvals),
            ["QueryEvidence: blocked candidate cannot be approved"],
        )

    def test_signature_must_match_the_exact_final_candidate(self) -> None:
        candidate = next(term for term in self.candidates if term["name"] == "QueryEvidence")
        stale = {"content": _markdown(candidate) + "changed\n"}
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
        event = {"content": _markdown(term)}
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

_MOCK_STRUCTURE_SRC = """\
import json
EMPTY_STRUCTURE = {
    "private_architectural_state": [],
    "public_api": [],
    "reexports": [],
}
def canonical_structure(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
def read_snapshot(path):
    raw = json.loads(path.read_text(encoding="utf-8"))
    return {
        entry["name"]: {
            "interface": entry["interface"],
            "review_problems": entry["review_problems"],
            "structure": entry["structure"],
        }
        for entry in raw["terms"]
    }, []
def snapshot_inputs_current(root, path):
    return path.is_file() and not (root / ".snapshot-inputs-stale").exists()
def compile_snapshot(root):
    return json.loads((root / "docs/internals/vocabulary-structure.json").read_text(encoding="utf-8"))
def render_snapshot(value):
    return json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\\n"
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
        '\n'
        '[[term]]\n'
        'name = "SimpleGroupStateEventKind"\n'
        'source = "probe"\n'
        'meaning = "Probe enum kind in local crate."\n'
        'symbols = ["fava_probe::SimpleGroupStateEventKind"]\n'
        'crates = ["fava-probe"]\n'
        '\n'
        '[[term]]\n'
        'name = "SavedGroupListMaterializer"\n'
        'source = "probe"\n'
        'meaning = "Probe private struct in local crate."\n'
        'owner = "fava-probe"\n'
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
        empty_structure = {
            "private_architectural_state": [],
            "public_api": [],
            "reexports": [],
        }
        snapshot = {
            "cargo_public_api": "mock",
            "format": 2,
            "inputs_sha256": "mock",
            "rustdoc_toolchain": "mock",
            "terms": [
                {
                    "name": name,
                    "interface": [],
                    "review_problems": [],
                    "structure": empty_structure,
                }
                for name in (
                    "Event",
                    "SavedGroupListMaterializer",
                    "SimpleGroupStateEventKind",
                )
            ],
        }
        (internals / "vocabulary-structure.json").write_text(
            json.dumps(snapshot, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

        probe_crate = self._root / "crates" / "fava-probe" / "src"
        probe_crate.mkdir(parents=True)
        (probe_crate / "lib.rs").write_text(
            "pub enum SimpleGroupStateEventKind {\n    Alpha,\n}\n"
            "struct SavedGroupListMaterializer;\n",
            encoding="utf-8",
        )

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
        (self._root / "vocabulary_structure.py").write_text(
            _MOCK_STRUCTURE_SRC, encoding="utf-8"
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

    def _post(self, path: str, body: Any) -> tuple[int, dict]:
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
        self.assertEqual(event["markdown"], _markdown(EVENT_TERM))
        self.assertEqual(event["rust_item"], "Event")
        self.assertEqual(event["rust_item_kind"], "non-Rust Concept")
        self.assertEqual(
            event["purpose"], "A signed Nostr event."
        )

    def test_rooted_enum_is_discovered_from_symbol_path(self) -> None:
        with self._get("/api/terms") as resp:
            payload = json.loads(resp.read())
        term = next(
            t for t in payload["terms"] if t["name"] == "SimpleGroupStateEventKind"
        )
        self.assertEqual(term["rust_item"], "fava_probe::SimpleGroupStateEventKind")
        self.assertEqual(term["rust_item_kind"], "Enum")

    def test_rooted_private_struct_is_discovered_by_owning_crate(self) -> None:
        with self._get("/api/terms") as resp:
            payload = json.loads(resp.read())
        term = next(
            t for t in payload["terms"] if t["name"] == "SavedGroupListMaterializer"
        )
        self.assertEqual(term["rust_item"], "SavedGroupListMaterializer")
        self.assertEqual(term["rust_item_kind"], "Struct")

    def test_get_blocks_unsigned_and_stales_signed_terms_on_input_drift(self) -> None:
        approval_event = dict(
            THROWAWAY_EVENT,
            id="1" * 64,
            content=_markdown(EVENT_TERM),
            tags=[["name", "Event"]],
        )
        approvals_path = self._root / "docs" / "internals" / "approvals.jsonl"
        approvals_path.write_text(json.dumps(approval_event) + "\n", encoding="utf-8")
        (self._root / ".snapshot-inputs-stale").write_text("stale", encoding="utf-8")

        with self._get("/api/terms") as resp:
            payload = json.loads(resp.read())

        self.assertFalse(payload["snapshot_inputs_current"])
        event = next(term for term in payload["terms"] if term["name"] == "Event")
        unsigned = next(
            term
            for term in payload["terms"]
            if term["name"] == "SimpleGroupStateEventKind"
        )
        self.assertEqual(event["status"], "stale")
        self.assertEqual(unsigned["status"], "blocked")
        self.assertTrue(
            any("compiler/doc inputs" in problem for problem in event["structural_problems"])
        )

    def test_wrong_path_returns_404(self) -> None:
        import urllib.error
        with self.assertRaises(urllib.error.HTTPError) as ctx:
            with self._get("/api/nonexistent"):
                pass
        self.assertEqual(ctx.exception.code, 404)
        ctx.exception.close()

    def test_single_term_signing_endpoint_accepts_and_persists_exact_event(self) -> None:
        status, body = self._post("/api/approvals", THROWAWAY_EVENT)
        self.assertEqual(status, 200, body)
        self.assertEqual(body["stored"], "Event")
        path = self._root / "docs" / "internals" / "approvals.jsonl"
        self.assertTrue(path.exists())
        stored = json.loads(path.read_text(encoding="utf-8").strip())
        self.assertEqual(stored["id"], THROWAWAY_EVENT["id"])

    def test_bulk_approval_body_is_rejected_without_writing(self) -> None:
        status, body = self._post(
            "/api/approvals", [THROWAWAY_EVENT, THROWAWAY_EVENT]
        )
        self.assertEqual(status, 400)
        self.assertIn("one event object", body["error"])
        path = self._root / "docs" / "internals" / "approvals.jsonl"
        self.assertFalse(path.exists())


if __name__ == "__main__":
    unittest.main()
