#!/usr/bin/env python3
"""MCP stdio server — E2E rules management.

Tools exposed:
  e2e_rules_add           create a rule file + regen README
  e2e_rules_edit          update fields on an existing rule + regen README
  e2e_rules_remove        delete a rule file + regen README
  e2e_rules_regen_readme  rewrite README from current rule files
"""
import json
import re
import sys
from pathlib import Path

RULES_DIR = Path(__file__).parent.resolve()


# ---------------------------------------------------------------------------
# Frontmatter helpers
# ---------------------------------------------------------------------------

def slugify(title: str) -> str:
    s = title.lower()
    s = re.sub(r"[^\w\s-]", "", s)
    s = re.sub(r"[\s_]+", "-", s)
    return re.sub(r"-+", "-", s).strip("-")


def _parse_fm(text: str) -> dict:
    result: dict = {}
    for line in text.strip().splitlines():
        m = re.match(r"^(\w+):\s*(.*)", line)
        if not m:
            continue
        key, val = m.group(1), m.group(2).strip()
        if val.startswith("["):
            result[key] = json.loads(val)
        elif val.startswith('"') or val.startswith("'"):
            result[key] = val[1:-1]
        else:
            result[key] = val
    return result


def _read_rule(path: Path) -> dict | None:
    text = path.read_text()
    if not text.startswith("---"):
        return None
    parts = text.split("---", 2)
    if len(parts) < 3:
        return None
    fm = _parse_fm(parts[1])
    return {"slug": path.stem, "file": path.name, **fm, "body": parts[2]}


def _list_rules() -> list[dict]:
    rules = []
    for p in sorted(RULES_DIR.glob("*.md")):
        if p.name == "README.md":
            continue
        r = _read_rule(p)
        if r:
            rules.append(r)
    return rules


def _write_rule(slug: str, title: str, summary: str, priority: str,
                questions: list[str], body: str | None = None) -> None:
    if body is None:
        body = (
            f"\n# {title}\n\n"
            + "## Questions\n\n"
            + "\n\n".join(f"- {q}" for q in questions)
            + "\n"
        )
    fm_lines = [
        f'title: "{title}"',
        f'summary: "{summary}"',
        f'priority: "{priority}"',
        f"questions: {json.dumps(questions)}",
    ]
    (RULES_DIR / f"{slug}.md").write_text(
        "---\n" + "\n".join(fm_lines) + "\n---\n" + body
    )


def _regen_readme() -> None:
    rules = _list_rules()
    rows = [
        "# E2E App Rules",
        "",
        "| Priority | Rule | Summary |",
        "| --- | --- | --- |",
    ]
    for r in rules:
        title = r.get("title", r["slug"])
        summary = r.get("summary", "")
        priority = r.get("priority", "")
        rows.append(f"| {priority} | [{title}]({r['file']}) | {summary} |")
    rows.append("")
    (RULES_DIR / "README.md").write_text("\n".join(rows))


# ---------------------------------------------------------------------------
# Tool registry
# ---------------------------------------------------------------------------

TOOLS = [
    {
        "name": "e2e_rules_add",
        "description": (
            "Add a new E2E rule. Writes examples/docs/rules/<slug>.md "
            "(slug auto-derived from title) and regenerates README."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "title":     {"type": "string", "description": "Rule title"},
                "summary":   {"type": "string", "description": "One-line summary"},
                "priority":  {"type": "string", "enum": ["MUST", "SHOULD", "MAY"]},
                "questions": {
                    "type": "array", "items": {"type": "string"},
                    "description": "Review checklist questions",
                },
                "body": {
                    "type": "string",
                    "description": "Custom markdown body (auto-generated from questions if omitted)",
                },
            },
            "required": ["title", "summary", "priority", "questions"],
        },
    },
    {
        "name": "e2e_rules_edit",
        "description": (
            "Edit an existing E2E rule identified by slug. "
            "Only supplied fields are updated; omitted fields keep their current values."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "slug":      {"type": "string", "description": "Filename stem (without .md)"},
                "title":     {"type": "string"},
                "summary":   {"type": "string"},
                "priority":  {"type": "string", "enum": ["MUST", "SHOULD", "MAY"]},
                "questions": {"type": "array", "items": {"type": "string"}},
                "body":      {"type": "string"},
            },
            "required": ["slug"],
        },
    },
    {
        "name": "e2e_rules_remove",
        "description": "Delete an E2E rule by slug and regenerate README.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "slug": {"type": "string", "description": "Filename stem (without .md)"},
            },
            "required": ["slug"],
        },
    },
    {
        "name": "e2e_rules_regen_readme",
        "description": "Regenerate examples/docs/rules/README.md from the current rule files.",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    },
]


def _handle(name: str, args: dict) -> dict:
    def ok(msg: str) -> dict:
        return {"content": [{"type": "text", "text": msg}]}

    def err(msg: str) -> dict:
        return {"content": [{"type": "text", "text": msg}], "isError": True}

    if name == "e2e_rules_add":
        slug = slugify(args["title"])
        path = RULES_DIR / f"{slug}.md"
        if path.exists():
            return err(f"Rule '{slug}' already exists. Use e2e_rules_edit to update it.")
        _write_rule(slug, args["title"], args["summary"], args["priority"],
                    args["questions"], args.get("body"))
        _regen_readme()
        return ok(f"Created {slug}.md and regenerated README.")

    if name == "e2e_rules_edit":
        slug = args["slug"]
        path = RULES_DIR / f"{slug}.md"
        if not path.exists():
            return err(f"Rule '{slug}' not found.")
        r = _read_rule(path)
        assert r is not None
        _write_rule(
            slug,
            args.get("title",     r.get("title",     slug)),
            args.get("summary",   r.get("summary",   "")),
            args.get("priority",  r.get("priority",  "SHOULD")),
            args.get("questions", r.get("questions", [])),
            args.get("body",      r.get("body")),
        )
        _regen_readme()
        return ok(f"Updated {slug}.md and regenerated README.")

    if name == "e2e_rules_remove":
        slug = args["slug"]
        path = RULES_DIR / f"{slug}.md"
        if not path.exists():
            return err(f"Rule '{slug}' not found.")
        path.unlink()
        _regen_readme()
        return ok(f"Removed {slug}.md and regenerated README.")

    if name == "e2e_rules_regen_readme":
        _regen_readme()
        return ok("README.md regenerated.")

    return err(f"Unknown tool: {name}")


# ---------------------------------------------------------------------------
# MCP stdio loop (JSON-RPC 2.0, newline-delimited)
# ---------------------------------------------------------------------------

def _send(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def main() -> None:
    for raw in sys.stdin:
        raw = raw.strip()
        if not raw:
            continue
        try:
            req = json.loads(raw)
        except json.JSONDecodeError:
            continue

        method = req.get("method", "")
        rid = req.get("id")

        if method == "initialize":
            _send({"jsonrpc": "2.0", "id": rid, "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "e2e-rules", "version": "1.0.0"},
            }})

        elif method == "notifications/initialized":
            pass  # notification — no response

        elif method == "tools/list":
            _send({"jsonrpc": "2.0", "id": rid, "result": {"tools": TOOLS}})

        elif method == "tools/call":
            params = req.get("params", {})
            result = _handle(params.get("name", ""), params.get("arguments", {}))
            _send({"jsonrpc": "2.0", "id": rid, "result": result})

        elif rid is not None:
            _send({"jsonrpc": "2.0", "id": rid, "error": {
                "code": -32601, "message": f"Method not found: {method}",
            }})


if __name__ == "__main__":
    main()
