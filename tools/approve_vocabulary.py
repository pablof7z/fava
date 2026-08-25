#!/usr/bin/env python3
"""Serve the local vocabulary approval app on 127.0.0.1."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tomllib
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

import vocabulary_approval as approval
import vocabulary_structure as structure

APP_HTML = Path(__file__).with_name("approve_vocabulary.html")
MAXIMUM_BODY_BYTES = 256 * 1024
SIGNING_PAUSED = True


def _find_verifier(root: Path) -> Path | None:
    """Locate the vocab-verify binary: cargo debug build, then PATH."""
    debug = root / "target/debug/vocab-verify"
    if debug.exists():
        return debug
    on_path = shutil.which("vocab-verify")
    return Path(on_path) if on_path else None


def read_terms(root: Path) -> list[dict[str, Any]]:
    """Load every registry term in file order."""
    registry = tomllib.loads(
        (root / "docs/internals/vocabulary.toml").read_text(encoding="utf-8")
    )
    return list(registry.get("term", []))


def read_candidates(
    root: Path, terms: list[dict[str, Any]]
) -> tuple[list[dict[str, Any]], list[str]]:
    research, problems = approval.load_candidate_research(
        root / approval.CANDIDATES_PATH
    )
    candidates, candidate_problems = approval.candidate_terms(terms, research, root)
    return candidates, [*problems, *candidate_problems]


class Handler(BaseHTTPRequestHandler):
    root: Path
    verifier_path: Path | None = None  # injected by --verifier or main()

    def log_message(self, format: str, *args: Any) -> None:
        return

    def _send(self, status: int, body: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_json(self, status: int, payload: Any) -> None:
        self._send(status, json.dumps(payload).encode("utf-8"), "application/json")

    def do_GET(self) -> None:
        path = urlsplit(self.path).path
        if path in {"/", "/index.html"}:
            self._send(200, APP_HTML.read_bytes(), "text/html; charset=utf-8")
        elif path == "/api/terms":
            self._send_json(200, self._terms_payload())
        else:
            self._send_json(404, {"error": "not found"})

    def do_POST(self) -> None:
        if self.path != "/api/approvals":
            self._send_json(404, {"error": "not found"})
            return
        if SIGNING_PAUSED:
            self._send_json(
                423,
                {
                    "error": (
                        "vocabulary signing is paused until the human-first "
                        "review contract receives independent acceptance"
                    )
                },
            )
            return
        length = int(self.headers.get("Content-Length") or 0)
        if length <= 0 or length > MAXIMUM_BODY_BYTES:
            self._send_json(413, {"error": "approval body is out of bounds"})
            return
        try:
            event = json.loads(self.rfile.read(length))
        except json.JSONDecodeError as error:
            self._send_json(400, {"error": f"unreadable event: {error}"})
            return

        problems = approval.verify_event(event)
        if problems:
            self._send_json(400, {"error": "; ".join(problems)})
            return

        structure_path = self.root / approval.STRUCTURE_PATH
        if not structure.snapshot_inputs_current(self.root, structure_path):
            self._send_json(
                409,
                {
                    "error": (
                        "Rust inputs changed after structural compilation; "
                        "refresh the snapshot and restart approval"
                    )
                },
            )
            return

        name = approval.approved_name(event)
        term_list = read_terms(self.root)
        candidates, candidate_problems = read_candidates(self.root, term_list)
        if candidate_problems:
            self._send_json(409, {"error": "; ".join(candidate_problems)})
            return
        terms = {term["name"]: term for term in [*term_list, *candidates]}
        term = terms.get(name)
        if term is None:
            self._send_json(400, {"error": f"no registry term named {name}"})
            return
        if term.get("disposition") == "blocked":
            self._send_json(
                409,
                {"error": f"{name}: candidate is blocked and cannot be signed"},
            )
            return
        hidden = approval.hidden_vocabulary(term_list)
        concealed = (
            approval.structural_problems_for_term(term, hidden)
            if term in term_list
            else []
        )
        if concealed:
            self._send_json(
                409,
                {
                    "error": (
                        f"{name}: cannot approve a term hiding differently named "
                        f"concepts: {', '.join(concealed)}"
                    )
                },
            )
            return
        structures, structure_problems = structure.read_snapshot(structure_path)
        if structure_problems:
            self._send_json(409, {"error": "; ".join(structure_problems)})
            return
        packet = structures.get(name)
        if packet is None:
            self._send_json(
                409, {"error": f"{name}: missing compiler-derived structure"}
            )
            return
        if packet["review_problems"]:
            self._send_json(
                409,
                {"error": f"{name}: " + "; ".join(packet["review_problems"])},
            )
            return
        try:
            expected = approval.canonical_markdown(term, packet)
        except ValueError as error:
            self._send_json(500, {"error": f"canonical_markdown error: {error}"})
            return
        if event["content"] != expected:
            self._send_json(
                400, {"error": f"{name}: signed text is not the term's current text"}
            )
            return

        # Cryptographic verification via the Rust binary.
        verifier = self.__class__.verifier_path or _find_verifier(self.root)
        if verifier is None:
            self._send_json(
                500,
                {"error": "vocab-verify binary not found; run 'cargo build -p fava --bin vocab-verify'"},
            )
            return
        result = subprocess.run(
            [str(verifier)],
            input=json.dumps(event).encode("utf-8"),
            capture_output=True,
            timeout=10,
        )
        if result.returncode != 0:
            self._send_json(
                400,
                {"error": result.stderr.decode("utf-8", errors="replace").strip()},
            )
            return

        path = self.root / approval.APPROVALS_PATH
        path.parent.mkdir(parents=True, exist_ok=True)

        # Preserve signed history. Exact event replays are idempotent; a new
        # signature for changed final markdown appends beside the old event.
        event_id = event.get("id", "")
        existing_approvals, _ = approval.load_approvals(path)
        if any(event.get("id") == event_id for event in existing_approvals.get(name, [])):
            self._send_json(200, {"stored": name, "note": "already stored"})
            return

        with path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(event, sort_keys=True) + "\n")
        self._send_json(200, {"stored": name})

    def _terms_payload(self) -> dict[str, Any]:
        terms = read_terms(self.root)
        hidden = approval.hidden_vocabulary(terms)
        candidates, candidate_problems = read_candidates(self.root, terms)
        approvals, approval_problems = approval.load_approvals(
            self.root / approval.APPROVALS_PATH
        )
        structures, structure_problems = structure.read_snapshot(
            self.root / approval.STRUCTURE_PATH
        )
        problems = [
            *candidate_problems,
            *approval_problems,
            *structure_problems,
        ]
        payload = []
        candidate_names = {term["name"] for term in candidates}
        for term in terms:
            if term["name"] in candidate_names:
                continue
            packet = structures.get(term["name"])
            markdown = (
                approval.canonical_markdown(term, packet)
                if packet is not None
                else ""
            )
            signatures = approvals.get(term["name"], [])
            signed = approval.authoritative_approval(signatures, markdown)
            concealed = approval.structural_problems_for_term(term, hidden)
            if packet is None:
                status = "invalid"
                concealed = ["missing compiler-derived structure", *concealed]
            elif packet["review_problems"]:
                status = "invalid"
                concealed = [*packet["review_problems"], *concealed]
            elif concealed:
                status = "invalid"
            elif signed is not None:
                status = "approved"
            elif not signatures:
                status = "unapproved"
            else:
                status = "stale"
            latest = signatures[-1] if signatures else None
            payload.append(
                {
                    "name": term["name"],
                    "source": term.get("source", ""),
                    "owner": term.get("owner", ""),
                    "markdown": markdown,
                    "rust_item": approval.symbol_for_term(term),
                    "rust_item_kind": approval.item_kind_for_term(term, root=self.root),
                    "purpose": approval.row_purpose(term),
                    "status": status,
                    "signed_at": signed["created_at"] if signed else None,
                    "signed_content": latest["content"] if latest else None,
                    "structural_problems": concealed,
                    "missing_term": False,
                    "candidate": False,
                }
            )

        for term in candidates:
            packet = structures.get(term["name"])
            markdown = (
                approval.canonical_markdown(term, packet)
                if packet is not None
                else ""
            )
            signatures = approvals.get(term["name"], [])
            blocked = term["disposition"] == "blocked"
            signed = None if blocked else approval.authoritative_approval(signatures, markdown)
            status = (
                "invalid"
                if packet is None or packet["review_problems"]
                else "blocked"
                if blocked
                else "approved"
                if signed
                else "stale"
                if signatures
                else "unapproved"
            )
            latest = signatures[-1] if signatures else None
            payload.append(
                {
                    "name": term["name"],
                    "source": term["source"],
                    "owner": term["owner"],
                    "markdown": markdown,
                    "rust_item": approval.symbol_for_term(term),
                    "rust_item_kind": approval.item_kind_for_term(term, root=self.root),
                    "purpose": approval.row_purpose(term),
                    "status": status,
                    "signed_at": signed["created_at"] if signed else None,
                    "signed_content": latest["content"] if latest else None,
                    "structural_problems": [],
                    "missing_term": False,
                    "candidate": True,
                    "disposition": term["disposition"],
                    "proposed_disposition": term["proposed_disposition"],
                }
            )
        return {
            "owner": approval.OWNER,
            "terms": payload,
            "problems": problems,
            "hidden_occurrences": sum(len(items) for items in hidden.values()),
            "hidden_names": len(hidden),
        }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".")
    parser.add_argument("--port", type=int, default=4319)
    parser.add_argument("--no-open", action="store_true")
    parser.add_argument(
        "--dump-candidates-json",
        action="store_true",
        help="print every validated candidate's exact review markdown and exit",
    )
    parser.add_argument(
        "--verifier",
        default=None,
        metavar="PATH",
        help="path to the vocab-verify binary (overrides auto-discovery)",
    )
    arguments = parser.parse_args()

    Handler.root = Path(arguments.root).resolve()
    if arguments.dump_candidates_json:
        terms = read_terms(Handler.root)
        candidates, problems = read_candidates(Handler.root, terms)
        structures, structure_problems = structure.read_snapshot(
            Handler.root / approval.STRUCTURE_PATH
        )
        problems.extend(structure_problems)
        problems.extend(
            f"{term['name']}: missing compiler-derived structure"
            for term in candidates
            if term["name"] not in structures
        )
        if problems:
            for problem in problems:
                print(problem, file=sys.stderr)
            return 1
        print(
            json.dumps(
                [
                    {
                        "name": term["name"],
                        "disposition": term["disposition"],
                        "markdown": approval.canonical_markdown(
                            term, structures[term["name"]]
                        ),
                    }
                    for term in candidates
                ],
                sort_keys=True,
            )
        )
        return 0
    expected_snapshot = structure.render_snapshot(
        structure.compile_snapshot(Handler.root)
    )
    snapshot_path = Handler.root / approval.STRUCTURE_PATH
    actual_snapshot = (
        snapshot_path.read_text(encoding="utf-8")
        if snapshot_path.exists()
        else ""
    )
    if actual_snapshot != expected_snapshot:
        print(
            "approval refused: compiler-derived vocabulary structure is stale; "
            "run python3 tools/vocabulary_structure.py update",
            file=sys.stderr,
        )
        return 1
    Handler.verifier_path = Path(arguments.verifier) if arguments.verifier else None
    server = ThreadingHTTPServer(("127.0.0.1", arguments.port), Handler)
    url = f"http://127.0.0.1:{arguments.port}/"
    print(f"vocabulary approval app: {url}")
    if not arguments.no_open:
        webbrowser.open(url)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        return 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
