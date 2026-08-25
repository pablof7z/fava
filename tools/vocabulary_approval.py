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
from pathlib import Path
from typing import Any

OWNER = "fa984bd7dbb282f07e16e7ae87b26a2a7b9b90b7246a44771f0cf5ae58018f52"
APPROVAL_KIND = 9999
APPROVALS_PATH = Path("docs/internals/approvals.jsonl")

PROSE_FIELDS = (
    "source",
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


def load_approvals(path: Path) -> tuple[dict[str, dict[str, Any]], list[str]]:
    """Load the newest structurally-valid owner approval for each term name."""
    approvals: dict[str, dict[str, Any]] = {}
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
        current = approvals.get(name)
        if current is None or event["created_at"] >= current["created_at"]:
            approvals[name] = event
    return approvals, problems


def unapproved_terms(
    terms: tuple[dict[str, Any], ...], approvals: dict[str, dict[str, Any]]
) -> list[str]:
    """Every term with no approval, or whose text no longer matches one."""
    problems: list[str] = []
    for term in terms:
        name = term["name"]
        approval = approvals.get(name)
        if approval is None:
            problems.append(f"{name}: no signed approval")
        elif approval["content"] != canonical_markdown(term):
            problems.append(f"{name}: changed since its approval was signed")
    return problems
