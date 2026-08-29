"""Bounded scenario admission and typed REPL evidence capture."""

from __future__ import annotations

import json
from typing import Any

from harness_safety import (
    MAX_ASSERTIONS,
    MAX_FILTER_BYTES,
    MAX_JSONL_ROWS,
    HarnessError,
)


def validate_executable_scenario(scenario: dict[str, Any]) -> None:
    if "required_facts" in scenario:
        raise HarnessError("executable scenario must convert required_facts into concrete assertions")
    if not isinstance(scenario.get("command_file"), str) or not scenario["command_file"]:
        raise HarnessError("executable scenario requires a command_file")
    if scenario.get("app_exit", "zero") not in {"zero", "nonzero"}:
        raise HarnessError("executable scenario app_exit must be 'zero' or 'nonzero'")
    assertions = scenario.get("assertions")
    stages = scenario.get("stages")
    if assertions is not None and stages is not None:
        raise HarnessError("executable scenario may use assertions or stages, not both")
    if stages is not None:
        if not isinstance(stages, list) or not stages:
            raise HarnessError("executable scenario requires nonempty bounded assertion stages")
        assertions = []
        previous = 0
        for number, stage in enumerate(stages, 1):
            if not isinstance(stage, dict) or not isinstance(stage.get("after_line"), int):
                raise HarnessError(f"scenario stage {number} was not bounded")
            if not previous < stage["after_line"] <= MAX_JSONL_ROWS:
                raise HarnessError("scenario stages must have strictly increasing after_line values")
            previous = stage["after_line"]
            staged = stage.get("assertions")
            if not isinstance(staged, list) or not staged:
                raise HarnessError(f"scenario stage {number} requires concrete assertions")
            assertions.extend(staged)
    if not isinstance(assertions, list) or not assertions:
        raise HarnessError("executable scenario requires at least one concrete assertion")
    if len(assertions) > MAX_ASSERTIONS:
        raise HarnessError(f"executable scenario exceeded {MAX_ASSERTIONS} assertions")
    for number, assertion in enumerate(assertions, 1):
        if not isinstance(assertion, dict) or assertion.get("relay") not in {"group", "state"}:
            raise HarnessError(f"scenario assertion {number} named an unsupported relay")
        if not isinstance(assertion.get("present"), bool):
            raise HarnessError(f"scenario assertion {number} must state present explicitly")
        event_filter = assertion.get("filter")
        if not isinstance(event_filter, dict) or not event_filter:
            raise HarnessError(f"scenario assertion {number} requires a nonempty relay filter")
        if len(json.dumps(event_filter, separators=(",", ":")).encode()) > MAX_FILTER_BYTES:
            raise HarnessError(f"scenario assertion {number} filter exceeded {MAX_FILTER_BYTES} bytes")
        if assertion["present"] and assertion.get("collection") == "relay-state":
            if assertion.get("required_kinds") != [39000, 39001, 39002, 39003]:
                raise HarnessError(f"scenario assertion {number} has invalid relay-state kinds")
        elif assertion["present"] and not {"id", "pubkey", "kind", "content", "tags"}.issubset(assertion.get("event", {})):
            raise HarnessError(f"scenario assertion {number} requires exact id, pubkey, kind, content, and tags")
        elif not assertion["present"] and "event" in assertion:
            raise HarnessError(f"negative scenario assertion {number} must not carry an ignored event")


def result_captures(rows: list[dict[str, Any]], definitions: dict[str, Any]) -> dict[str, dict[str, Any]]:
    captures: dict[str, dict[str, Any]] = {}
    for name, selector in definitions.items():
        matches = [row for row in rows if row.get("kind") == selector.get("kind") and isinstance(row.get("fields"), dict) and all(row["fields"].get(key) == value for key, value in selector.get("fields", {}).items())]
        if len(matches) != 1 or not isinstance(matches[0].get("fields"), dict):
            raise HarnessError(f"capture {name!r} matched {len(matches)} application result rows")
        captures[name] = matches[0]["fields"]
    return captures


def resolve(value: Any, context: dict[str, Any]) -> Any:
    if isinstance(value, str) and value.startswith("$"):
        current: Any = context
        for segment in value[1:].split("."):
            if not isinstance(current, dict) or segment not in current:
                raise HarnessError(f"scenario reference {value!r} was unavailable")
            current = current[segment]
        return current
    if isinstance(value, list):
        return [resolve(item, context) for item in value]
    if isinstance(value, dict):
        return {key: resolve(item, context) for key, item in value.items()}
    return value
