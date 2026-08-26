#!/usr/bin/env python3
"""Maintain complete Rust crate public-API inventories in crate READMEs."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Sequence


PUBLIC_API_VERSION = "0.52.0"
RUSTDOC_TOOLCHAIN = "nightly-2026-07-07"
BEGIN_MARKER = "<!-- BEGIN crate-readme-api inventory -->"
END_MARKER = "<!-- END crate-readme-api inventory -->"
SECTION = f"""\
## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

{BEGIN_MARKER}
{END_MARKER}
"""
IDENTIFIER = r"(?:r#)?[A-Za-z_][A-Za-z0-9_]*"
FUNCTION = re.compile(
    r"^pub\s+(?:(?:const|async|unsafe)\s+)*(?:[A-Za-z]+\s+)?fn\s+"
)


@dataclass(frozen=True)
class Package:
    name: str
    crate_name: str
    directory: Path
    manifest: Path
    readme: Path


@dataclass(frozen=True, order=True)
class ApiItem:
    path: str
    kind: str
    signature: str = field(default="", compare=False)


@dataclass(frozen=True)
class CatalogEntry:
    purpose: str
    evidence: str
    example: str | None = None


class InventoryError(RuntimeError):
    """A user-actionable inventory failure."""


def run(
    command: Sequence[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def workspace_packages(root: Path) -> dict[str, Package]:
    result = run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=root,
    )
    if result.returncode:
        raise InventoryError(result.stderr.strip() or "cargo metadata failed")
    metadata = json.loads(result.stdout)
    packages: dict[str, Package] = {}
    for raw in metadata["packages"]:
        library = next(
            (
                target
                for target in raw["targets"]
                if set(target["kind"]) & {"lib", "rlib", "dylib", "proc-macro"}
            ),
            None,
        )
        if library is None:
            continue
        manifest = Path(raw["manifest_path"]).resolve()
        readme_value = raw.get("readme") or "README.md"
        readme = Path(readme_value)
        if not readme.is_absolute():
            readme = manifest.parent / readme
        packages[raw["name"]] = Package(
            name=raw["name"],
            crate_name=library["name"],
            directory=manifest.parent,
            manifest=manifest,
            readme=readme.resolve(),
        )
    return packages


def require_packages(
    available: dict[str, Package], names: Sequence[str]
) -> list[Package]:
    unknown = sorted(set(names) - available.keys())
    if unknown:
        raise InventoryError(f"unknown library crate(s): {', '.join(unknown)}")
    return [available[name] for name in names]


def extractor_command(package: Package, target_dir: Path) -> list[str]:
    return [
        "cargo",
        f"+{RUSTDOC_TOOLCHAIN}",
        "rustdoc",
        "--manifest-path",
        str(package.manifest),
        "--lib",
        "--all-features",
        "--locked",
        "--target-dir",
        str(target_dir),
        "--",
        "-Z",
        "unstable-options",
        "--output-format",
        "json",
        "--document-hidden-items",
    ]


def renderer_command(rustdoc_json: Path) -> list[str]:
    return [
        "cargo",
        f"+{RUSTDOC_TOOLCHAIN}",
        "public-api",
        "--rustdoc-json",
        str(rustdoc_json),
        "--omit",
        "blanket-impls",
        "--omit",
        "auto-trait-impls",
        "--omit",
        "auto-derived-impls",
        "--color",
        "never",
    ]


def verify_extractor(root: Path) -> None:
    result = run(
        ["cargo", f"+{RUSTDOC_TOOLCHAIN}", "public-api", "--version"], cwd=root
    )
    if result.returncode:
        raise InventoryError(
            f"missing pinned extractor; install with: cargo install "
            f"cargo-public-api --version {PUBLIC_API_VERSION} --locked\n"
            f"{result.stderr.strip()}"
        )
    expected = f"cargo-public-api {PUBLIC_API_VERSION}"
    if result.stdout.strip() != expected:
        raise InventoryError(
            f"expected {expected}, found {result.stdout.strip() or 'unknown version'}"
        )


def extract_public_api(root: Path, package: Package) -> str:
    verify_extractor(root)
    environment = os.environ.copy()
    for variable in ("CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS", "RUSTDOCFLAGS"):
        environment.pop(variable, None)
    target_dir = root / "target" / "crate-readme-api"
    result = run(extractor_command(package, target_dir), cwd=root, env=environment)
    if result.returncode:
        detail = result.stderr.strip() or result.stdout.strip()
        raise InventoryError(f"could not extract {package.name}:\n{detail}")
    rustdoc_json = target_dir / "doc" / f"{package.crate_name}.json"
    if not rustdoc_json.is_file():
        raise InventoryError(f"rustdoc did not produce {rustdoc_json}")
    rendered = run(renderer_command(rustdoc_json), cwd=root, env=environment)
    if rendered.returncode:
        detail = rendered.stderr.strip() or rendered.stdout.strip()
        raise InventoryError(f"could not render {package.name}:\n{detail}")
    return rendered.stdout


def exported_path(line: str, crate_name: str) -> str:
    if line.startswith("pub proc macro "):
        path = line.removeprefix("pub proc macro ").strip()
        return path.removesuffix("()")
    match = re.search(rf"\b{re.escape(crate_name)}(?:::{IDENTIFIER})*", line)
    if match is None:
        raise InventoryError(f"cannot find exported path in extractor line: {line}")
    path = match.group(0)
    if line.startswith("pub macro "):
        path += "!"
    return path


def public_lines(output: str, crate_name: str) -> list[tuple[str, str, str | None]]:
    """Return public lines with paths and any explicit trait implementation."""
    records: list[tuple[str, str, str | None]] = []
    implementation_type: str | None = None
    implementation_qualification: str | None = None
    external_implementation = False
    for raw_line in output.splitlines():
        line = raw_line.strip()
        if line.startswith(("impl ", "impl<")):
            before, separator, after = line.removeprefix("impl").strip().rpartition(
                " for "
            )
            if separator:
                owner = after.partition(" where ")[0]
                external_implementation = re.match(
                    rf"^{re.escape(crate_name)}(?:\b|::|<)", owner
                ) is None
                if external_implementation:
                    implementation_type = None
                    implementation_qualification = None
                    continue
                try:
                    implementation_type = exported_path(after, crate_name)
                except InventoryError:
                    implementation_type = None
                trait = without_impl_generics(before)
                implementation_qualification = (
                    f"<{owner} as {trait}>" if implementation_type else None
                )
            else:
                external_implementation = False
                implementation_type = None
                implementation_qualification = None
            continue
        if not line.startswith("pub "):
            continue
        if external_implementation:
            if FUNCTION.match(line) or line.startswith(("pub const ", "pub type ")):
                continue
            external_implementation = False
        path = exported_path(line, crate_name)
        parent = path.removesuffix("!").rpartition("::")[0]
        qualification = (
            implementation_qualification
            if implementation_type is not None and parent == implementation_type
            else None
        )
        if implementation_type is not None and parent != implementation_type:
            implementation_type = None
            implementation_qualification = None
        records.append((line, path, qualification))
    return records


def without_impl_generics(header: str) -> str:
    if not header.startswith("<"):
        return header
    depth = 0
    for index, character in enumerate(header):
        if character == "<":
            depth += 1
        elif character == ">":
            depth -= 1
            if depth == 0:
                return header[index + 1 :].strip()
    raise InventoryError(f"unclosed impl generics: {header}")


def qualified_path(path: str, qualification: str | None) -> str:
    if qualification is None:
        return path
    name = path.rpartition("::")[2]
    return f"{qualification}::{name}"


def item_sort_key(item: ApiItem) -> tuple[str, str, str]:
    path = item.path
    if path.startswith("<") and " as " in path and ">::" in path:
        owner = path[1:].split(" as ", maxsplit=1)[0]
        member = path.rsplit(">::", maxsplit=1)[1]
        path = f"{owner}::{member}"
    return path, item.kind, item.path


def split_top_level(text: str) -> list[str]:
    values: list[str] = []
    start = 0
    nesting = 0
    openers = {"(", "[", "{", "<"}
    closers = {")", "]", "}", ">"}
    for index, character in enumerate(text):
        if character in openers:
            nesting += 1
        elif character in closers:
            nesting = max(0, nesting - 1)
        elif character == "," and nesting == 0:
            values.append(text[start:index].strip())
            start = index + 1
    tail = text[start:].strip()
    if tail:
        values.append(tail)
    return values


def tuple_fields(line: str, path: str, *, enum_variant: bool) -> list[ApiItem]:
    start = line.find("(", line.find(path) + len(path))
    if start == -1:
        return []
    depth = 0
    end = -1
    for index in range(start, len(line)):
        if line[index] == "(":
            depth += 1
        elif line[index] == ")":
            depth -= 1
            if depth == 0:
                end = index
                break
    if end == -1:
        raise InventoryError(f"unclosed tuple declaration: {line}")
    fields = split_top_level(line[start + 1 : end])
    if enum_variant:
        return [
            ApiItem(f"{path}::{index}", "Public field", field)
            for index, field in enumerate(fields)
        ]
    return [
        ApiItem(f"{path}::{index}", "Public field", field)
        for index, field in enumerate(fields)
        if field.startswith("pub ")
    ]


def parse_public_api(output: str, crate_name: str) -> list[ApiItem]:
    records = public_lines(output, crate_name)
    typed: dict[str, str] = {}
    for line, path, _ in records:
        match = re.match(r"^pub\s+(mod|struct|enum|union|trait)\s+", line)
        if match:
            typed[path] = match.group(1)

    variants: set[str] = set()
    for line, path, _ in records:
        if re.match(
            r"^pub\s+(?:mod|struct|enum|union|trait|type|const|static|mut static|macro|proc macro)\s+",
            line,
        ) or FUNCTION.match(line):
            continue
        parent = path.rpartition("::")[0]
        if typed.get(parent) == "enum":
            variants.add(path)

    items: set[ApiItem] = set()
    nominal_labels = {
        "mod": "Module",
        "struct": "Struct",
        "enum": "Enum",
        "union": "Union",
        "trait": "Trait",
    }
    for line, path, trait in records:
        item_path = qualified_path(path, trait)
        nominal = re.match(r"^pub\s+(mod|struct|enum|union|trait)\s+", line)
        if nominal:
            keyword = nominal.group(1)
            items.add(ApiItem(path, nominal_labels[keyword], line))
            if keyword == "struct":
                items.update(tuple_fields(line, path, enum_variant=False))
            continue
        if re.match(r"^pub\s+type\s+", line):
            items.add(ApiItem(item_path, "Type alias", line))
        elif FUNCTION.match(line):
            parent = path.rpartition("::")[0]
            kind = (
                "Method"
                if typed.get(parent) in {"struct", "enum", "union", "trait"}
                else "Function"
            )
            items.add(ApiItem(item_path, kind, line))
        elif re.match(r"^pub\s+const\s+", line):
            items.add(ApiItem(item_path, "Constant", line))
        elif re.match(r"^pub\s+(?:mut\s+)?static\s+", line):
            items.add(ApiItem(item_path, "Static", line))
        elif line.startswith(("pub macro ", "pub proc macro ")):
            items.add(ApiItem(path, "Macro", line))
        elif path in variants:
            items.add(ApiItem(path, "Enum variant", line))
            items.update(tuple_fields(line, path, enum_variant=True))
        else:
            parent = path.rpartition("::")[0]
            if parent in typed or parent in variants:
                items.add(ApiItem(path, "Public field", line))
            else:
                raise InventoryError(f"unclassified extractor line: {line}")
    return sorted(items, key=item_sort_key)


def split_markdown_row(line: str) -> list[str]:
    cells: list[str] = []
    current: list[str] = []
    escaped = False
    for character in line.strip():
        if character == "|" and not escaped:
            cells.append("".join(current).strip())
            current = []
        else:
            current.append(character)
        if character == "\\" and not escaped:
            escaped = True
        else:
            escaped = False
    cells.append("".join(current).strip())
    if cells and not cells[0]:
        cells.pop(0)
    if cells and not cells[-1]:
        cells.pop()
    return cells


API_ITEM = re.compile(r"<!-- api-item (\{.*\}) -->")


def existing_catalog(body: str) -> dict[tuple[str, str], CatalogEntry]:
    catalog: dict[tuple[str, str], CatalogEntry] = {}
    lines = body.splitlines()

    if lines[:1] == ["| Kind | Item | Description |"]:
        for line in lines[2:]:
            cells = split_markdown_row(line)
            if len(cells) != 3:
                raise InventoryError(f"malformed managed inventory row: {line}")
            kind, rendered_path, purpose = cells
            purpose = purpose.replace("\\|", "|")
            if not (rendered_path.startswith("`") and rendered_path.endswith("`")):
                raise InventoryError(f"malformed managed API item: {rendered_path}")
            path = rendered_path[1:-1]
            key = (kind, path)
            if key in catalog:
                raise InventoryError(f"duplicate managed API item: {kind} {path}")
            catalog[key] = CatalogEntry(
                purpose=purpose,
                evidence=default_evidence(ApiItem(path, kind)),
            )
        return catalog

    owner_purpose: list[str] | None = None
    for line in lines:
        if line.startswith("### "):
            owner_purpose = []
            continue
        match = API_ITEM.search(line)
        if match is None:
            if owner_purpose is not None and line.strip():
                owner_purpose.append(line.strip())
            continue
        try:
            metadata = json.loads(match.group(1))
        except json.JSONDecodeError as error:
            raise InventoryError(f"malformed api-item metadata: {line}") from error
        kind = metadata.get("kind")
        path = metadata.get("item")
        evidence = metadata.get("evidence")
        if not all(isinstance(value, str) and value for value in (kind, path, evidence)):
            raise InventoryError(f"incomplete api-item metadata: {line}")
        if line.startswith("|"):
            cells = split_markdown_row(line)
            if len(cells) != 2:
                raise InventoryError(f"malformed managed inventory row: {line}")
            purpose = cells[1].replace("\\|", "|")
        else:
            purpose = " ".join(owner_purpose or []).strip()
            owner_purpose = None
        key = (kind, path)
        if key in catalog:
            raise InventoryError(f"duplicate managed API item: {kind} {path}")
        example = metadata.get("example")
        if example is not None and not isinstance(example, str):
            raise InventoryError(f"invalid api-item example metadata: {line}")
        catalog[key] = CatalogEntry(
            purpose=purpose,
            evidence=evidence,
            example=example,
        )
    return catalog


def managed_region(readme: str) -> tuple[int, int, str] | None:
    starts = [match.start() for match in re.finditer(re.escape(BEGIN_MARKER), readme)]
    ends = [match.start() for match in re.finditer(re.escape(END_MARKER), readme)]
    if not starts and not ends:
        return None
    if len(starts) != 1 or len(ends) != 1 or starts[0] >= ends[0]:
        raise InventoryError("README must contain exactly one ordered inventory marker pair")
    body_start = starts[0] + len(BEGIN_MARKER)
    return body_start, ends[0], readme[body_start : ends[0]].strip("\n")


def default_evidence(item: ApiItem) -> str:
    detail = item.signature or item.path
    return f"cargo-public-api@{PUBLIC_API_VERSION}: {detail}"


def current_evidence(item: ApiItem, entry: CatalogEntry | None) -> str:
    if entry is None or entry.evidence.startswith("cargo-public-api@"):
        return default_evidence(item)
    return entry.evidence


def default_purpose(item: ApiItem, owner: str | None = None) -> str:
    if owner is None:
        return f"Public {item.kind.lower()} `{item.path}`."
    return f"Public {item.kind.lower()} owned by `{owner}`."


def item_metadata(item: ApiItem, evidence: str, example: str | None) -> str:
    values = {
        "kind": item.kind,
        "item": item.path,
        "signature": item.signature or item.path,
        "evidence": evidence,
    }
    if example:
        values["example"] = example
    metadata = json.dumps(
        values,
        ensure_ascii=True,
        separators=(",", ":"),
    )
    return f"<!-- api-item {metadata.replace('|', r'\u007c')} -->"


def owner_path(item: ApiItem, owners: set[str]) -> str:
    path = item.path
    if path.startswith("<") and " as " in path and ">::" in path:
        path = path[1:].split(" as ", maxsplit=1)[0]
        path = path.split("<", maxsplit=1)[0]
    candidates = [
        owner
        for owner in owners
        if path == owner or path.startswith(f"{owner}::")
    ]
    if not candidates:
        raise InventoryError(f"public API item has no owning module/type: {item.path}")
    return max(candidates, key=len)


def owner_label(item: ApiItem) -> str:
    if item.kind == "Module":
        return item.path
    return item.path.rpartition("::")[2]


def leaf_label(item: ApiItem, owner: str) -> str:
    if item.path.startswith("<") and " as " in item.path and ">::" in item.path:
        trait = item.path.split(" as ", maxsplit=1)[1].split(">::", maxsplit=1)[0]
        return f"{trait}::{item.path.rsplit('>::', maxsplit=1)[1]}"
    relative = item.path.removeprefix(f"{owner}::")
    if item.kind == "Public field" and "::" in relative:
        parent, _, field_name = relative.rpartition("::")
        return f"Field `{field_name}` of `{parent}`"
    return relative


def escape_purpose(value: str) -> str:
    return " ".join(value.split()).replace("|", "\\|")


def preserved_examples(body: str) -> dict[tuple[str, str], str]:
    examples: dict[tuple[str, str], str] = {}
    for section in re.split(r"(?=^### )", body, flags=re.MULTILINE):
        metadata = API_ITEM.search(section)
        anchor = re.search(r"(?m)^<a id=", section)
        if metadata is None or anchor is None:
            continue
        values = json.loads(metadata.group(1))
        key = (values.get("kind"), values.get("item"))
        if all(isinstance(value, str) for value in key):
            examples[key] = section[anchor.start() :].strip()
    return examples


def render_body(
    items: Sequence[ApiItem],
    catalog: dict[tuple[str, str], CatalogEntry],
    examples: dict[tuple[str, str], str],
) -> str:
    owner_kinds = {"Module", "Struct", "Enum", "Union", "Trait"}
    owners = {item.path for item in items if item.kind in owner_kinds}
    owner_items = {item.path: item for item in items if item.path in owners}
    grouped: dict[str, list[ApiItem]] = {owner: [] for owner in owners}
    for item in items:
        if item.path in owners:
            continue
        grouped[owner_path(item, owners)].append(item)

    lines: list[str] = []
    for owner in sorted(owners, key=lambda path: item_sort_key(owner_items[path])):
        item = owner_items[owner]
        entry = catalog.get((item.kind, item.path))
        purpose = entry.purpose if entry and entry.purpose else default_purpose(item)
        evidence = current_evidence(item, entry)
        lines.extend(
            [
                f"### `{owner_label(item)}` ({item.kind})",
                "",
                escape_purpose(purpose),
                item_metadata(item, evidence, entry.example if entry else None),
            ]
        )
        if entry and entry.example:
            anchor = entry.example.lower()
            lines.append(f"Example coverage: [{entry.example}](#{anchor}).")
        leaves = sorted(grouped[owner], key=item_sort_key)
        lines.extend(["", "| Item | Purpose |", "| --- | --- |"])
        if leaves:
            for leaf in leaves:
                leaf_entry = catalog.get((leaf.kind, leaf.path))
                leaf_purpose = (
                    leaf_entry.purpose
                    if leaf_entry and leaf_entry.purpose
                    else default_purpose(leaf, owner)
                )
                leaf_evidence = current_evidence(leaf, leaf_entry)
                lines.append(
                    f"| **`{leaf_label(leaf, owner)}`**<br><sub>{leaf.kind}</sub>"
                    f"{item_metadata(leaf, leaf_evidence, leaf_entry.example if leaf_entry else None)} | {escape_purpose(leaf_purpose)} |"
                )
        else:
            lines.append("| _No standalone item_ | Public API is described by its owning sections. |")
        example = examples.get((item.kind, item.path))
        if example:
            lines.extend(["", example])
        lines.append("")
    return "\n".join(lines).rstrip()


def expected_readme(readme: str, items: Sequence[ApiItem]) -> str:
    region = managed_region(readme)
    if region is None:
        if not readme or readme.endswith("\n\n"):
            separator = ""
        elif readme.endswith("\n"):
            separator = "\n"
        else:
            separator = "\n\n"
        base = readme + separator + SECTION
        region = managed_region(base)
        assert region is not None
        readme = base
    body_start, body_end, old_body = region
    catalog = existing_catalog(old_body) if old_body else {}
    examples = preserved_examples(old_body) if old_body else {}
    body = render_body(items, catalog, examples)
    return readme[:body_start] + "\n" + body + "\n" + readme[body_end:]


def inventory_for(root: Path, package: Package) -> list[ApiItem]:
    return parse_public_api(extract_public_api(root, package), package.crate_name)


def update_package(root: Path, package: Package) -> None:
    original = (
        package.readme.read_text(encoding="utf-8")
        if package.readme.is_file()
        else f"# {package.name}\n"
    )
    updated = expected_readme(original, inventory_for(root, package))
    if updated != original:
        package.readme.write_text(updated, encoding="utf-8")
        print(f"updated {package.readme.relative_to(root)}")
    else:
        print(f"current {package.readme.relative_to(root)}")


def check_package(root: Path, package: Package) -> str | None:
    if not package.readme.is_file():
        return f"{package.name}: missing crate README at {package.readme.relative_to(root)}"
    original = package.readme.read_text(encoding="utf-8")
    if managed_region(original) is None:
        return f"{package.name}: missing README public-API inventory"
    expected = expected_readme(original, inventory_for(root, package))
    if expected != original:
        return (
            f"{package.name}: stale README public-API inventory; run "
            f"python3 tools/crate_readme_api.py update {package.name}"
        )
    return None


def changed_paths(root: Path, base: str, head: str) -> list[Path]:
    if re.fullmatch(r"0+", base):
        return [Path("Cargo.toml")]
    result = run(
        ["git", "diff", "--name-only", "--diff-filter=ACDMR", f"{base}...{head}"],
        cwd=root,
    )
    if result.returncode:
        raise InventoryError(result.stderr.strip() or "git diff failed")
    return [Path(line) for line in result.stdout.splitlines() if line]


def modified_packages(
    root: Path, packages: dict[str, Package], paths: Sequence[Path]
) -> list[Package]:
    if Path("Cargo.toml") in paths:
        return sorted(packages.values(), key=lambda package: package.name)
    changed = {(root / path).resolve() for path in paths}
    return sorted(
        (
            package
            for package in packages.values()
            if any(
                path == package.directory or package.directory in path.parents
                for path in changed
            )
        ),
        key=lambda package: package.name,
    )


def parser() -> argparse.ArgumentParser:
    command = argparse.ArgumentParser(description=__doc__)
    subcommands = command.add_subparsers(dest="command", required=True)
    for name in ("update", "check"):
        subcommand = subcommands.add_parser(name)
        subcommand.add_argument("crates", nargs="+", metavar="CRATE")
    modified = subcommands.add_parser("check-modified")
    modified.add_argument("--base", required=True)
    modified.add_argument("--head", default="HEAD")
    return command


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    root = Path(__file__).resolve().parents[1]
    try:
        available = workspace_packages(root)
        if args.command == "check-modified":
            packages = modified_packages(
                root, available, changed_paths(root, args.base, args.head)
            )
        else:
            packages = require_packages(available, args.crates)
        if args.command == "update":
            for package in packages:
                update_package(root, package)
            return 0
        problems = [
            problem
            for package in packages
            if (problem := check_package(root, package))
        ]
    except (InventoryError, OSError, json.JSONDecodeError) as error:
        print(f"crate README API inventory error: {error}", file=sys.stderr)
        return 2
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1
    if packages:
        print(
            "current README public-API inventories: "
            + ", ".join(package.name for package in packages)
        )
    else:
        print("no modified library crates")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
