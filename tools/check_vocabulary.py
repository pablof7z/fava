#!/usr/bin/env python3
"""Check that architectural Rust symbols and crates have defined vocabulary."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


PUBLIC_NOUN = re.compile(
    r"^\s*pub\s+(?:unsafe\s+)?(?:struct|enum|trait|type)\s+([A-Z][A-Za-z0-9_]*)",
    re.MULTILINE,
)
SPEC_CRATE = re.compile(r"\b(fava(?:-[a-z0-9]+)+)(?![-a-z0-9])")
CAMEL_WORD = re.compile(r"[A-Z]+(?=[A-Z][a-z]|\d|$)|[A-Z]?[a-z]+|\d+")
REQUIRED_TERM_FIELDS = {"name", "source", "meaning", "owner", "symbols", "crates"}


@dataclass(frozen=True)
class Registry:
    terms: tuple[dict[str, object], ...]
    symbols: frozenset[str]
    crates: frozenset[str]
    spec_symbols: frozenset[str]
    spec_crates: frozenset[str]


def words(name: str) -> tuple[str, ...]:
    """Split one Rust or registry name into lowercase words."""
    return tuple(part.lower() for part in CAMEL_WORD.findall(name))


def load_registry(path: Path) -> tuple[Registry | None, list[str]]:
    """Load and validate the vocabulary registry."""
    problems: list[str] = []
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        return None, [f"cannot read {path}: {error}"]

    if data.get("version") != 1:
        problems.append("vocabulary registry version must be 1")
    raw_terms = data.get("term")
    if not isinstance(raw_terms, list) or not raw_terms:
        return None, problems + ["vocabulary registry must contain [[term]] entries"]

    names: set[str] = set()
    symbols: set[str] = set()
    crates: set[str] = set()
    spec_symbols: set[str] = set()
    spec_crates: set[str] = set()
    terms: list[dict[str, object]] = []
    for index, raw_term in enumerate(raw_terms, start=1):
        if not isinstance(raw_term, dict):
            problems.append(f"term {index} must be a table")
            continue
        term = dict(raw_term)
        missing = sorted(REQUIRED_TERM_FIELDS - term.keys())
        if missing:
            problems.append(f"term {index} is missing: {', '.join(missing)}")
            continue

        name = term["name"]
        source = term["source"]
        if not isinstance(name, str) or not name.strip():
            problems.append(f"term {index} has an invalid name")
            continue
        if name in names:
            problems.append(f"duplicate term name: {name}")
        names.add(name)

        if source not in {"nostr", "fava"}:
            problems.append(f"{name}: source must be nostr or fava")
        if source == "nostr" and not term.get("protocol"):
            problems.append(f"{name}: Nostr terms require protocol attribution")
        if source == "fava":
            if not term.get("nearest_nostr"):
                problems.append(f"{name}: Fava terms require nearest_nostr")
            if not term.get("distinction"):
                problems.append(f"{name}: Fava terms require an exact distinction")

        for field in ("meaning", "owner"):
            if not isinstance(term[field], str) or not str(term[field]).strip():
                problems.append(f"{name}: {field} must be non-empty text")

        for field, destination in (("symbols", symbols), ("crates", crates)):
            values = term[field]
            if not isinstance(values, list):
                problems.append(f"{name}: {field} must be a list")
                continue
            for value in values:
                if not isinstance(value, str) or not value.strip():
                    problems.append(f"{name}: {field} contains an invalid value")
                elif value in destination:
                    problems.append(f"duplicate registered {field[:-1]}: {value}")
                else:
                    destination.add(value)
        for field, destination in (
            ("spec_symbols", spec_symbols),
            ("spec_crates", spec_crates),
        ):
            values = term.get(field, [])
            if not isinstance(values, list):
                problems.append(f"{name}: {field} must be a list")
                continue
            for value in values:
                if not isinstance(value, str) or not value.strip():
                    problems.append(f"{name}: {field} contains an invalid value")
                elif value in destination:
                    problems.append(f"duplicate registered {field[:-1]}: {value}")
                else:
                    destination.add(value)
        terms.append(term)

    return Registry(
        tuple(terms),
        frozenset(symbols),
        frozenset(crates),
        frozenset(spec_symbols),
        frozenset(spec_crates),
    ), problems


def crate_name(manifest: Path) -> str:
    """Read a package name from one crate manifest."""
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    return str(data["package"]["name"])


def collect_public_symbols(root: Path) -> tuple[set[str], set[str], list[str]]:
    """Collect public nominal Rust symbols and package names under crates/."""
    symbols: set[str] = set()
    crates: set[str] = set()
    problems: list[str] = []
    crates_root = root / "crates"
    if not crates_root.is_dir():
        return symbols, crates, [f"missing crates directory: {crates_root}"]

    for manifest in sorted(crates_root.glob("*/Cargo.toml")):
        try:
            package = crate_name(manifest)
        except (KeyError, OSError, tomllib.TOMLDecodeError) as error:
            problems.append(f"cannot read package name from {manifest}: {error}")
            continue
        crates.add(package)
        rust_crate = package.replace("-", "_")
        for source in sorted((manifest.parent / "src").rglob("*.rs")):
            try:
                text = source.read_text(encoding="utf-8")
            except OSError as error:
                problems.append(f"cannot read {source}: {error}")
                continue
            symbols.update(f"{rust_crate}::{name}" for name in PUBLIC_NOUN.findall(text))
    return symbols, crates, problems


def collect_spec_vocabulary(root: Path) -> tuple[set[str], set[str], list[str]]:
    """Collect public symbols and Fava crate names declared by architecture docs."""
    symbols: set[str] = set()
    crates: set[str] = set()
    problems: list[str] = []
    spec_root = root / "docs" / "spec"
    if not spec_root.is_dir():
        return symbols, crates, [f"missing specification directory: {spec_root}"]
    documents = list(spec_root.glob("*.md"))
    planning_root = root / ".planning"
    if planning_root.is_dir():
        documents.extend(planning_root.rglob("*.md"))
    for document in sorted(documents):
        try:
            content = document.read_text(encoding="utf-8")
        except OSError as error:
            problems.append(f"cannot read {document}: {error}")
            continue
        symbols.update(PUBLIC_NOUN.findall(content))
        for line in content.splitlines():
            for match in SPEC_CRATE.finditer(line):
                prefix = line[max(0, match.start() - len("/tmp/")) : match.start()]
                if prefix != "/tmp/":
                    crates.add(match.group(1))
    return symbols, crates, problems


def closest_registered_noun(symbol: str, registry: Registry) -> str | None:
    """Find an approved noun embedded in an unregistered symbol."""
    candidate_words = set(words(symbol.rsplit("::", maxsplit=1)[-1]))
    matches: list[str] = []
    for term in registry.terms:
        name = str(term["name"])
        term_words = words(name)
        if term_words and set(term_words).issubset(candidate_words):
            matches.append(name)
    return max(matches, key=len, default=None)


def check(root: Path) -> list[str]:
    """Return every vocabulary violation in one repository."""
    registry_path = root / "docs" / "internals" / "vocabulary.toml"
    registry, problems = load_registry(registry_path)
    if registry is None:
        return problems

    public_symbols, package_names, source_problems = collect_public_symbols(root)
    problems.extend(source_problems)
    spec_symbols, spec_crates, spec_problems = collect_spec_vocabulary(root)
    problems.extend(spec_problems)
    for symbol in sorted(public_symbols - registry.symbols):
        message = f"undocumented public architectural symbol: {symbol}"
        noun = closest_registered_noun(symbol, registry)
        if noun:
            message += f" (existing noun: {noun})"
        problems.append(message)
    for symbol in sorted(registry.symbols - public_symbols):
        problems.append(f"registered public symbol does not exist: {symbol}")
    for package in sorted(package_names - registry.crates):
        problems.append(f"undocumented architectural crate: {package}")
    for package in sorted(registry.crates - package_names):
        problems.append(f"registered architectural crate does not exist: {package}")

    current_symbol_names = {
        symbol.rsplit("::", maxsplit=1)[-1] for symbol in registry.symbols
    }
    allowed_spec_symbols = current_symbol_names | set(registry.spec_symbols)
    for symbol in sorted(spec_symbols - allowed_spec_symbols):
        message = f"undocumented specified architectural symbol: {symbol}"
        noun = closest_registered_noun(symbol, registry)
        if noun:
            message += f" (existing noun: {noun})"
        problems.append(message)
    for symbol in sorted(registry.spec_symbols - spec_symbols):
        problems.append(f"registered specified symbol does not exist: {symbol}")

    allowed_spec_crates = set(registry.crates) | set(registry.spec_crates)
    for package in sorted(spec_crates - allowed_spec_crates):
        problems.append(f"undocumented specified architectural crate: {package}")
    for package in sorted(registry.spec_crates - spec_crates):
        problems.append(f"registered specified crate does not exist: {package}")
    return problems


def main() -> int:
    """Run the repository vocabulary gate."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    problems = check(root)
    if problems:
        print("ARCHITECTURAL VOCABULARY REVIEW REQUIRED", file=sys.stderr)
        for problem in problems:
            print(f"- {problem}", file=sys.stderr)
        return 1
    print("architectural vocabulary check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
