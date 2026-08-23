#!/usr/bin/env python3
"""Check that architectural Rust symbols and crates have defined vocabulary."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

from rust_declarations import nominal_declarations


PUBLIC_NOUN = re.compile(
    r"^\s*pub\s+(?:unsafe\s+)?(?:struct|enum|trait|type|union)\s+([A-Z][A-Za-z0-9_]*)",
    re.MULTILINE,
)
# Build output and retained canary evidence (`apps/canary/runs/`) hold no
# package manifest and can be very large.
IGNORED_DIRECTORY_NAMES = frozenset({"target", "node_modules", "runs"})
RUST_SOURCE_DIRECTORIES = ("src", "tests", "benches", "examples")
SPEC_CRATE = re.compile(r"\b(fava(?:-[a-z0-9]+)+)(?![-a-z0-9])")
CAMEL_WORD = re.compile(r"[A-Z]+(?=[A-Z][a-z]|\d|$)|[A-Z]?[a-z]+|\d+")
REQUIRED_TERM_FIELDS = {"name", "source", "meaning", "owner", "symbols", "crates"}
PHASE_METADATA = re.compile(
    r"^\s*[*_`-]*\s*(?:phase|slug)\s*:\s*[\"']?(?P<value>[A-Za-z0-9._/-]+)",
    re.IGNORECASE,
)
PHASE_PATH_PREFIX = re.compile(r"\d+(?:\.\d+)+(?:-[a-z0-9]+)*-")
PATH_CHARACTER = re.compile(r"[A-Za-z0-9._/-]")
SPEC_CRATE_DIAGNOSTIC = "undocumented specified architectural crate: "


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


def package_manifests(root: Path) -> list[Path]:
    """Return every Cargo package manifest in the repository."""
    manifests: list[Path] = []
    pending = [root]
    while pending:
        directory = pending.pop()
        try:
            entries = sorted(directory.iterdir())
        except OSError:
            continue
        for entry in entries:
            if entry.is_symlink():
                continue
            if entry.is_dir():
                if entry.name.startswith(".") or entry.name.startswith("bazel-"):
                    continue
                if entry.name in IGNORED_DIRECTORY_NAMES:
                    continue
                pending.append(entry)
            elif entry.name == "Cargo.toml":
                manifests.append(entry)
    return sorted(manifests)


def collect_rust_vocabulary(
    root: Path,
) -> tuple[set[str], set[str], set[str], list[str]]:
    """Collect public and internal nominal symbols and every package name."""
    public_symbols: set[str] = set()
    nominal_symbols: set[str] = set()
    crates: set[str] = set()
    problems: list[str] = []
    crates_root = root / "crates"
    if not crates_root.is_dir():
        return (
            public_symbols,
            nominal_symbols,
            crates,
            [f"missing crates directory: {crates_root}"],
        )

    for manifest in package_manifests(root):
        try:
            data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError) as error:
            problems.append(f"cannot read package name from {manifest}: {error}")
            continue
        package = data.get("package", {}).get("name")
        if not isinstance(package, str) or not package.strip():
            if "workspace" in data and "package" not in data:
                continue
            problems.append(f"cannot read package name from {manifest}")
            continue
        crates.add(package)
        rust_crate = package.replace("-", "_")
        # A library crate under `crates/` owns closed Fava vocabulary at every
        # visibility. Any other package (the canary application, the external
        # falsifier proofs) is downstream: only the public names it declares are
        # architectural vocabulary, its private helpers are its own business.
        library = manifest.parent.parent == crates_root
        for directory in RUST_SOURCE_DIRECTORIES:
            for source in sorted((manifest.parent / directory).rglob("*.rs")):
                try:
                    text = source.read_text(encoding="utf-8")
                except OSError as error:
                    problems.append(f"cannot read {source}: {error}")
                    continue
                internal = library and directory == "src"
                for name, is_public in nominal_declarations(text):
                    if internal:
                        nominal_symbols.add(f"{rust_crate}::{name}")
                    if is_public:
                        public_symbols.add(f"{rust_crate}::{name}")
                        nominal_symbols.add(f"{rust_crate}::{name}")
    return public_symbols, nominal_symbols, crates, problems


def is_structural_crate_metadata(line: str, candidate: re.Match[str]) -> bool:
    """Return whether one crate-like token is structural metadata, not a declaration."""
    metadata = PHASE_METADATA.match(line)
    if metadata and metadata.start("value") <= candidate.start() < metadata.end("value"):
        return True

    diagnostic = line[
        max(0, candidate.start() - len(SPEC_CRATE_DIAGNOSTIC)) : candidate.start()
    ]
    if diagnostic == SPEC_CRATE_DIAGNOSTIC:
        return True

    prefix = line[max(0, candidate.start() - len("/tmp/")) : candidate.start()]
    if prefix == "/tmp/":
        return True

    path_start = candidate.start()
    while path_start > 0 and PATH_CHARACTER.fullmatch(line[path_start - 1]):
        path_start -= 1
    path_end = candidate.end()
    while path_end < len(line) and PATH_CHARACTER.fullmatch(line[path_end]):
        path_end += 1
    if "/" not in line[path_start:path_end]:
        return False

    segment_start = line.rfind("/", path_start, candidate.start()) + 1
    next_slash = line.find("/", candidate.end(), path_end)
    segment_end = path_end if next_slash == -1 else next_slash
    if candidate.end() != segment_end:
        return False

    segment_prefix = line[segment_start : candidate.start()]
    if PHASE_PATH_PREFIX.fullmatch(segment_prefix):
        return True
    return candidate.start() == segment_start and "-worktree-agent-" in candidate.group(1)


def collect_spec_vocabulary(root: Path) -> tuple[set[str], set[str], list[str]]:
    """Collect public symbols and Fava crate names declared by architecture docs."""
    symbols: set[str] = set()
    crates: set[str] = set()
    problems: list[str] = []
    spec_root = root / "docs" / "spec"
    if not spec_root.is_dir():
        return symbols, crates, [f"missing specification directory: {spec_root}"]
    # Authority is `docs/spec/**` and `docs/internals/vocabulary.toml`, nothing
    # else. `.planning/**` records plans, reviews, and audits; harvesting them
    # let any prose invent a crate or a symbol and flip this gate in either
    # direction. See `.planning/audit/2026-08-23/vocabulary.md`
    # (`vocab-planning-md-is-authority`).
    documents = list(spec_root.rglob("*.md"))
    for document in sorted(documents):
        try:
            content = document.read_text(encoding="utf-8")
        except OSError as error:
            problems.append(f"cannot read {document}: {error}")
            continue
        symbols.update(PUBLIC_NOUN.findall(content))
        for line in content.splitlines():
            for match in SPEC_CRATE.finditer(line):
                if not is_structural_crate_metadata(line, match):
                    crates.add(match.group(1))
    return symbols, crates, problems


def closest_registered_noun(symbol: str, registry: Registry) -> str | None:
    """Find an approved noun embedded in an unregistered symbol."""
    candidate_parts = CAMEL_WORD.findall(symbol.rsplit("::", maxsplit=1)[-1])
    candidate_words = words(symbol.rsplit("::", maxsplit=1)[-1])
    registered_names: list[str] = []
    for term in registry.terms:
        registered_names.append(str(term["name"]))
    registered_names.extend(
        symbol.rsplit("::", maxsplit=1)[-1] for symbol in registry.symbols
    )
    registered_names.extend(registry.spec_symbols)

    concept_matches: list[tuple[int, int, str]] = []
    registered_parts: set[str] = set()
    for name in registered_names:
        name_words = words(name)
        registered_parts.update(name_words)
        if not name_words or len(name_words) > len(candidate_words):
            continue
        if any(
            candidate_words[index : index + len(name_words)] == name_words
            for index in range(len(candidate_words) - len(name_words) + 1)
        ):
            concept_matches.append((len(name_words), len(name), name))
    if concept_matches:
        return max(concept_matches)[2]

    for part in reversed(candidate_parts):
        if part.lower() in registered_parts:
            return part
    return None


def approved_nominal_names(registry: Registry) -> dict[str, set[str]]:
    """Return the nominal names each crate is approved to declare.

    Approval is crate-scoped. A name registered for one owner does not approve
    a homonym in an unrelated crate: `Group` is an approved NIP-29 noun owned
    by `fava-simple-groups`, and a struct of the same name in another crate is
    a distinct concept wearing an approved spelling.
    """
    approved: dict[str, set[str]] = {}
    for symbol in registry.symbols:
        crate, _, name = symbol.rpartition("::")
        if crate:
            approved.setdefault(crate, set()).add(name)
    for term in registry.terms:
        owners = {str(term["owner"])}
        owners.update(str(value) for value in term.get("crates", []))
        names: set[str] = set()
        term_name = str(term["name"])
        if re.fullmatch(r"[A-Z][A-Za-z0-9_]*", term_name):
            names.add(term_name)
        names.update(
            str(value)
            for value in term.get("spec_symbols", [])
            if re.fullmatch(r"[A-Z][A-Za-z0-9_]*", str(value))
        )
        names.update(
            str(value).rsplit("::", maxsplit=1)[-1] for value in term.get("symbols", [])
        )
        for owner in owners:
            approved.setdefault(owner.replace("-", "_"), set()).update(names)
    return approved


def check(root: Path) -> list[str]:
    """Return every vocabulary violation in one repository."""
    registry_path = root / "docs" / "internals" / "vocabulary.toml"
    registry, problems = load_registry(registry_path)
    if registry is None:
        return problems

    public_symbols, nominal_symbols, package_names, source_problems = (
        collect_rust_vocabulary(root)
    )
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
    approved_names = approved_nominal_names(registry)
    internal_symbols = nominal_symbols - public_symbols
    for symbol in sorted(internal_symbols):
        crate, _, name = symbol.rpartition("::")
        if name in approved_names.get(crate, frozenset()):
            continue
        # Vocabulary is closed by default: a single-word name and a name that
        # embeds no registered noun are both unapproved nominal vocabulary.
        # Filtering either one silenced the `Group` homonym and two of the nine
        # unapproved lifecycle owners.
        message = f"unapproved nominal vocabulary variant: {symbol}"
        noun = closest_registered_noun(symbol, registry)
        if noun:
            message += f" (existing noun: {noun})"
        problems.append(message)
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

    # The specified half of the registry is an approval record, not evidence of
    # delivery. Check it against reality too, so an approved crate or symbol
    # that was never built is visible instead of silent.
    declared_names = {symbol.rsplit("::", maxsplit=1)[-1] for symbol in nominal_symbols}
    for package in sorted(registry.spec_crates - package_names):
        problems.append(f"specified architectural crate is not implemented: {package}")
    for symbol in sorted(registry.spec_symbols - declared_names):
        problems.append(f"specified architectural symbol is not implemented: {symbol}")
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
