#!/usr/bin/env python3
"""Signed approval of vocabulary terms.

One approval is a Nostr kind-9999 event whose content is the canonical
markdown of exactly one registry term and whose `name` tag carries that
term's name. The signature binds the term's full text, so editing a term
after approval invalidates it.

Cryptographic verification (event-id hash and Schnorr signature) is
performed by the Rust governance test via nostr::Event::verify(). Python
handles only parsing, rendering, and serving.
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

OWNER = "fa984bd7dbb282f07e16e7ae87b26a2a7b9b90b7246a44771f0cf5ae58018f52"
APPROVAL_KIND = 9999
APPROVALS_PATH = Path("docs/internals/approvals.jsonl")
CANDIDATES_PATH = Path("docs/internals/vocabulary-candidates.jsonl")

PROSE_FIELDS = (
    "source",
    "evidence",
    "disposition",
    "proposed_disposition",
    "protocol",
    "owner",
    "nearest_nostr",
    "meaning",
    "distinction",
    "counterexample",
    "lifecycle",
    "forcing_requirement",
    "falsifier",
)
LIST_FIELDS = ("symbols", "crates", "spec_symbols", "spec_crates")

REQUIRED_CANDIDATE_FIELDS = (
    "owner",
    "nearest_nostr",
    "meaning",
    "distinction",
    "counterexample",
    "lifecycle",
    "forcing_requirement",
    "falsifier",
)

EXPLICIT_CANDIDATE_FIELDS = (
    "disposition",
    "proposed_disposition",
    *REQUIRED_CANDIDATE_FIELDS,
)
BASE_CANDIDATE_FIELDS = ("name", "evidence")
CANDIDATE_METADATA_FIELDS = ("category",)
DISPOSITIONS = {"candidate", "blocked"}

EVIDENCE_LOCATION = re.compile(r"^(?P<path>[^:]+):(?P<line>[1-9][0-9]*)$")

# These phrases identify the rejected category generator, not researched
# architecture. Keeping them here makes regression to superficially complete
# prose a hard error instead of a review-time discovery.
GENERIC_PROSE = (
    re.compile(r"\bFor [A-Za-z][A-Za-z0-9_]*:"),
    re.compile(r"\bThe [A-Za-z][A-Za-z0-9_]* lifecycle:"),
    re.compile(r"\bauthoritative surface names\b", re.IGNORECASE),
    re.compile(r"\bclosed vocabulary therefore\b", re.IGNORECASE),
    re.compile(r"\bindependently named .+ described above\b", re.IGNORECASE),
    re.compile(r"\bDelete or blank the .+ research record\b", re.IGNORECASE),
    re.compile(r"\bCandidateCoverageTest\b"),
)

BEHAVIORAL_REQUIREMENT = re.compile(
    r"\b(?:GOALS|ARCH|QUERY|WRITE|RELAY|CORE|OBS|OPS|CACHE|GROUP|PROTO|RUNTIME|NIP)-?\d*\b",
    re.IGNORECASE,
)
EXECUTABLE_COMMAND = re.compile(r"`(?:cargo test|python3 -m unittest)[^`]*`")
DELIBERATE_BREAK = re.compile(
    r"^(?!Delete or blank).{12,}`cargo test",
    re.IGNORECASE,
)


def hidden_vocabulary(terms: list[dict[str, Any]]) -> dict[str, list[dict[str, str]]]:
    """Every differently named concept concealed in symbols/spec_symbols.

    Results are keyed by the hidden terminal nominal name. Each occurrence
    retains its parent term, registry field, and exact registry value so the
    approval UI exposes the complete surface rather than only a count.
    """
    hidden: dict[str, list[dict[str, str]]] = {}
    for term in terms:
        parent = term.get("name")
        if not isinstance(parent, str):
            continue
        for field in ("symbols", "spec_symbols"):
            values = term.get(field, [])
            if not isinstance(values, list):
                continue
            for value in values:
                if not isinstance(value, str) or not value.strip():
                    continue
                terminal = value.rsplit("::", maxsplit=1)[-1]
                if terminal == parent:
                    continue
                hidden.setdefault(terminal, []).append(
                    {"parent": parent, "field": field, "value": value}
                )
    return hidden


def structural_problems_for_term(
    term: dict[str, Any], hidden: dict[str, list[dict[str, str]]]
) -> list[str]:
    """Hidden concepts wrongly filed under one term."""
    name = term.get("name")
    return [
        f"{occurrence['field']}: {occurrence['value']}"
        for occurrences in hidden.values()
        for occurrence in occurrences
        if occurrence["parent"] == name
    ]


def reviewable_vocabulary_names(terms: list[dict[str, Any]]) -> set[str]:
    """Hidden names plus registered generic Evidence names Pablo challenged."""
    return set(hidden_vocabulary(terms)) | {
        str(term["name"])
        for term in terms
        if isinstance(term.get("name"), str) and "Evidence" in term["name"]
    }


def load_candidate_research(path: Path) -> tuple[dict[str, dict[str, str]], list[str]]:
    """Load the explicit research record for every proposed hidden name."""
    records: dict[str, dict[str, str]] = {}
    problems: list[str] = []
    if not path.exists():
        return records, [f"cannot read candidate research: {path}"]
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            raw = json.loads(line)
        except json.JSONDecodeError as error:
            problems.append(f"{path}:{number}: unreadable candidate: {error}")
            continue
        if not isinstance(raw, dict):
            problems.append(f"{path}:{number}: candidate must be an object")
            continue
        required = set(BASE_CANDIDATE_FIELDS)
        missing = sorted(required - raw.keys())
        if missing:
            problems.append(
                f"{path}:{number}: candidate is missing: {', '.join(missing)}"
            )
            continue
        if any(
            not isinstance(raw[field], str) or not raw[field].strip()
            for field in required
        ):
            problems.append(f"{path}:{number}: candidate fields must be non-empty text")
            continue
        name = raw["name"]
        if name in records:
            problems.append(f"{path}:{number}: duplicate candidate name: {name}")
            continue
        if "disposition" not in raw:
            records[name] = raw
            continue
        required.update(EXPLICIT_CANDIDATE_FIELDS)
        incomplete = sorted(
            field
            for field in required
            if not isinstance(raw.get(field), str) or not raw[field].strip()
        )
        if incomplete:
            problems.append(
                f"{path}:{number}: {name}: incomplete candidate: "
                f"{', '.join(incomplete)}"
            )
            continue
        if raw["disposition"] not in DISPOSITIONS:
            problems.append(
                f"{path}:{number}: {name}: disposition must be candidate or blocked"
            )
            continue
        if "Evidence" in name and raw["disposition"] != "blocked":
            problems.append(
                f"{path}:{number}: {name}: generic Evidence name must remain blocked "
                "until an exact established concept overcomes the naming objection"
            )
            continue
        unsupported = sorted(
            key
            for key, value in raw.items()
            if key not in {
                *BASE_CANDIDATE_FIELDS,
                *CANDIDATE_METADATA_FIELDS,
                *EXPLICIT_CANDIDATE_FIELDS,
            }
            or not isinstance(value, str)
        )
        if unsupported:
            problems.append(
                f"{path}:{number}: {name}: unsupported candidate fields: "
                f"{', '.join(unsupported)}"
            )
            continue
        for field in REQUIRED_CANDIDATE_FIELDS:
            value = raw[field]
            matched = next(
                (pattern.pattern for pattern in GENERIC_PROSE if pattern.search(value)),
                None,
            )
            if matched:
                problems.append(
                    f"{path}:{number}: {name}: {field} contains generated prose: {matched}"
                )
        if not BEHAVIORAL_REQUIREMENT.search(raw["forcing_requirement"]):
            problems.append(
                f"{path}:{number}: {name}: forcing_requirement must cite a behavioral authority"
            )
        falsifier = raw["falsifier"]
        if not EXECUTABLE_COMMAND.search(falsifier):
            problems.append(
                f"{path}:{number}: {name}: falsifier must contain an executable test command in backticks"
            )
        if not DELIBERATE_BREAK.search(falsifier) or "fail" not in falsifier.lower():
            problems.append(
                f"{path}:{number}: {name}: falsifier must name a deliberate break and observable failure"
            )
        records[name] = raw

    for field in (
        "meaning",
        "distinction",
        "counterexample",
        "lifecycle",
        "forcing_requirement",
        "falsifier",
    ):
        seen: dict[str, str] = {}
        for name, record in records.items():
            if "disposition" not in record:
                continue
            normalized = " ".join(record[field].casefold().split())
            other = seen.get(normalized)
            if other is not None:
                problems.append(
                    f"{path}: {name}: {field} duplicates {other}; candidates require distinct research"
                )
            else:
                seen[normalized] = name
    return records, problems


def _evidence_problem(root: Path, name: str, evidence: str) -> str | None:
    locations = [location.strip() for location in evidence.split(";")]
    if not locations or any(not location for location in locations):
        return f"{name}: evidence must contain exact repository paths and lines"
    for location in locations:
        match = EVIDENCE_LOCATION.fullmatch(location)
        if match is None:
            return f"{name}: evidence must contain exact repository paths and lines"
        path = (root / match.group("path")).resolve()
        try:
            path.relative_to(root.resolve())
        except ValueError:
            return f"{name}: evidence escapes the repository: {location}"
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except OSError as error:
            return f"{name}: cannot read evidence {location}: {error}"
        line_number = int(match.group("line"))
        if line_number > len(lines):
            return f"{name}: evidence line does not exist: {location}"
    first = EVIDENCE_LOCATION.fullmatch(locations[0])
    assert first is not None
    first_lines = (root / first.group("path")).read_text(encoding="utf-8").splitlines()
    if name not in first_lines[int(first.group("line")) - 1]:
        return f"{name}: first evidence line does not name the candidate: {locations[0]}"
    return None


def _crate_name(symbol: str) -> str:
    return symbol.split("::", maxsplit=1)[0].replace("_", "-")


def candidate_terms(
    terms: list[dict[str, Any]],
    research: dict[str, dict[str, str]],
    root: Path,
) -> tuple[list[dict[str, Any]], list[str]]:
    """Build independently reviewable packets for researched candidate names.

    Discovery never fabricates candidate prose. A hidden name without one
    explicit, source-anchored research record is a hard problem and produces no
    reviewable row. A blocked row remains deliberately unsigned.
    """
    hidden = hidden_vocabulary(terms)
    required_names = reviewable_vocabulary_names(terms)
    problems: list[str] = []
    selected = {
        name: record for name, record in research.items() if "disposition" in record
    }
    by_name = {term["name"]: term for term in terms}
    missing = sorted(required_names - set(research))
    extra = sorted(set(selected) - required_names)
    problems.extend(f"{name}: missing researched vocabulary candidate" for name in missing)
    problems.extend(f"{name}: candidate is not in the discovered hidden surface" for name in extra)

    candidates: list[dict[str, Any]] = []
    for name in sorted(set(selected) - set(extra)):
        record = selected[name]
        evidence = record["evidence"]
        evidence_problem = _evidence_problem(root, name, evidence)
        if evidence_problem:
            problems.append(evidence_problem)
            continue

        occurrences = hidden.get(name, [])
        if occurrences:
            symbols = sorted(
                {item["value"] for item in occurrences if item["field"] == "symbols"}
            )
            spec_symbols = [name] if any(
                item["field"] == "spec_symbols" for item in occurrences
            ) else []
            crates = sorted({_crate_name(symbol) for symbol in symbols})
        else:
            registered = by_name[name]
            symbols = list(registered.get("symbols", []))
            spec_symbols = list(registered.get("spec_symbols", []))
            crates = list(registered.get("crates", []))
        candidate = {
            "name": name,
            "source": "fava",
            "evidence": evidence,
            **{field: record[field].strip() for field in EXPLICIT_CANDIDATE_FIELDS},
            "symbols": symbols,
            "crates": crates,
            "spec_symbols": spec_symbols,
            "spec_crates": [],
        }
        incomplete = [
            field
            for field in EXPLICIT_CANDIDATE_FIELDS
            if not isinstance(candidate.get(field), str) or not candidate[field].strip()
        ]
        if incomplete:
            problems.append(f"{name}: incomplete candidate fields: {', '.join(incomplete)}")
            continue
        candidates.append(candidate)
    return candidates, problems


def canonical_markdown(term: dict[str, Any]) -> str:
    """Render one registry term as the exact text an approval signs.

    Raises ValueError if any field value has an unexpected type so that a
    malformed term cannot silently produce an unrendered or wrong approval.
    """
    lines = [f"# {term['name']}", ""]
    for field in PROSE_FIELDS:
        value = term.get(field)
        if value is None:
            continue
        if not isinstance(value, str):
            raise ValueError(
                f"field '{field}' must be a str, got {type(value).__name__}"
            )
        stripped = value.strip()
        if stripped:
            lines.append(f"**{field}**: {stripped}")
            lines.append("")
    for field in LIST_FIELDS:
        value = term.get(field)
        if value is None:
            continue
        if not isinstance(value, list):
            raise ValueError(
                f"field '{field}' must be a list, got {type(value).__name__}"
            )
        if not value:
            continue
        for i, element in enumerate(value):
            if not isinstance(element, str):
                raise ValueError(
                    f"field '{field}[{i}]' must be a str, got {type(element).__name__}"
                )
        lines.append(f"**{field}**:")
        lines.extend(f"- {v}" for v in sorted(value))
        lines.append("")
    extra = sorted(
        key
        for key in term
        if key not in {"name", *PROSE_FIELDS, *LIST_FIELDS}
    )
    for field in extra:
        value = term[field]
        if not isinstance(value, (str, int, float, bool)):
            raise ValueError(
                f"extra field '{field}' has unrenderable type {type(value).__name__}"
            )
        lines.append(f"**{field}**: {value}")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def approved_name(event: dict[str, Any]) -> str | None:
    """The single term name one approval event carries, if it carries one."""
    names = [
        tag[1]
        for tag in event.get("tags", [])
        if isinstance(tag, list) and len(tag) >= 2 and tag[0] == "name"
    ]
    return names[0] if len(names) == 1 else None


def verify_event(event: dict[str, Any]) -> list[str]:
    """Structural reasons one event is not a valid approval by the owner.

    Cryptographic checks (event-id hash and Schnorr signature) are not
    performed here; they are performed by the Rust governance test via
    nostr::Event::verify().
    """
    problems: list[str] = []
    required = {"id", "pubkey", "created_at", "kind", "tags", "content", "sig"}
    missing = sorted(required - event.keys())
    if missing:
        return [f"event is missing: {', '.join(missing)}"]
    if event["pubkey"] != OWNER:
        problems.append(f"event is not signed by the owner: {event['pubkey']}")
    if event["kind"] != APPROVAL_KIND:
        problems.append(f"event kind must be {APPROVAL_KIND}, not {event['kind']}")
    if approved_name(event) is None:
        problems.append("event must carry exactly one name tag")
    return problems


def load_approvals(path: Path) -> tuple[dict[str, list[dict[str, Any]]], list[str]]:
    """Load all structurally-valid owner signatures, preserving their history."""
    approvals: dict[str, list[dict[str, Any]]] = {}
    problems: list[str] = []
    if not path.exists():
        return approvals, problems
    for number, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), start=1
    ):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            problems.append(f"{path}:{number}: unreadable approval: {error}")
            continue
        reasons = verify_event(event)
        if reasons:
            problems.extend(f"{path}:{number}: {reason}" for reason in reasons)
            continue
        name = approved_name(event)
        events = approvals.setdefault(name, [])
        if not any(stored.get("id") == event.get("id") for stored in events):
            events.append(event)
            events.sort(key=lambda stored: (stored["created_at"], stored["id"]))
    return approvals, problems


def authoritative_approval(
    events: list[dict[str, Any]] | None, markdown: str
) -> dict[str, Any] | None:
    """Newest signature whose content is exactly the current candidate."""
    matching = [event for event in events or [] if event.get("content") == markdown]
    return matching[-1] if matching else None


def unapproved_terms(
    terms: tuple[dict[str, Any], ...], approvals: dict[str, list[dict[str, Any]]]
) -> list[str]:
    """Every term with no approval, or whose text no longer matches one."""
    problems: list[str] = []
    for term in terms:
        name = term["name"]
        if term.get("disposition") == "blocked":
            problems.append(f"{name}: blocked candidate cannot be approved")
            continue
        events = approvals.get(name)
        if not events:
            problems.append(f"{name}: no signed approval")
        elif authoritative_approval(events, canonical_markdown(term)) is None:
            problems.append(f"{name}: changed since its approval was signed")
    return problems
