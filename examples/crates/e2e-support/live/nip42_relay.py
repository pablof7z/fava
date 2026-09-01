#!/usr/bin/env python3
"""A small, bounded, harness-owned NIP-42-enforcing relay.

Real third-party relays give a genuine NIP-42 *challenge*, but the one this
repository can pin (`nostr-rs-relay`) does not enforce anything once
challenged: an unauthenticated `REQ` or `EVENT` still succeeds. Proving what
Fava does when a relay actually *demands* authentication -- refuses reads and
writes until a valid response arrives, and can accept, reject, or accept the
proof while still refusing the request -- needs a relay this harness fully
controls. This module is that relay.

It speaks only the exact NIP-01/NIP-42 wire subset the `relay-auth` E2E
scenario needs: `EVENT`, `REQ` (matched by `ids`, `authors`, `kinds`, and
`limit` only), `CLOSE`, `AUTH`, and the server frames `EVENT`, `EOSE`, `OK`,
`CLOSED`, and one unsolicited `AUTH` challenge per connection.

Every kind-22242 `AUTH` event's id and signature are verified for real, by
shelling out to the `nip01_wire` Rust binary (`verify` subcommand) built from
this same crate -- Python has no bundled BIP-340 schnorr implementation, and
this relay must not pretend to check what it cannot check.

Modes (selected once, at process start, with `--mode`):

* ``accept``        -- a valid `AUTH` is answered `OK true`; the connection is
  then authenticated and ordinary reads/writes succeed.
* ``reject``         -- a valid `AUTH` is answered `OK false "error: ..."`
  (not `restricted:`); the connection is never authenticated.
* ``accept-refuse``  -- a valid `AUTH` is answered `OK false "restricted: ..."`;
  the connection is never authenticated either, so reads/writes keep failing
  with `auth-required:`. This is the wire shape Fava classifies as
  `AcceptedButStillRefused` (see `crates/fava-auth/src/authenticator/answer.rs`):
  the proof itself was not rejected outright, but the account remains refused.

A malformed or unverifiable `AUTH` (bad signature, wrong challenge, wrong
kind) is always answered `OK false "invalid: <reason>"`, regardless of mode.

Every connection is challenged exactly once, with a fresh random nonce; a new
connection gets a new challenge.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import hashlib
import json
import time
import uuid
from dataclasses import dataclass, field
from typing import Any

WEBSOCKET_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
MAX_FRAME_BYTES = 131_072
MAX_STORED_EVENTS = 1_000
MAX_CONNECTIONS = 16
CLOCK_SKEW_SECONDS = 600
VERIFY_TIMEOUT_SECONDS = 5.0


@dataclass
class Store:
    """Bounded in-memory event store shared by every connection."""

    events: list[dict[str, Any]] = field(default_factory=list)

    def accept(self, event: dict[str, Any]) -> bool:
        if len(self.events) >= MAX_STORED_EVENTS:
            return False
        self.events.append(event)
        return True

    def match(self, event_filter: dict[str, Any]) -> list[dict[str, Any]]:
        ids = event_filter.get("ids")
        authors = event_filter.get("authors")
        kinds = event_filter.get("kinds")
        limit = event_filter.get("limit")
        matched = [
            event
            for event in self.events
            if (ids is None or event["id"] in ids)
            and (authors is None or event["pubkey"] in authors)
            and (kinds is None or event["kind"] in kinds)
        ]
        if isinstance(limit, int) and limit >= 0:
            matched = matched[-limit:] if limit else []
        return matched


class RelayError(RuntimeError):
    """A frame this relay refuses to process further."""


async def verify_event(verify_bin: str, event: dict[str, Any]) -> tuple[bool, str]:
    """Verify one event's exact NIP-01 id/signature through the Rust helper."""

    process = await asyncio.create_subprocess_exec(
        verify_bin,
        "verify",
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    payload = json.dumps(event).encode("utf-8")
    try:
        stdout, stderr = await asyncio.wait_for(
            process.communicate(payload), VERIFY_TIMEOUT_SECONDS
        )
    except TimeoutError:
        process.kill()
        return False, "verifier timed out"
    if process.returncode == 0:
        return True, stdout.decode("utf-8", "replace").strip()
    return False, stderr.decode("utf-8", "replace").strip() or "verification failed"


class Connection:
    def __init__(
        self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter,
        store: Store, mode: str, verify_bin: str, relay_url: str,
    ) -> None:
        self.reader = reader
        self.writer = writer
        self.store = store
        self.mode = mode
        self.verify_bin = verify_bin
        self.relay_url = relay_url
        self.challenge = str(uuid.uuid4())
        self.authenticated = False
        self.buffer = bytearray()

    async def handshake(self) -> bool:
        request = b""
        while b"\r\n\r\n" not in request:
            chunk = await self.reader.read(4_096)
            if not chunk:
                return False
            request += chunk
            if len(request) > 16_384:
                return False
        head, _, rest = request.partition(b"\r\n\r\n")
        self.buffer.extend(rest)
        lines = head.decode("ascii", "replace").split("\r\n")
        headers = {}
        for line in lines[1:]:
            name, separator, value = line.partition(":")
            if separator:
                headers[name.strip().lower()] = value.strip()
        key = headers.get("sec-websocket-key")
        if not key:
            return False
        accept = base64.b64encode(
            hashlib.sha1((key + WEBSOCKET_GUID).encode("ascii")).digest()
        ).decode("ascii")
        response = (
            "HTTP/1.1 101 Switching Protocols\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Accept: {accept}\r\n\r\n"
        )
        self.writer.write(response.encode("ascii"))
        await self.writer.drain()
        return True

    async def send_json(self, value: Any) -> None:
        payload = json.dumps(value, separators=(",", ":")).encode("utf-8")
        if len(payload) > MAX_FRAME_BYTES:
            raise RelayError("outbound frame exceeded the relay's own bound")
        header = bytearray([0x81])
        if len(payload) < 126:
            header.append(len(payload))
        elif len(payload) <= 65_535:
            header.append(126)
            header.extend(len(payload).to_bytes(2, "big"))
        else:
            header.append(127)
            header.extend(len(payload).to_bytes(8, "big"))
        self.writer.write(bytes(header) + payload)
        await self.writer.drain()

    async def recv_json(self) -> Any | None:
        while True:
            while len(self.buffer) < 2:
                chunk = await self.reader.read(4_096)
                if not chunk:
                    return None
                self.buffer.extend(chunk)
            first, second = self.buffer[0], self.buffer[1]
            opcode = first & 0x0F
            masked = bool(second & 0x80)
            length = second & 0x7F
            header_len = 2
            if length == 126:
                header_len += 2
            elif length == 127:
                header_len += 8
            if masked:
                header_len += 4
            while len(self.buffer) < header_len:
                chunk = await self.reader.read(4_096)
                if not chunk:
                    return None
                self.buffer.extend(chunk)
            offset = 2
            if length == 126:
                length = int.from_bytes(self.buffer[2:4], "big")
                offset = 4
            elif length == 127:
                length = int.from_bytes(self.buffer[2:10], "big")
                offset = 10
            if length > MAX_FRAME_BYTES:
                raise RelayError("inbound frame exceeded the relay's bound")
            mask = bytes(self.buffer[offset : offset + 4]) if masked else b""
            offset += 4 if masked else 0
            total = offset + length
            while len(self.buffer) < total:
                chunk = await self.reader.read(4_096)
                if not chunk:
                    return None
                self.buffer.extend(chunk)
            payload = bytes(self.buffer[offset:total])
            del self.buffer[:total]
            if masked:
                payload = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
            if opcode == 0x8:
                return None
            if opcode == 0x9:
                await self._pong(payload)
                continue
            if opcode == 0xA:
                continue
            if opcode != 0x1:
                continue
            try:
                return json.loads(payload.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError):
                continue

    async def _pong(self, payload: bytes) -> None:
        header = bytes([0x8A, len(payload)])
        self.writer.write(header + payload)
        await self.writer.drain()

    async def run(self) -> None:
        if not await self.handshake():
            self.writer.close()
            return
        await self.send_json(["AUTH", self.challenge])
        try:
            while True:
                frame = await self.recv_json()
                if frame is None:
                    return
                if not isinstance(frame, list) or not frame or not isinstance(frame[0], str):
                    continue
                await self._dispatch(frame)
        except RelayError as error:
            await self.send_json(["NOTICE", str(error)])
        finally:
            self.writer.close()

    async def _dispatch(self, frame: list[Any]) -> None:
        kind = frame[0]
        if kind == "EVENT" and len(frame) >= 2 and isinstance(frame[1], dict):
            await self._on_event(frame[1])
        elif kind == "REQ" and len(frame) >= 2 and isinstance(frame[1], str):
            await self._on_req(frame[1], frame[2] if len(frame) > 2 and isinstance(frame[2], dict) else {})
        elif kind == "CLOSE":
            return
        elif kind == "AUTH" and len(frame) >= 2 and isinstance(frame[1], dict):
            await self._on_auth(frame[1])

    async def _on_event(self, event: dict[str, Any]) -> None:
        event_id = str(event.get("id", "0" * 64))
        if not self.authenticated:
            await self.send_json(
                ["OK", event_id, False, "auth-required: this relay requires authentication for writes"]
            )
            return
        valid, detail = await verify_event(self.verify_bin, event)
        if not valid:
            await self.send_json(["OK", event_id, False, f"invalid: {detail}"])
            return
        if not self.store.accept(event):
            await self.send_json(["OK", event_id, False, "error: relay storage bound reached"])
            return
        await self.send_json(["OK", event_id, True, ""])

    async def _on_req(self, subscription: str, event_filter: dict[str, Any]) -> None:
        if not self.authenticated:
            await self.send_json(
                ["CLOSED", subscription, "auth-required: this relay requires authentication for reads"]
            )
            return
        for event in self.store.match(event_filter):
            await self.send_json(["EVENT", subscription, event])
        await self.send_json(["EOSE", subscription])

    async def _on_auth(self, event: dict[str, Any]) -> None:
        event_id = str(event.get("id", "0" * 64))
        reason = self._auth_defect(event)
        if reason is not None:
            await self.send_json(["OK", event_id, False, f"invalid: {reason}"])
            return
        valid, detail = await verify_event(self.verify_bin, event)
        if not valid:
            await self.send_json(["OK", event_id, False, f"invalid: {detail}"])
            return
        if self.mode == "accept":
            self.authenticated = True
            await self.send_json(["OK", event_id, True, "authenticated"])
        elif self.mode == "reject":
            await self.send_json(["OK", event_id, False, "error: authentication rejected by policy"])
        elif self.mode == "accept-refuse":
            await self.send_json(["OK", event_id, False, "restricted: pubkey not permitted by policy"])
        else:
            raise RelayError(f"unknown relay mode {self.mode!r}")

    def _auth_defect(self, event: dict[str, Any]) -> str | None:
        if event.get("kind") != 22242:
            return "AUTH event must be kind 22242"
        tags = event.get("tags")
        if not isinstance(tags, list):
            return "AUTH event tags must be a list"
        challenge_values = [tag[1] for tag in tags if isinstance(tag, list) and len(tag) >= 2 and tag[0] == "challenge"]
        if challenge_values != [self.challenge]:
            return "challenge tag does not match the exact challenge this connection was given"
        relay_values = [tag[1] for tag in tags if isinstance(tag, list) and len(tag) >= 2 and tag[0] == "relay"]
        if not relay_values or not relay_values[0]:
            return "AUTH event carries no relay tag"
        created_at = event.get("created_at")
        if not isinstance(created_at, int) or abs(created_at - int(time.time())) > CLOCK_SKEW_SECONDS:
            return "AUTH event created_at is outside the accepted clock skew"
        return None


async def serve(host: str, port: int, mode: str, verify_bin: str) -> None:
    store = Store()
    relay_url = f"ws://{host}:{port}"
    active = 0

    async def handle(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        nonlocal active
        if active >= MAX_CONNECTIONS:
            writer.close()
            return
        active += 1
        try:
            connection = Connection(reader, writer, store, mode, verify_bin, relay_url)
            await connection.run()
        finally:
            active -= 1

    server = await asyncio.start_server(handle, host, port)
    print(f"ready {relay_url}", flush=True)
    async with server:
        await server.serve_forever()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--mode", choices=["accept", "reject", "accept-refuse"], required=True)
    parser.add_argument("--verify-bin", required=True, help="path to the built nip01_wire binary")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        asyncio.run(serve(args.host, args.port, args.mode, args.verify_bin))
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
