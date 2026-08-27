#!/usr/bin/env python3
"""Build the human-readable half of deterministic vocabulary review packets."""

from __future__ import annotations

import json
import re
from itertools import product
from pathlib import Path
from typing import Any, Sequence

import crate_readme_api as public_api

MISSING_DESCRIPTION = (
    "Review blocked: no human description is bound for this compiler-visible item."
)
TAUTOLOGICAL_DESCRIPTIONS = (
    re.compile(r"^Compiler-visible (?:.+ owned by|(?:struct|enum|trait|union|module)?) ?.+\.$"),
    re.compile(r"^Provides the compiler-visible .+ shown below\.$"),
    re.compile(r"^Exposes the .+ value with the exact type shown below\.$"),
    re.compile(r"^Represents the .+ case of the containing enum\.$"),
    re.compile(r"^Names the exact type shown below\.$"),
    re.compile(r"^Bound implementation state for this term\.$"),
    re.compile(r"^Implements `.+` for `.+`\.$"),
)


class ReviewError(RuntimeError):
    """A stale or ambiguous human interface catalog."""


def _single_line(value: str) -> str:
    normalized = re.sub(r"<br\s*/?>", " ", value, flags=re.IGNORECASE)
    normalized = re.sub(r"\s*Example:\s*\[[^]]+\]\([^)]*\)\.?\s*$", "", normalized)
    return " ".join(normalized.replace("\\|", "|").split())


def public_api_catalog(
    root: Path, packages: Sequence[public_api.Package] | None = None
) -> dict[str, dict[str, str]]:
    """Human descriptions keyed by exact compiler-visible public path."""
    catalog: dict[str, dict[str, str]] = {}
    available = packages or tuple(public_api.workspace_packages(root).values())
    for package in sorted(available, key=lambda value: value.name):
        if not package.readme.is_file():
            continue
        managed = public_api.managed_region(package.readme.read_text(encoding="utf-8"))
        if managed is None:
            continue
        metadata_by_identity: dict[tuple[str, str], dict[str, Any]] = {}
        for match in public_api.API_ITEM.finditer(managed[2]):
            metadata = json.loads(match.group(1))
            path = metadata.get("item")
            kind = metadata.get("kind")
            if isinstance(path, str) and isinstance(kind, str):
                metadata_by_identity[(kind, path)] = metadata
        for (kind, path), entry in public_api.existing_catalog(managed[2]).items():
            metadata = metadata_by_identity.get((kind, path), {})
            signature = metadata.get("signature", "")
            if not isinstance(signature, str):
                raise ReviewError(f"invalid human interface signature: {path}")
            key = f"{path}\0{signature}" if signature else path
            if key in catalog:
                raise ReviewError(f"duplicate human interface description: {path}")
            value = {
                "kind": kind,
                "purpose": _single_line(entry.purpose),
                "signature": signature,
            }
            catalog[key] = value
            catalog.setdefault(path, value)
    return catalog


def _constructor(kind: str, path: str, signature: str) -> str:
    if kind != "Method" or "fn " not in signature:
        return kind
    leaf = path.rsplit("::", maxsplit=1)[-1]
    arguments = signature.partition("(")[2].partition(")")[0]
    if not any(receiver in arguments for receiver in ("self", "&self", "&mut self")) and (
        leaf == "new"
        or leaf.startswith("new_")
        or leaf.startswith("from_")
        or leaf.startswith("try_")
    ):
        return "Constructor"
    return kind


def _fallback_kind(declaration: str) -> str:
    for prefix, kind in (
        ("pub struct ", "Struct"),
        ("pub enum ", "Enum"),
        ("pub trait ", "Trait"),
        ("pub type ", "Type alias"),
        ("pub const ", "Constant"),
        ("pub fn ", "Function"),
        ("pub async fn ", "Function"),
        ("pub const fn ", "Function"),
    ):
        if declaration.startswith(prefix):
            return kind
    return "Compiler-visible item"


def _readable_docs(value: Any) -> str | None:
    if not isinstance(value, str) or not value.strip():
        return None
    kept: list[str] = []
    fenced = False
    for line in value.splitlines():
        if line.strip().startswith("```"):
            fenced = not fenced
            continue
        if fenced:
            continue
        stripped = line.strip()
        if stripped.startswith("# "):
            stripped = stripped.removeprefix("# ") + ":"
        if stripped:
            kept.append(stripped)
    return _single_line(" ".join(kept)) or None


def _field_ids(kind: Any) -> list[str]:
    if not isinstance(kind, dict):
        return []
    for value in kind.values():
        if isinstance(value, dict) and isinstance(value.get("fields"), list):
            return [str(item) for item in value["fields"]]
    return []


def rustdoc_descriptions(
    rustdoc: dict[str, Any], aliases: list[dict[str, Any]]
) -> dict[str, str]:
    """Human rustdoc text projected onto every canonical and re-export root."""
    index = rustdoc["index"]
    roots: dict[str, set[str]] = {}
    for item_id, value in rustdoc.get("paths", {}).items():
        if value.get("crate_id") == 0:
            roots.setdefault(str(item_id), set()).add("::".join(value["path"]))
    for alias in aliases:
        roots.setdefault(alias["target"], set()).add(alias["path"])
    descriptions: dict[str, str] = {}

    def retain(path: str, item: dict[str, Any]) -> None:
        docs = _readable_docs(item.get("docs"))
        if docs:
            descriptions[path] = docs

    def arguments(value: Any) -> list[str]:
        if value is None:
            return [""]
        if not isinstance(value, dict):
            return []
        angle = value.get("angle_bracketed")
        if not isinstance(angle, dict):
            return []
        rendered: list[list[str]] = []
        for argument in angle.get("args", []):
            if not isinstance(argument, dict):
                return []
            if "type" in argument:
                names = type_names(argument["type"])
            elif "lifetime" in argument:
                names = [str(argument["lifetime"])]
            elif "const" in argument:
                constant = argument["const"]
                names = [str(constant.get("expr", ""))] if isinstance(constant, dict) else []
            else:
                names = []
            if not names:
                return []
            rendered.append(names)
        return [
            "<" + ", ".join(values) + ">" if values else ""
            for values in product(*rendered)
        ] if rendered else [""]

    def resolved_names(value: Any) -> list[str]:
        if not isinstance(value, dict):
            return []
        item_id = str(value.get("id"))
        bases = roots.get(item_id)
        if not bases:
            path = rustdoc.get("paths", {}).get(item_id, {}).get("path")
            bases = {"::".join(path)} if isinstance(path, list) else set()
        suffixes = arguments(value.get("args"))
        return sorted(f"{base}{suffix}" for base in bases for suffix in suffixes)

    def type_names(value: Any) -> list[str]:
        if not isinstance(value, dict):
            return []
        if "resolved_path" in value:
            return resolved_names(value["resolved_path"])
        if "generic" in value:
            return [str(value["generic"])]
        if "primitive" in value:
            return [str(value["primitive"])]
        if "borrowed_ref" in value:
            reference = value["borrowed_ref"]
            if not isinstance(reference, dict):
                return []
            prefix = "&"
            if reference.get("lifetime"):
                prefix += str(reference["lifetime"]) + " "
            if reference.get("is_mutable"):
                prefix += "mut "
            return [prefix + name for name in type_names(reference.get("type"))]
        if "slice" in value:
            return [f"[{name}]" for name in type_names(value["slice"])]
        if "tuple" in value and isinstance(value["tuple"], list):
            members = [type_names(member) for member in value["tuple"]]
            if any(not member for member in members):
                return []
            return ["(" + ", ".join(values) + ")" for values in product(*members)]
        return []

    for item_id, exported_roots in roots.items():
        item = index.get(item_id)
        if item is None:
            continue
        inner = item["inner"]
        for exported_root in exported_roots:
            retain(exported_root, item)
        if "struct" in inner:
            fields = _field_ids(inner["struct"].get("kind"))
            impls = inner["struct"].get("impls", [])
            variants: list[str] = []
        elif "union" in inner:
            fields = [str(value) for value in inner["union"].get("fields", [])]
            impls = inner["union"].get("impls", [])
            variants = []
        elif "enum" in inner:
            fields = []
            impls = inner["enum"].get("impls", [])
            variants = [str(value) for value in inner["enum"].get("variants", [])]
        elif "trait" in inner:
            fields = []
            impls = []
            variants = []
            for child_id in inner["trait"].get("items", []):
                child = index.get(str(child_id))
                if child is None or not child.get("name"):
                    continue
                for exported_root in exported_roots:
                    retain(f"{exported_root}::{child['name']}", child)
        else:
            continue
        for field_id in fields:
            field = index.get(field_id)
            if field is None or not field.get("name"):
                continue
            for exported_root in exported_roots:
                retain(f"{exported_root}::{field['name']}", field)
        for variant_id in variants:
            variant = index.get(variant_id)
            if variant is None or not variant.get("name"):
                continue
            for exported_root in exported_roots:
                variant_path = f"{exported_root}::{variant['name']}"
                retain(variant_path, variant)
                variant_kind = variant["inner"]["variant"].get("kind")
                for field_id in _field_ids(variant_kind):
                    field = index.get(field_id)
                    if field is not None and field.get("name"):
                        retain(f"{variant_path}::{field['name']}", field)
        for impl_id in impls:
            implementation = index.get(str(impl_id), {}).get("inner", {}).get("impl")
            if not isinstance(implementation, dict):
                continue
            trait = implementation.get("trait")
            trait_path = trait.get("path") if isinstance(trait, dict) else None
            for child_id in implementation.get("items", []):
                child = index.get(str(child_id))
                if child is None or not child.get("name"):
                    continue
                for exported_root in exported_roots:
                    retain(f"{exported_root}::{child['name']}", child)
                    if isinstance(trait_path, str):
                        retain(f"<{exported_root} as {trait_path}>::{child['name']}", child)

    for item in index.values():
        implementation = item.get("inner", {}).get("impl")
        if not isinstance(implementation, dict) or implementation.get("blanket_impl") is not None:
            continue
        trait = implementation.get("trait")
        if not isinstance(trait, dict):
            continue
        owners = type_names(implementation.get("for"))
        traits = resolved_names(trait)
        if not owners or not traits:
            continue
        for child_id in implementation.get("items", []):
            child = index.get(str(child_id))
            if child is None or not child.get("name"):
                continue
            for owner, trait_name in product(owners, traits):
                retain(f"<{owner} as {trait_name}>::{child['name']}", child)
    return descriptions


def _generated_description(kind: str, path: str) -> str:
    if path.startswith("<") and " as " in path and ">::" in path:
        owner, trait_and_member = path[1:].split(" as ", maxsplit=1)
        trait, member = trait_and_member.split(">::", maxsplit=1)
        if member == "fmt":
            return "Formats this value for human-readable output."
        return f"Implements `{trait}::{member}` for `{owner}`."
    leaf = path.rsplit("::", maxsplit=1)[-1]
    if kind == "Public field":
        return f"Exposes the `{leaf}` value with the exact type shown below."
    if kind == "Enum variant":
        return f"Represents the `{leaf}` case of the containing enum."
    if kind == "Type alias":
        return "Names the exact type shown below."
    return f"Provides the compiler-visible `{leaf}` {kind.lower()} shown below."


def tautological_description(description: str) -> bool:
    """Whether prose merely restates compiler visibility, kind, or name."""
    normalized = _single_line(description)
    return any(pattern.fullmatch(normalized) for pattern in TAUTOLOGICAL_DESCRIPTIONS)


def human_review_inventory(
    term: dict[str, Any],
    structure: dict[str, Any],
    catalog: dict[str, dict[str, str]],
    documentation: dict[str, str] | None = None,
    semantic_crates: set[str] | None = None,
) -> tuple[list[dict[str, str]], list[str]]:
    """Readable one-to-one projection of every bound structural identity."""
    interface: list[dict[str, str]] = []
    problems: list[str] = []
    meaning = _single_line(str(term.get("meaning", "")))
    lifecycle = _single_line(str(term.get("lifecycle", "")))
    private_description = meaning
    if lifecycle:
        private_description = f"{private_description} Lifecycle: {lifecycle}".strip()
    if not private_description:
        private_description = "Bound implementation state for this term."

    for record in structure["private_architectural_state"]:
        interface.append({
            "description": private_description,
            "kind": "Bound declaration",
            "path": record["path"],
            "signature": record["declaration"],
        })
    for record in structure["public_api"]:
        path = record["path"]
        signature = record["declaration"]
        described = catalog.get(f"{path}\0{signature}")
        if described is None:
            fallback = catalog.get(path)
            described = (
                fallback
                if fallback is not None
                and (not fallback["signature"] or fallback["signature"] == signature)
                else None
            )
        if described is None and record.get("binding_roots"):
            member = path.rsplit("::", maxsplit=1)[-1]
            for root in record["binding_roots"]:
                described = catalog.get(f"{root}::{member}")
                if described is not None:
                    break
        if described is None and documentation is not None:
            kind = _fallback_kind(signature)
            description = documentation.get(path) or _generated_description(kind, path)
        elif described is None:
            kind = _fallback_kind(signature)
            description = MISSING_DESCRIPTION
            problems.append(f"{path}: missing human interface description")
        else:
            kind = described["kind"]
            description = described["purpose"]
            if not description:
                description = MISSING_DESCRIPTION
                problems.append(f"{path}: missing human interface description")
        interface.append({
            "description": description,
            "kind": _constructor(kind, path, signature),
            "path": path,
            "signature": signature,
        })
    for record in structure["reexports"]:
        path = record["path"]
        source = record["source"]
        interface.append({
            "description": f"Exports this term at `{path}` from `{source}`.",
            "kind": "Public export",
            "path": path,
            "signature": f"pub use {source} as {path}",
        })
    public_records = {
        (record["path"], record["declaration"]): record
        for record in structure["public_api"]
    }
    for item in interface:
        record = public_records.get((item["path"], item["signature"]), {})
        crates = {
            path.lstrip("<").split("::", maxsplit=1)[0]
            for path in [item["path"], *record.get("binding_roots", [])]
        }
        if (
            item["description"] != MISSING_DESCRIPTION
            and tautological_description(item["description"])
            and (semantic_crates is None or bool(crates & semantic_crates))
        ):
            problems.append(
                f"{item['path']}: tautological human interface description"
            )
    return interface, problems
