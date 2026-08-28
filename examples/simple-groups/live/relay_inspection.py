"""Bounded, independent NIP-01 WebSocket inspection for local relay evidence.

This module deliberately knows only the NIP-01 wire envelope.  It neither
constructs nor signs events; the application under test owns that work through
the public Fava API.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import socket
import struct
import time
from dataclasses import dataclass
from typing import Any
from urllib.parse import urlsplit


MAX_FRAME_BYTES = 131_072
MAX_EVENTS = 256
MAX_EVIDENCE_BYTES = 1_048_576
HANDSHAKE_BYTES = 16_384
WEBSOCKET_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


class InspectionError(RuntimeError):
    """The independent relay observation could not reach its bounded result."""


@dataclass(frozen=True)
class Inspection:
    """One exact REQ/EOSE readback, including only bounded event evidence."""

    subscription: str
    filter: dict[str, Any]
    events: list[dict[str, Any]]

    def as_json(self) -> dict[str, Any]:
        return {
            "events": self.events,
            "filter": self.filter,
            "subscription": self.subscription,
            "terminal": "EOSE",
        }


class WebSocket:
    """Tiny local ``ws://`` client with explicit frame, event, and time bounds."""

    def __init__(self, url: str, timeout_seconds: float) -> None:
        parsed = urlsplit(url)
        if parsed.scheme != "ws" or not parsed.hostname or parsed.username or parsed.password:
            raise InspectionError("inspection accepts only unauthenticated ws:// relay URLs")
        if parsed.fragment:
            raise InspectionError("relay URL may not contain a fragment")
        self.url = url
        self.timeout_seconds = timeout_seconds
        self._socket = socket.create_connection(
            (parsed.hostname, parsed.port or 80), timeout=timeout_seconds
        )
        self._socket.settimeout(timeout_seconds)
        self._buffer = bytearray()
        self._handshake(parsed.hostname, parsed.port or 80, parsed.path or "/", parsed.query)

    def _handshake(self, host: str, port: int, path: str, query: str) -> None:
        target = path if not query else f"{path}?{query}"
        key = base64.b64encode(os.urandom(16)).decode("ascii")
        host_header = host if port == 80 else f"{host}:{port}"
        request = (
            f"GET {target} HTTP/1.1\r\n"
            f"Host: {host_header}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n"
        ).encode("ascii")
        self._socket.sendall(request)
        while b"\r\n\r\n" not in self._buffer:
            chunk = self._socket.recv(4_096)
            if not chunk:
                raise InspectionError("relay closed during WebSocket handshake")
            self._buffer.extend(chunk)
            if len(self._buffer) > HANDSHAKE_BYTES:
                raise InspectionError("relay WebSocket handshake exceeded its byte bound")
        raw_headers, remainder = bytes(self._buffer).split(b"\r\n\r\n", 1)
        self._buffer = bytearray(remainder)
        lines = raw_headers.decode("ascii", "strict").split("\r\n")
        if not lines or " 101 " not in f" {lines[0]} ":
            raise InspectionError(f"relay rejected WebSocket upgrade: {lines[0] if lines else ''}")
        headers: dict[str, str] = {}
        for line in lines[1:]:
            name, separator, value = line.partition(":")
            if not separator:
                raise InspectionError("relay sent a malformed WebSocket header")
            headers[name.strip().lower()] = value.strip()
        accepted = headers.get("sec-websocket-accept")
        expected = base64.b64encode(
            hashlib.sha1((key + WEBSOCKET_GUID).encode("ascii")).digest()
        ).decode("ascii")
        if accepted != expected:
            raise InspectionError("relay WebSocket handshake acceptance did not match")

    def _read_exact(self, count: int) -> bytes:
        while len(self._buffer) < count:
            chunk = self._socket.recv(min(16_384, count - len(self._buffer)))
            if not chunk:
                raise InspectionError("relay closed its WebSocket before the expected frame")
            self._buffer.extend(chunk)
        value = bytes(self._buffer[:count])
        del self._buffer[:count]
        return value

    def send_json(self, value: Any) -> None:
        payload = json.dumps(value, separators=(",", ":"), ensure_ascii=True).encode("utf-8")
        if len(payload) > MAX_FRAME_BYTES:
            raise InspectionError("outbound REQ exceeded the WebSocket frame bound")
        header = bytearray([0x81])
        if len(payload) < 126:
            header.append(0x80 | len(payload))
        elif len(payload) <= 65_535:
            header.extend([0x80 | 126])
            header.extend(struct.pack("!H", len(payload)))
        else:
            header.extend([0x80 | 127])
            header.extend(struct.pack("!Q", len(payload)))
        mask = os.urandom(4)
        masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
        self._socket.sendall(bytes(header) + mask + masked)

    def receive_json(self) -> Any:
        while True:
            first, second = self._read_exact(2)
            fin = bool(first & 0x80)
            opcode = first & 0x0F
            masked = bool(second & 0x80)
            length = second & 0x7F
            if not fin:
                raise InspectionError("fragmented relay frames are outside the inspection contract")
            if masked:
                raise InspectionError("relay sent an invalid masked server frame")
            if length == 126:
                length = struct.unpack("!H", self._read_exact(2))[0]
            elif length == 127:
                length = struct.unpack("!Q", self._read_exact(8))[0]
            if length > MAX_FRAME_BYTES:
                raise InspectionError("relay frame exceeded the inspection byte bound")
            payload = self._read_exact(length)
            if opcode == 0x9:
                self._send_control(0xA, payload)
                continue
            if opcode == 0x8:
                raise InspectionError("relay closed before the required EOSE")
            if opcode != 0x1:
                raise InspectionError(f"relay sent unsupported WebSocket opcode {opcode}")
            try:
                return json.loads(payload.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise InspectionError("relay sent invalid JSON") from error

    def _send_control(self, opcode: int, payload: bytes) -> None:
        if len(payload) > 125:
            raise InspectionError("relay control frame exceeded the protocol bound")
        mask = os.urandom(4)
        masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
        self._socket.sendall(bytes([0x80 | opcode, 0x80 | len(payload)]) + mask + masked)

    def close(self) -> None:
        try:
            self._send_control(0x8, b"")
        except OSError:
            pass
        finally:
            self._socket.close()


def inspect_until_eose(
    url: str,
    subscription: str,
    event_filter: dict[str, Any],
    timeout_seconds: float,
) -> Inspection:
    """Issue one bounded REQ and return only after its matching EOSE frame."""

    deadline = time.monotonic() + timeout_seconds
    socket_client = WebSocket(url, timeout_seconds)
    events: list[dict[str, Any]] = []
    evidence_bytes = 0
    try:
        socket_client.send_json(["REQ", subscription, event_filter])
        while time.monotonic() < deadline:
            remaining = max(0.001, deadline - time.monotonic())
            socket_client._socket.settimeout(remaining)
            frame = socket_client.receive_json()
            if not isinstance(frame, list) or not frame or not isinstance(frame[0], str):
                continue
            kind = frame[0]
            if kind == "EVENT" and len(frame) >= 3 and frame[1] == subscription:
                if not isinstance(frame[2], dict):
                    raise InspectionError("relay EVENT omitted an object event body")
                event_bytes = len(
                    json.dumps(frame[2], ensure_ascii=True, separators=(",", ":")).encode("utf-8")
                )
                evidence_bytes += event_bytes
                if evidence_bytes > MAX_EVIDENCE_BYTES:
                    raise InspectionError("relay event evidence exceeded the inspection byte bound")
                events.append(frame[2])
                if len(events) > MAX_EVENTS:
                    raise InspectionError("relay returned more events than the inspection bound")
            elif kind == "EOSE" and len(frame) >= 2 and frame[1] == subscription:
                return Inspection(subscription, event_filter, events)
            elif kind in {"CLOSED", "NOTICE"} and len(frame) >= 2 and frame[1] == subscription:
                raise InspectionError(f"relay ended REQ before EOSE: {frame}")
        raise InspectionError("relay did not produce EOSE before the inspection deadline")
    finally:
        socket_client.close()


def assert_event(event: dict[str, Any], expected: dict[str, Any]) -> None:
    """Require the exact event fields owned by an E2E scenario contract."""

    verify_event_id(event)
    for field in ("id", "pubkey", "kind", "content", "tags"):
        if field in expected and event.get(field) != expected[field]:
            raise InspectionError(
                f"event {event.get('id', '<unknown>')} had wrong {field}: "
                f"expected {expected[field]!r}, got {event.get(field)!r}"
            )


def verify_event_id(event: dict[str, Any]) -> None:
    """Check the NIP-01 event-id preimage without constructing an event."""

    required = ("id", "pubkey", "created_at", "kind", "tags", "content")
    if any(field not in event for field in required):
        raise InspectionError("relay event omitted a NIP-01 id field")
    if not isinstance(event["tags"], list):
        raise InspectionError("relay event tags were not an array")
    preimage = [
        0,
        event["pubkey"],
        event["created_at"],
        event["kind"],
        event["tags"],
        event["content"],
    ]
    try:
        encoded = json.dumps(preimage, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise InspectionError("relay event id preimage was not canonical JSON data") from error
    actual = hashlib.sha256(encoded).hexdigest()
    if event["id"] != actual:
        raise InspectionError("relay event id did not match its NIP-01 preimage")
