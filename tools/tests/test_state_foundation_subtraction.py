"""Structural subtraction proof for the approved relay/state/query foundation."""

from __future__ import annotations

import json
import pathlib
import re
import subprocess
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
RETIRED = (
    "RelayEvidence",
    "RelayObservation",
    "CachedEvent",
    "CacheMutation",
    "EventStateDecision",
    "StateSlice",
    "StateLookup",
    "CommittedCacheChange",
    "CacheMaintenance",
    "VerifiedRelayEvent",
    "SimpleGroupSnapshot",
    "GroupSnapshot",
)
SNAPSHOT_PATTERN = re.compile(
    r"SimpleGroupSnapshot|GroupSnapshot|deduplicate_events|\.project\(|"
    r"metadata_differ|admins_differ|members_differ|roles_differ|"
    r"participants_differ|pins_differ"
)
HISTORICAL_SNAPSHOT_PATHS = {
    ".planning/audit/2026-08-23/identity-protocols.md",
    ".planning/audit/2026-08-23/public-surface.md",
    ".planning/audit/2026-08-23/query-state-cache.md",
    ".planning/audit/2026-08-23/vocabulary.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-02-PLAN.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-02-SUMMARY.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-07-SUMMARY.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-08-PLAN.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-08-SUMMARY.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-09-SUMMARY.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-12-PLAN.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-CONTEXT.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-PATTERNS.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-RESEARCH.md",
    ".planning/phases/07.1.1-deliver-fava-simple-groups-as-the-multi-relay-nip-29-capabil/07.1.1-REVIEW-FIX.capability-bounds.md",
}
PRIVATE_CALLERS = (
    "crates/fava-nip02/src/query.rs",
    "crates/fava-simple-groups/src/query.rs",
    "crates/fava/tests/publication_door.rs",
    "falsifiers/external-semantic-capability/tests/public_capability.rs",
    "falsifiers/external-semantic-capability/tests/support/waits.rs",
)


def json_strings(value: object):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for nested in value.values():
            yield from json_strings(nested)
    elif isinstance(value, list):
        for nested in value:
            yield from json_strings(nested)


class StateFoundationSubtraction(unittest.TestCase):
    def test_retired_rust_surface_is_absent(self) -> None:
        offenders: list[str] = []
        for base in (ROOT / "crates", ROOT / "apps", ROOT / "falsifiers"):
            for path in base.rglob("*.rs"):
                if "tests" in path.parts:
                    continue
                text = path.read_text()
                for retired in RETIRED:
                    if retired in text:
                        offenders.append(f"{path.relative_to(ROOT)}: {retired}")
        self.assertEqual([], offenders)

    def test_event_record_is_private_and_every_known_caller_uses_accessors(self) -> None:
        source = (ROOT / "crates/fava-query/src/lib.rs").read_text()
        body = re.search(r"pub struct EventRecord \{(?P<body>.*?)\n\}", source, re.S)
        self.assertIsNotNone(body)
        self.assertNotRegex(body.group("body"), r"\bpub\s+\w+\s*:")
        self.assertIn("RelayOccurrenceEventMismatch", source)
        field_access = re.compile(r"\brecord\.(?:event|publication|relay_occurrences)\b(?!\s*\()")
        for relative in PRIVATE_CALLERS:
            self.assertNotRegex((ROOT / relative).read_text(), field_access, relative)

    def test_snapshot_family_has_exact_historical_hidden_manifest(self) -> None:
        actual: set[str] = set()
        for root in ("apps", "crates", "docs", ".planning", ".bg-shell"):
            for path in (ROOT / root).rglob("*"):
                if not path.is_file() or "target" in path.parts:
                    continue
                if path.name == "07.1.1-VALIDATION.md" or "tests" in path.parts:
                    continue
                try:
                    text = path.read_text()
                except UnicodeDecodeError:
                    continue
                if SNAPSHOT_PATTERN.search(text):
                    actual.add(str(path.relative_to(ROOT)))
        self.assertEqual(HISTORICAL_SNAPSHOT_PATHS, actual)
        for relative in actual:
            text = (ROOT / relative).read_text()
            self.assertIn("Superseded by STATE-ARCH-1", text, relative)
            self.assertIn("not current implementation guidance", text, relative)

    def test_current_docs_and_state_have_no_retired_architecture(self) -> None:
        current = [
            *sorted((ROOT / "docs/spec").glob("*.md")),
            ROOT / ".planning/STATE.md",
            ROOT / ".planning/REQUIREMENTS.md",
            ROOT / ".planning/codebase/ARCHITECTURE.md",
            ROOT / ".planning/codebase/CONCERNS.md",
            ROOT / "crates/fava-simple-groups/README.md",
        ]
        offenders = []
        for path in current:
            text = path.read_text()
            for retired in RETIRED:
                if retired in text:
                    offenders.append(f"{path.relative_to(ROOT)}: {retired}")
        self.assertEqual([], offenders)

    def test_jsonl_catalogs_parse_and_have_no_projection_model(self) -> None:
        forbidden = re.compile(
            r"SimpleGroupSnapshot|GroupSnapshot|SimpleGroup::project|snapshot\.rs|"
            r"host slot|projection (?:uses|needs|assigns)|same-id.{0,30}(?:merge|dedup)|"
            r"merged relay evidence|newest (?:admin|metadata|member|role|pin|participant) record|"
            r"selected independently per host",
            re.I,
        )
        for relative in (".bg-shell/simple-groups-semantic-catalog.jsonl",):
            for number, line in enumerate((ROOT / relative).read_text().splitlines(), 1):
                value = json.loads(line)
                for string in json_strings(value):
                    self.assertNotRegex(string, forbidden, f"{relative}:{number}: {string}")

    def test_proto_006_and_live_retention_state_positive_truth(self) -> None:
        goals = (ROOT / "docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md").read_text()
        self.assertIn("use the selected relays as result-provenance authority", goals)
        self.assertIn("The capability MUST NOT impose a private result limit", goals)
        self.assertIn("State-event decoders MUST check only the expected kind", goals)
        architecture = (ROOT / "docs/spec/ARCHITECTURE.md").read_text()
        self.assertIn("4,096 events per exact `RelaySessionKey`", architecture)
        self.assertIn("LiveRetentionLimit", architecture)
        self.assertIn("not a Nostr protocol rule", architecture)
        self.assertIn("Pablo may\noverrule it before merge", architecture)
        self.assertIn("cache refusal never", architecture.lower())

    def test_exact_relay_authority_proof_is_rustfmt_2024_and_256(self) -> None:
        compiled = (ROOT / "crates/fava-simple-groups/tests/exact_host_records.rs").read_text()
        self.assertIn("let relays = (0..256)", compiled)
        self.assertIn("&ResultAuthority::OnlyRelays(expected)", compiled)
        self.assertIn("assert_eq!(query.result_limit(), None);", compiled)
        with tempfile.NamedTemporaryFile(mode="w", suffix=".rs") as source:
            source.write(compiled)
            source.flush()
            result = subprocess.run(
                ["rustfmt", "--edition", "2024", "--check", source.name],
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertEqual(0, result.returncode, result.stderr)

    def test_syntax_aware_comparator_is_the_only_textual_proof(self) -> None:
        source = (ROOT / "crates/fava-state/tests/downstream_comparator_subtraction.rs").read_text()
        self.assertIn("syn::parse_file", source)
        self.assertIn("normalized signature", source)
        self.assertIn("expected_manifest", source)
        self.assertIn("repository_discovery_and_exact_manifest", source)
        self.assertIn("workspace_members", source)
        self.assertIn("self.helper", source)
        self.assertIn("unmanifested comparator callers", source)
        self.assertIn("arbitrary timestamp/id comparison", source)
        self.assertIn("controlled_owner_sinks", source)
        self.assertIn("expected_controlled_sink_manifest", source)
        self.assertIn("exact owner-controlled insertion/refusal sink manifest", source)
        self.assertIn("NoLocalSelection module", source)
        self.assertIn("expected_non_winner_ordering_manifest", source)
        self.assertIn("arbitrary_timestamp_or_id_ordering", source)
        self.assertIn("call_form_ordering_is_governed", source)
        self.assertIn("helper_resolution_rejects_module_import_and_signature_decoys", source)
        self.assertIn("unrelated_branch_effect_is_not_an_owner_controlled_sink", source)
        self.assertIn("renamed_tuples_alias_and_genuine_owner_call", source)
        self.assertIn("dead owner calls are not proof", source)
        self.assertNotIn('source.contains("event_is_newer")', source)

    def test_nip65_composes_query_ownership_without_local_selection(self) -> None:
        source = (ROOT / "crates/fava-nip65/src/lib.rs").read_text()
        self.assertIn("pub fn relay_lists", source)
        self.assertNotIn("fn supersedes", source)
        self.assertIn("WrongKind {", source)
        self.assertTrue((ROOT / "crates/fava-nip65/README.md").exists())
        self.assertTrue((ROOT / "crates/fava-nip65/tests/decoder.rs").exists())

    def test_access_lifecycle_and_overflow_retraction_proofs_are_causal(self) -> None:
        access = (ROOT / "crates/fava/tests/access_isolation.rs").read_text()
        self.assertIn(
            "query_access_survives_facade_planner_transport_observation_lifecycle", access
        )
        self.assertIn("plan_is_exact", access)
        self.assertIn("private_peer.reconnect()", access)
        self.assertIn("transport.holders(&public_key).is_none()", access)

        registry = (ROOT / "crates/fava-observe/src/registry.rs").read_text()
        clear = registry.index("live.retractions.clear();")
        refusal = registry.index("if next_events.len() > LIVE_EVENTS_PER_SESSION.get()")
        self.assertLess(clear, refusal)

        bound = (ROOT / "crates/fava-observe/tests/relay_occurrence_bound.rs").read_text()
        self.assertIn("saw_live_retraction(replaceable.id)", bound)
        self.assertIn("saw_live_retraction(deleted_target.id)", bound)
        self.assertIn("overflow refusal must preserve the complete accepted live state", bound)
        self.assertIn("refused overflow revision must not repeat", bound)

    def test_cache_contract_remains_deferred_despite_live_bound_amendment(self) -> None:
        issue = (ROOT / "docs/issues/0023-state-foundation.md").read_text().lower()
        self.assertIn("exact cache contract", issue)
        self.assertIn("remain deferred", issue)
        self.assertIn("4,096", issue)
        self.assertRegex(issue, r"pablo may\s+overrule")


if __name__ == "__main__":
    unittest.main()
