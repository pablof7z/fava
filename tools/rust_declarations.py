#!/usr/bin/env python3
"""Recognise nominal Rust declarations well enough to judge vocabulary.

The vocabulary gate must tell a declaration from an associated item: a
`type IntoIter = ...;` inside an `impl IntoIterator` block introduces no new
architectural noun, and a line-anchored regex cannot see the difference. This
module does the smallest amount of lexing that makes the distinction sound —
comments and literals are blanked, then brace nesting is tracked so every
declaration is attributed to its enclosing block.
"""

from __future__ import annotations

import re


DECLARATION_KEYWORDS = frozenset({"struct", "enum", "trait", "type", "union"})
DECLARATION_MODIFIERS = frozenset(
    {"pub", "restricted", "unsafe", "default", "async", "const", "extern"}
)
RUST_TOKEN = re.compile(r"[A-Za-z_][A-Za-z0-9_]*|.", re.DOTALL)
IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
RAW_STRING_OPENER = re.compile(r"(?:b?r)(#*)\"")
CHARACTER_LITERAL = re.compile(r"'(?:\\.|[^\\'])'")


def strip_rust_noise(text: str) -> str:
    """Blank out comments and literals so brace nesting can be trusted."""
    output: list[str] = []
    index = 0
    length = len(text)
    while index < length:
        character = text[index]
        if character == "/" and text.startswith("//", index):
            end = text.find("\n", index)
            end = length if end == -1 else end
            output.append(" " * (end - index))
            index = end
            continue
        if character == "/" and text.startswith("/*", index):
            depth = 0
            scan = index
            while scan < length:
                if text.startswith("/*", scan):
                    depth += 1
                    scan += 2
                elif text.startswith("*/", scan):
                    depth -= 1
                    scan += 2
                    if depth == 0:
                        break
                else:
                    scan += 1
            output.append("".join(" " if c != "\n" else "\n" for c in text[index:scan]))
            index = scan
            continue
        raw = RAW_STRING_OPENER.match(text, index)
        if raw and (index == 0 or not IDENTIFIER.fullmatch(text[index - 1])):
            terminator = '"' + raw.group(1)
            end = text.find(terminator, raw.end())
            end = length if end == -1 else end + len(terminator)
            output.append("".join(" " if c != "\n" else "\n" for c in text[index:end]))
            index = end
            continue
        if character == '"':
            scan = index + 1
            while scan < length:
                if text[scan] == "\\":
                    scan += 2
                    continue
                if text[scan] == '"':
                    scan += 1
                    break
                scan += 1
            output.append("".join(" " if c != "\n" else "\n" for c in text[index:scan]))
            index = scan
            continue
        if character == "'":
            literal = CHARACTER_LITERAL.match(text, index)
            if literal:
                output.append(" " * (literal.end() - index))
                index = literal.end()
                continue
        output.append(character)
        index += 1
    return "".join(output)


def nominal_declarations(text: str) -> list[tuple[str, bool]]:
    """Return every nominal declaration in one Rust file as (name, is_public).

    Associated items inside `impl` and `trait` bodies are not declarations of
    new architectural nouns, so `type IntoIter = ...;` inside an
    `impl IntoIterator` block is excluded.
    """
    source = strip_rust_noise(text)
    declarations: list[tuple[str, bool]] = []
    block_kinds: list[str] = []
    segment: list[str] = []
    parentheses = 0
    brackets = 0
    for match in RUST_TOKEN.finditer(source):
        token = match.group(0)
        if token == "(":
            parentheses += 1
            if parentheses == 1 and segment and segment[-1] == "pub":
                segment[-1] = "restricted"
            continue
        if token == ")":
            parentheses = max(0, parentheses - 1)
            continue
        if token == "[":
            brackets += 1
            continue
        if token == "]":
            brackets = max(0, brackets - 1)
            continue
        if parentheses or brackets:
            continue
        if token == "{":
            if "impl" in segment:
                block_kinds.append("impl")
            elif "trait" in segment:
                block_kinds.append("trait")
            else:
                block_kinds.append("other")
            segment = []
            continue
        if token == "}":
            if block_kinds:
                block_kinds.pop()
            segment = []
            continue
        if token == ";":
            segment = []
            continue
        if not IDENTIFIER.fullmatch(token):
            continue
        if (
            segment
            and segment[-1] in DECLARATION_KEYWORDS
            and token[0].isupper()
            and all(part in DECLARATION_MODIFIERS for part in segment[:-1])
        ):
            enclosing = block_kinds[-1] if block_kinds else None
            associated_item = segment[-1] == "type" and enclosing in {"impl", "trait"}
            if not associated_item:
                declarations.append((token, "pub" in segment[:-1]))
        segment.append(token)
    return declarations
