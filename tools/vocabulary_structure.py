#!/usr/bin/env python3
"""Compile the deterministic Rust structure bound to vocabulary approvals."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import tomllib
from pathlib import Path
from typing import Any, Sequence

import crate_readme_api as public_api
import vocabulary_approval as approval
import vocabulary_review as review


SNAPSHOT_PATH = Path("docs/internals/vocabulary-structure.json")
FORMAT_VERSION = 2
TARGET_DIRECTORY = Path("target/vocabulary-structure")
MAXIMUM_TERM_STRUCTURE_BYTES = 192 * 1024
NOMINAL_KINDS = frozenset({"enum", "struct", "trait", "type_alias", "union"})
EMPTY_STRUCTURE: dict[str, list[Any]] = {
    "private_architectural_state": [],
    "public_api": [],
    "reexports": [],
}

human_review_inventory = review.human_review_inventory
public_api_catalog = review.public_api_catalog
rustdoc_descriptions = review.rustdoc_descriptions


class StructureError(RuntimeError):
    """A deterministic structural extraction or snapshot failure."""


def _environment() -> dict[str, str]:
    environment = os.environ.copy()
    for variable in ("CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS", "RUSTDOCFLAGS"):
        environment.pop(variable, None)
    return environment


def _private_extractor_command(
    package: public_api.Package, target_dir: Path
) -> list[str]:
    command = public_api.extractor_command(package, target_dir)
    command.extend(["--document-private-items"])
    return command


def _run_checked(command: Sequence[str], root: Path, description: str) -> str:
    result = public_api.run(command, cwd=root, env=_environment())
    if result.returncode:
        detail = result.stderr.strip() or result.stdout.strip()
        raise StructureError(f"{description}:\n{detail}")
    return result.stdout


def _compiled_package(
    root: Path, package: public_api.Package
) -> tuple[str, dict[str, Any], dict[str, Any]]:
    public_target = root / TARGET_DIRECTORY / "public"
    private_target = root / TARGET_DIRECTORY / "private"
    _run_checked(
        public_api.extractor_command(package, public_target),
        root,
        f"could not extract public API for {package.name}",
    )
    public_json_path = public_target / "doc" / f"{package.crate_name}.json"
    if not public_json_path.is_file():
        raise StructureError(f"rustdoc did not produce {public_json_path}")
    rendered = _run_checked(
        public_api.renderer_command(public_json_path),
        root,
        f"could not render public API for {package.name}",
    )
    public_json = json.loads(public_json_path.read_text(encoding="utf-8"))

    _run_checked(
        _private_extractor_command(package, private_target),
        root,
        f"could not extract private structure for {package.name}",
    )
    private_json_path = private_target / "doc" / f"{package.crate_name}.json"
    if not private_json_path.is_file():
        raise StructureError(f"rustdoc did not produce {private_json_path}")
    private_json = json.loads(private_json_path.read_text(encoding="utf-8"))
    if not private_json.get("includes_private"):
        raise StructureError(f"{package.name}: rustdoc omitted private items")
    return rendered, public_json, private_json


def _exported_paths(text: str, crate_name: str) -> list[str]:
    pattern = re.compile(
        rf"\b{re.escape(crate_name)}(?:::{public_api.IDENTIFIER})*"
    )
    return sorted({match.group(0) for match in pattern.finditer(text)})


def _associated_member(declaration: str, owner: str) -> str | None:
    if re.match(
        r'^pub\s+(?:(?:const|async|unsafe)\s+)*(?:extern\s+"[^"]+"\s+)?'
        r"(?:fn|type|const|static)\s+",
        declaration,
    ) is None:
        return None
    head = declaration.split("=", maxsplit=1)[0].split("(", maxsplit=1)[0]
    match = re.search(
        rf"{re.escape(owner)}::(?P<name>{public_api.IDENTIFIER})\s*(?::|$)",
        head,
    )
    return match.group("name") if match is not None else None


def public_api_identity(record: dict[str, Any]) -> str:
    """Stable semantic identity of one compiler-visible public API item."""
    return record["path"]


def public_records(output: str, crate_name: str) -> list[dict[str, Any]]:
    """Exact compiler-rendered declarations with stable exported paths."""
    records: list[dict[str, Any]] = []
    implementation_type: str | None = None
    implementation_qualification: str | None = None
    implementation_trait: str | None = None
    implementation_owner: str | None = None
    implementation: str | None = None
    implementation_binding_roots: list[str] = []
    for raw_line in output.splitlines():
        line = raw_line.strip()
        if line.startswith(("impl ", "impl<")):
            before, separator, after = line.removeprefix("impl").strip().rpartition(
                " for "
            )
            try:
                implementation_type = public_api.exported_path(
                    after, crate_name
                )
            except public_api.InventoryError:
                implementation_type = None
            owner = after.partition(" where ")[0]
            implementation_owner = owner
            trait = public_api.without_impl_generics(before) if separator else None
            try:
                implementation_trait = (
                    public_api.exported_path(trait, crate_name)
                    if trait is not None
                    else None
                )
            except public_api.InventoryError:
                implementation_trait = None
            implementation_qualification = (
                f"<{owner} as {trait}>"
                if trait is not None
                and (implementation_type is not None or implementation_trait is not None)
                else None
            )
            implementation = line if implementation_qualification or implementation_type else None
            implementation_binding_roots = _exported_paths(line, crate_name)
            continue
        if not line.startswith("pub "):
            continue
        if implementation_qualification is not None and implementation_type is None:
            member = _associated_member(line, implementation_owner or "")
            if member is not None:
                record: dict[str, Any] = {
                    "declaration": line,
                    "implementation": implementation or "",
                    "path": f"{implementation_qualification}::{member}",
                }
                if implementation_binding_roots:
                    record["binding_roots"] = implementation_binding_roots
                records.append(record)
                continue
            implementation_type = None
            implementation_qualification = None
            implementation_trait = None
            implementation_owner = None
            implementation = None
            implementation_binding_roots = []
        try:
            path = public_api.exported_path(line, crate_name)
        except public_api.InventoryError:
            if implementation_qualification is None or implementation_trait is None:
                continue
            declaration_head = line.split("=", maxsplit=1)[0].split("(", maxsplit=1)[0]
            match = re.search(r"::((?:r#)?[A-Za-z_][A-Za-z0-9_]*)\s*$", declaration_head)
            if match is None:
                continue
            records.append(
                {
                    "declaration": line,
                    "implementation": implementation or "",
                    "path": f"{implementation_qualification}::{match.group(1)}",
                }
            )
            continue
        parent = path.removesuffix("!").rpartition("::")[0]
        context = implementation if implementation_type == parent else None
        qualification = (
            implementation_qualification if implementation_type == parent else None
        )
        if implementation_type is not None and parent != implementation_type:
            implementation_type = None
            implementation_qualification = None
            implementation_trait = None
            implementation_owner = None
            implementation = None
            implementation_binding_roots = []
        record = {
            "declaration": line,
            "path": public_api.qualified_path(path, qualification),
        }
        if context is not None:
            record["implementation"] = context
        records.append(record)
    return records


def reexports(rustdoc: dict[str, Any]) -> list[dict[str, Any]]:
    """Public `use` paths, source paths, and compiler target ids."""
    index = rustdoc["index"]
    root_id = str(rustdoc["root"])
    crate_name = index[root_id]["name"]
    found: list[dict[str, Any]] = []

    def walk(module_id: str, parent: list[str]) -> None:
        module = index[module_id]
        for raw_id in module["inner"]["module"]["items"]:
            item_id = str(raw_id)
            item = index[item_id]
            inner = item["inner"]
            if "use" in inner and item.get("visibility") == "public":
                value = inner["use"]
                name = value.get("name")
                target = value.get("id")
                if name and target is not None:
                    found.append(
                        {
                            "path": "::".join([*parent, name]),
                            "source": value["source"],
                            "target": str(target),
                        }
                    )
            elif "module" in inner and item.get("visibility") == "public":
                name = item.get("name")
                if name:
                    walk(item_id, [*parent, name])

    walk(root_id, [crate_name])
    return sorted(found, key=lambda value: (value["path"], value["source"]))


def _root_matches(path: str, root: str) -> bool:
    if "::" not in root:
        return path == root
    return (
        path == root
        or path.startswith(root + "::")
        or path.startswith("<" + root + " as ")
        or f" as {root}>" in path
    )


def _record_matches_root(record: dict[str, Any], root: str) -> bool:
    return _root_matches(record["path"], root) or any(
        _root_matches(binding_root, root)
        for binding_root in record.get("binding_roots", [])
    )


def _term_crates(term: dict[str, Any]) -> set[str]:
    crates: set[str] = set()
    owner = term.get("owner")
    if isinstance(owner, str):
        crates.add(owner.replace("-", "_"))
    for field in ("crates", "symbols"):
        values = term.get(field, [])
        if not isinstance(values, list):
            continue
        for value in values:
            if not isinstance(value, str):
                continue
            crate = value.split("::", maxsplit=1)[0]
            crates.add(crate.replace("-", "_"))
    return crates


def _term_roots(
    term: dict[str, Any], crate_name: str, aliases: list[dict[str, Any]], rustdoc: dict[str, Any]
) -> tuple[set[str], list[dict[str, str]]]:
    roots = {
        symbol
        for symbol in term.get("symbols", [])
        if isinstance(symbol, str) and symbol.split("::", maxsplit=1)[0] == crate_name
    }
    path_targets = {item["path"]: item["target"] for item in aliases}
    canonical_targets = {
        "::".join(value["path"]): str(item_id)
        for item_id, value in rustdoc.get("paths", {}).items()
        if value.get("crate_id") == 0
    }
    targets = {
        target
        for root in roots
        for target in (path_targets.get(root), canonical_targets.get(root))
        if target is not None
    }
    bound_aliases = [
        {"path": item["path"], "source": item["source"]}
        for item in aliases
        if item["target"] in targets
    ]
    roots.update(item["path"] for item in bound_aliases)
    return roots, bound_aliases


def _source_declaration(root: Path, span: dict[str, Any]) -> tuple[str, str]:
    source_path = (root / span["filename"]).resolve()
    try:
        relative = source_path.relative_to(root.resolve()).as_posix()
    except ValueError as error:
        raise StructureError(f"rustdoc source escapes repository: {source_path}") from error
    lines = source_path.read_bytes().splitlines(keepends=True)
    begin_line, begin_column = span["begin"]
    end_line, end_column = span["end"]
    selected = lines[begin_line - 1 : end_line]
    if not selected:
        raise StructureError(f"empty rustdoc span in {relative}")
    selected[0] = selected[0][begin_column - 1 :]
    selected[-1] = selected[-1][: end_column - 1]
    return relative, b"".join(selected).decode("utf-8")


def _stable_visibility(value: Any) -> Any:
    if isinstance(value, dict) and set(value) == {"restricted"}:
        restricted = value["restricted"]
        if isinstance(restricted, dict) and isinstance(restricted.get("path"), str):
            return {"restricted": restricted["path"]}
    return value


def private_state_records(
    root: Path,
    package: public_api.Package,
    rustdoc: dict[str, Any],
    term: dict[str, Any],
) -> list[dict[str, Any]]:
    if package.crate_name not in _term_crates(term):
        return []
    name = term["name"]
    records: list[dict[str, Any]] = []
    for item_id, item in rustdoc["index"].items():
        kind = next(iter(item["inner"]))
        if item.get("name") != name or kind not in NOMINAL_KINDS:
            continue
        span = item.get("span")
        path = rustdoc.get("paths", {}).get(item_id, {}).get("path")
        if span is None or not path:
            continue
        source, declaration = _source_declaration(root, span)
        records.append(
            {
                "declaration": declaration,
                "kind": kind,
                "path": "::".join(path),
                "source": source,
                "visibility": _stable_visibility(item["visibility"]),
            }
        )
    return sorted(records, key=lambda value: (value["path"], value["source"]))


def canonical_structure(structure: dict[str, Any]) -> str:
    """Exact JSON embedded in the signed payload."""
    return json.dumps(structure, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def input_fingerprint(root: Path) -> str:
    paths = {
        root / "Cargo.lock",
        root / "Cargo.toml",
        root / "rust-toolchain.toml",
        root / "docs" / "internals" / "vocabulary.toml",
        root / "docs" / "internals" / "vocabulary-candidates.jsonl",
        root / "tools" / "crate_readme_api.py",
        root / "tools" / "vocabulary_approval.py",
        root / "tools" / "vocabulary_review.py",
        root / "tools" / "vocabulary_structure.py",
    }
    paths.update((root / "crates").rglob("*.rs"))
    paths.update((root / "crates").rglob("Cargo.toml"))
    paths.update((root / "crates").rglob("build.rs"))
    paths.update((root / "crates").rglob("README.md"))
    digest = hashlib.sha256()
    for path in sorted(path for path in paths if path.is_file()):
        relative = path.relative_to(root).as_posix().encode("utf-8")
        content = path.read_bytes()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return digest.hexdigest()


def _terms(root: Path) -> list[dict[str, Any]]:
    registry = _registry(root)["term"]
    research, _ = approval.load_candidate_research(root / approval.CANDIDATES_PATH)
    candidates, _ = approval.candidate_terms(registry, research, root)
    by_name = {term["name"]: term for term in registry}
    by_name.update({term["name"]: term for term in candidates})
    return [by_name[name] for name in sorted(by_name)]


def _registry(root: Path) -> dict[str, Any]:
    return tomllib.loads(
        (root / "docs/internals/vocabulary.toml").read_text(encoding="utf-8")
    )


def public_api_binding_coverage(
    records: list[dict[str, Any]], packets: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    """One-to-one ownership report for a compiler-rendered crate public API."""
    compiler_items: dict[str, dict[str, dict[str, Any]]] = {}
    for record in records:
        compiler_items.setdefault(public_api_identity(record), {})[
            canonical_structure(record)
        ] = record
    collisions = [
        {
            "identity": identity,
            "records": sorted(variants.values(), key=canonical_structure),
        }
        for identity, variants in compiler_items.items()
        if len(variants) > 1
    ]
    bindings: dict[str, set[str]] = {identity: set() for identity in compiler_items}
    for name, packet in packets.items():
        structure = packet.get("structure", packet)
        for record in structure.get("public_api", []):
            identity = public_api_identity(record)
            variants = compiler_items.get(identity)
            if variants is not None and canonical_structure(record) in variants:
                bindings[identity].add(name)
    unbound = [
        next(iter(compiler_items[identity].values()))
        for identity, owners in bindings.items()
        if not owners
    ]
    multiply_bound = [
        {
            "item": next(iter(compiler_items[identity].values())),
            "owners": sorted(owners),
        }
        for identity, owners in bindings.items()
        if len(owners) > 1
    ]
    collision_identities = {item["identity"] for item in collisions}
    return {
        "bound_items": sum(
            len(owners) == 1 and identity not in collision_identities
            for identity, owners in bindings.items()
        ),
        "collisions": sorted(collisions, key=lambda value: value["identity"]),
        "multiply_bound": sorted(
            multiply_bound, key=lambda value: canonical_structure(value["item"])
        ),
        "public_items": len(compiler_items),
        "unbound": sorted(unbound, key=canonical_structure),
    }


def compile_snapshot(root: Path) -> dict[str, Any]:
    public_api.verify_extractor(root)
    registry = _registry(root)
    terms = _terms(root)
    structures = {name["name"]: {key: [] for key in EMPTY_STRUCTURE} for name in terms}
    packages = [
        package
        for package in public_api.workspace_packages(root).values()
        if package.directory.parent == root / "crates"
    ]
    catalog = public_api_catalog(root, packages)
    documentation: dict[str, str] = {}
    required_coverage = set(registry.get("complete_public_api_crates", []))
    semantic_crates = {name.replace("-", "_") for name in required_coverage}
    covered_packages: set[str] = set()
    for package in sorted(packages, key=lambda value: value.name):
        output, public_json, private_json = _compiled_package(root, package)
        records = public_records(output, package.crate_name)
        aliases = reexports(public_json)
        documentation.update(rustdoc_descriptions(public_json, aliases))
        for term in terms:
            roots, bound_aliases = _term_roots(term, package.crate_name, aliases, public_json)
            if roots:
                structures[term["name"]]["public_api"].extend(
                    record
                    for record in records
                    if any(_record_matches_root(record, root) for root in roots)
                )
                structures[term["name"]]["reexports"].extend(bound_aliases)
            structures[term["name"]]["private_architectural_state"].extend(
                private_state_records(root, package, private_json, term)
            )
        if package.name in required_coverage:
            covered_packages.add(package.name)
            coverage = public_api_binding_coverage(
                records,
                {
                    name: {"structure": value}
                    for name, value in structures.items()
                },
            )
            if coverage["unbound"] or coverage["multiply_bound"] or coverage["collisions"]:
                unbound = ", ".join(item["path"] for item in coverage["unbound"])
                repeated = ", ".join(
                    f"{item['item']['path']} ({', '.join(item['owners'])})"
                    for item in coverage["multiply_bound"]
                )
                collided = ", ".join(
                    item["identity"] for item in coverage["collisions"]
                )
                details = "; ".join(
                    value
                    for value in (
                        f"unbound: {unbound}" if unbound else "",
                        f"multiply bound: {repeated}" if repeated else "",
                        f"identity collisions: {collided}" if collided else "",
                    )
                    if value
                )
                raise StructureError(
                    f"{package.name}: public API vocabulary coverage failed: {details}"
                )
    missing_coverage = sorted(required_coverage - covered_packages)
    if missing_coverage:
        raise StructureError(
            "public API coverage names unknown packages: " + ", ".join(missing_coverage)
        )
    entries = []
    terms_by_name = {term["name"]: term for term in terms}
    for name, structure in structures.items():
        for field in structure:
            structure[field] = sorted(
                {canonical_structure(item): item for item in structure[field]}.values(),
                key=canonical_structure,
            )
        interface, review_problems = human_review_inventory(
            terms_by_name[name],
            structure,
            catalog,
            documentation,
            semantic_crates=semantic_crates,
        )
        packet = {
            "interface": interface,
            "review_problems": review_problems,
            "structure": structure,
        }
        encoded_bytes = len(canonical_structure(packet).encode("utf-8"))
        if encoded_bytes > MAXIMUM_TERM_STRUCTURE_BYTES:
            raise StructureError(
                f"{name}: vocabulary review packet exceeds bound: "
                f"{encoded_bytes} > {MAXIMUM_TERM_STRUCTURE_BYTES} bytes"
            )
        entries.append({"name": name, **packet})
    return {
        "cargo_public_api": public_api.PUBLIC_API_VERSION,
        "format": FORMAT_VERSION,
        "inputs_sha256": input_fingerprint(root),
        "rustdoc_toolchain": public_api.RUSTDOC_TOOLCHAIN,
        "terms": entries,
    }


def read_snapshot(path: Path) -> tuple[dict[str, dict[str, Any]], list[str]]:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return {}, [f"cannot read structural snapshot: {error}"]
    problems: list[str] = []
    if raw.get("format") != FORMAT_VERSION:
        problems.append(f"structural snapshot format must be {FORMAT_VERSION}")
    if raw.get("cargo_public_api") != public_api.PUBLIC_API_VERSION:
        problems.append("structural snapshot cargo-public-api version is stale")
    if raw.get("rustdoc_toolchain") != public_api.RUSTDOC_TOOLCHAIN:
        problems.append("structural snapshot rustdoc toolchain is stale")
    terms: dict[str, dict[str, Any]] = {}
    for entry in raw.get("terms", []):
        name = entry.get("name")
        interface = entry.get("interface")
        review_problems = entry.get("review_problems")
        structure = entry.get("structure")
        if (
            not isinstance(name, str)
            or not isinstance(interface, list)
            or not isinstance(review_problems, list)
            or not isinstance(structure, dict)
        ):
            problems.append("structural snapshot contains an invalid term entry")
            continue
        if name in terms:
            problems.append(f"structural snapshot repeats term: {name}")
            continue
        if any(not isinstance(problem, str) for problem in review_problems):
            problems.append(f"{name}: review problems must be strings")
            continue
        if any(
            not isinstance(item, dict)
            or any(
                not isinstance(item.get(field), str) or not item[field].strip()
                for field in ("description", "kind", "path", "signature")
            )
            for item in interface
        ):
            problems.append(f"{name}: human interface contains an invalid item")
            continue
        if any(not isinstance(structure.get(field), list) for field in EMPTY_STRUCTURE):
            problems.append(f"{name}: compiler-derived structure is incomplete")
            continue
        visible = [item["signature"] for item in interface]
        bound = [
            item.get("declaration")
            for field in ("private_architectural_state", "public_api")
            for item in structure[field]
        ] + [
            f"pub use {item.get('source')} as {item.get('path')}"
            for item in structure["reexports"]
        ]
        concealed = [
            declaration
            for declaration in bound
            if not isinstance(declaration, str) or visible.count(declaration) != 1
        ]
        if concealed:
            problems.append(
                f"{name}: human interface does not expose every bound identity exactly once"
            )
            continue
        terms[name] = {
            "interface": interface,
            "review_problems": review_problems,
            "structure": structure,
        }
    return terms, problems


def snapshot_inputs_current(root: Path, path: Path) -> bool:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False
    return raw.get("inputs_sha256") == input_fingerprint(root)


def render_snapshot(snapshot: dict[str, Any]) -> str:
    return json.dumps(snapshot, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("update", "check"))
    parser.add_argument("--root", type=Path, default=Path.cwd())
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    expected = render_snapshot(compile_snapshot(root))
    path = root / SNAPSHOT_PATH
    if arguments.command == "update":
        path.write_text(expected, encoding="utf-8")
        print(f"updated {SNAPSHOT_PATH}")
        return 0
    actual = path.read_text(encoding="utf-8") if path.exists() else ""
    if actual != expected:
        print(
            "compiler-derived vocabulary structure is stale; run "
            "python3 tools/vocabulary_structure.py update",
            file=sys.stderr,
        )
        return 1
    print("compiler-derived vocabulary structure is current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
