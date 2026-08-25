#!/usr/bin/env python3
"""Serve the local vocabulary approval app on 127.0.0.1."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import tomllib
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

import vocabulary_approval as approval

APP_HTML = Path(__file__).with_name("approve_vocabulary.html")
MAXIMUM_BODY_BYTES = 256 * 1024


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
        if self.path in {"/", "/index.html"}:
            self._send(200, APP_HTML.read_bytes(), "text/html; charset=utf-8")
        elif self.path == "/api/terms":
            self._send_json(200, self._terms_payload())
        else:
            self._send_json(404, {"error": "not found"})

    def do_POST(self) -> None:
        if self.path != "/api/approvals":
            self._send_json(404, {"error": "not found"})
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

        name = approval.approved_name(event)
        terms = {term["name"]: term for term in read_terms(self.root)}
        term = terms.get(name)
        if term is None:
            self._send_json(400, {"error": f"no registry term named {name}"})
            return
        try:
            expected = approval.canonical_markdown(term)
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

        # Deduplication: reject if this id is already stored, or if the term
        # already has a different approval (competing signatures).
        event_id = event.get("id", "")
        existing_approvals, _ = approval.load_approvals(path)
        existing = existing_approvals.get(name)
        if existing is not None:
            if existing.get("id") == event_id:
                self._send_json(200, {"stored": name, "note": "already stored"})
                return
            self._send_json(
                409,
                {
                    "error": (
                        f"{name}: already has a different approval "
                        "(edit approvals.jsonl to replace it)"
                    )
                },
            )
            return

        with path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(event, sort_keys=True) + "\n")
        self._send_json(200, {"stored": name})

    def _terms_payload(self) -> dict[str, Any]:
        terms = read_terms(self.root)
        approvals, problems = approval.load_approvals(
            self.root / approval.APPROVALS_PATH
        )
        payload = []
        for term in terms:
            markdown = approval.canonical_markdown(term)
            signed = approvals.get(term["name"])
            if signed is None:
                status = "unapproved"
            elif signed["content"] == markdown:
                status = "approved"
            else:
                status = "stale"
            payload.append(
                {
                    "name": term["name"],
                    "source": term.get("source", ""),
                    "owner": term.get("owner", ""),
                    "markdown": markdown,
                    "status": status,
                    "signed_at": signed["created_at"] if signed else None,
                    "signed_content": signed["content"] if signed else None,
                }
            )
        return {"owner": approval.OWNER, "terms": payload, "problems": problems}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".")
    parser.add_argument("--port", type=int, default=4319)
    parser.add_argument("--no-open", action="store_true")
    parser.add_argument(
        "--verifier",
        default=None,
        metavar="PATH",
        help="path to the vocab-verify binary (overrides auto-discovery)",
    )
    arguments = parser.parse_args()

    Handler.root = Path(arguments.root).resolve()
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
