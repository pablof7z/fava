#!/usr/bin/env python3
"""Build and verify the canonical per-term vocabulary signing package."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tomllib
from pathlib import Path
from typing import Any, Iterable

import vocabulary_approval as approval
import vocabulary_structure as structure


PACKAGE_OWNER = "fava-simple-groups"
TERM_COUNT = 34
MANIFEST_PATH = Path(
    "docs/internals/fava-simple-groups-vocabulary-package.json"
)
FORMAT = "fava-vocabulary-markdown-package-v1"


class PackageError(RuntimeError):
    """The selected terms cannot form the exact review package."""


def payload_frame(name: str, markdown: str) -> bytes:
    """Length-delimit one named UTF-8 Markdown payload.

    The byte record is exactly:

        u64be(len(name_utf8)) || name_utf8 ||
        u64be(len(markdown_utf8)) || markdown_utf8

    No separator, terminator, normalization, or platform newline conversion is
    applied. The unsigned 64-bit lengths make every boundary unambiguous.
    """
    if not isinstance(name, str) or not name:
        raise PackageError("package term name must be non-empty text")
    if not isinstance(markdown, str):
        raise PackageError("package Markdown must be text")
    name_bytes = name.encode("utf-8")
    markdown_bytes = markdown.encode("utf-8")
    try:
        return (
            len(name_bytes).to_bytes(8, "big")
            + name_bytes
            + len(markdown_bytes).to_bytes(8, "big")
            + markdown_bytes
        )
    except OverflowError as error:
        raise PackageError("package field exceeds the unsigned 64-bit bound") from error


def ordered_payloads(
    payloads: Iterable[tuple[str, str]],
) -> list[tuple[str, str]]:
    """Validate identities and sort them by their exact UTF-8 name bytes."""
    validated: list[tuple[str, str]] = []
    for name, markdown in payloads:
        # Validate types and encoding before either value participates in sorting.
        payload_frame(name, markdown)
        validated.append((name, markdown))
    ordered = sorted(validated, key=lambda payload: payload[0].encode("utf-8"))
    names: set[str] = set()
    for name, _markdown in ordered:
        if name in names:
            raise PackageError(f"package repeats term name: {name}")
        names.add(name)
    return ordered


def canonical_package(payloads: Iterable[tuple[str, str]]) -> bytes:
    """Serialize sorted named payloads as concatenated length-delimited records."""
    return b"".join(
        payload_frame(name, markdown)
        for name, markdown in ordered_payloads(payloads)
    )


def manifest_for_payloads(
    owner: str, payloads: Iterable[tuple[str, str]]
) -> dict[str, Any]:
    """Describe every canonical payload and the complete package bytes."""
    ordered = ordered_payloads(payloads)
    package = b"".join(payload_frame(name, markdown) for name, markdown in ordered)
    terms = []
    for index, (name, markdown) in enumerate(ordered):
        name_bytes = name.encode("utf-8")
        markdown_bytes = markdown.encode("utf-8")
        terms.append(
            {
                "index": index,
                "name": name,
                "name_utf8_byte_length": len(name_bytes),
                "markdown_utf8_byte_length": len(markdown_bytes),
                "markdown_sha256": hashlib.sha256(markdown_bytes).hexdigest(),
                "frame_byte_length": 16 + len(name_bytes) + len(markdown_bytes),
            }
        )
    return {
        "format": FORMAT,
        "owner": owner,
        "ordering": "ascending lexicographic order of exact UTF-8 term-name bytes",
        "framing": {
            "integer_encoding": "unsigned 64-bit big-endian",
            "record": (
                "name_utf8_byte_length || name_utf8 || "
                "markdown_utf8_byte_length || markdown_utf8"
            ),
            "package": "concatenation of all ordered records with no prefix or suffix",
        },
        "term_count": len(terms),
        "terms": terms,
        "package_byte_length": len(package),
        "package_sha256": hashlib.sha256(package).hexdigest(),
    }


def review_payloads(root: Path) -> list[tuple[str, str]]:
    """Render the exact current Markdown for the package owner's registry terms."""
    snapshot_path = root / approval.STRUCTURE_PATH
    if not structure.snapshot_inputs_current(root, snapshot_path):
        raise PackageError(
            "compiler/documentation inputs differ from the structural snapshot; "
            "run python3 tools/vocabulary_structure.py update"
        )
    packets, problems = structure.read_snapshot(snapshot_path)
    if problems:
        raise PackageError("invalid structural snapshot: " + "; ".join(problems))

    registry_path = root / "docs/internals/vocabulary.toml"
    registry = tomllib.loads(registry_path.read_text(encoding="utf-8"))
    terms = [
        term for term in registry.get("term", []) if term.get("owner") == PACKAGE_OWNER
    ]
    if len(terms) != TERM_COUNT:
        raise PackageError(
            f"{PACKAGE_OWNER} package must contain exactly {TERM_COUNT} terms, "
            f"found {len(terms)}"
        )

    payloads: list[tuple[str, str]] = []
    for term in terms:
        name = term.get("name")
        if not isinstance(name, str) or not name:
            raise PackageError("package registry term has no valid name")
        packet = packets.get(name)
        if packet is None:
            raise PackageError(f"structural snapshot has no packet for {name}")
        review_problems = packet.get("review_problems")
        if review_problems:
            raise PackageError(f"{name} has review blockers: {'; '.join(review_problems)}")
        payloads.append((name, approval.canonical_markdown(term, packet)))
    return ordered_payloads(payloads)


def expected_manifest(root: Path) -> dict[str, Any]:
    return manifest_for_payloads(PACKAGE_OWNER, review_payloads(root))


def render_manifest(manifest: dict[str, Any]) -> bytes:
    return (
        json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def manifest_matches(path: Path, expected: bytes) -> bool:
    """Exact bytes are required; semantic JSON equivalence is insufficient."""
    try:
        return path.read_bytes() == expected
    except OSError:
        return False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("update", "check"))
    parser.add_argument("--root", type=Path, default=Path.cwd())
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    try:
        expected = render_manifest(expected_manifest(root))
    except (OSError, PackageError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"cannot build vocabulary package: {error}", file=sys.stderr)
        return 1

    path = root / MANIFEST_PATH
    if arguments.command == "update":
        path.write_bytes(expected)
        manifest = json.loads(expected)
        print(
            f"updated {MANIFEST_PATH}: {manifest['term_count']} terms, "
            f"{manifest['package_byte_length']} bytes, {manifest['package_sha256']}"
        )
        return 0
    if not manifest_matches(path, expected):
        print(
            "canonical vocabulary package manifest is stale; run "
            "python3 tools/vocabulary_package.py update",
            file=sys.stderr,
        )
        return 1
    manifest = json.loads(expected)
    print(
        f"canonical vocabulary package is current: {manifest['term_count']} terms, "
        f"{manifest['package_byte_length']} bytes, {manifest['package_sha256']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
