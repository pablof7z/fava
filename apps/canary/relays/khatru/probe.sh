#!/usr/bin/env bash
set -euo pipefail

readonly readiness_seconds=10
readonly stop_seconds=5
readonly max_document_bytes=65536
readonly max_subscriptions=3
readonly max_message_length=4096
readonly max_limit=17
readonly max_subid_length=64

probe_tmp="$(mktemp -d /tmp/fava-khatru-probe.XXXXXX)"
relay_pid=""
watchdog_pid=""

print_logs() {
  if [[ -s "$probe_tmp/relay.stdout" ]]; then
    printf '%s\n' '--- relay stdout ---' >&2
    sed -n '1,120p' "$probe_tmp/relay.stdout" >&2
  fi
  if [[ -s "$probe_tmp/relay.stderr" ]]; then
    printf '%s\n' '--- relay stderr ---' >&2
    sed -n '1,120p' "$probe_tmp/relay.stderr" >&2
  fi
}

cleanup() {
  if [[ -n "$watchdog_pid" ]] && kill -0 "$watchdog_pid" 2>/dev/null; then
    kill "$watchdog_pid" 2>/dev/null || true
    wait "$watchdog_pid" 2>/dev/null || true
  fi
  if [[ -n "$relay_pid" ]] && kill -0 "$relay_pid" 2>/dev/null; then
    kill -TERM "$relay_pid" 2>/dev/null || true
    sleep 0.1
    kill -KILL "$relay_pid" 2>/dev/null || true
    wait "$relay_pid" 2>/dev/null || true
  fi
  case "$probe_tmp" in
    /tmp/fava-khatru-probe.*) rm -rf -- "$probe_tmp" ;;
    *) printf 'refusing to remove unexpected probe path: %s\n' "$probe_tmp" >&2 ;;
  esac
}

fail() {
  printf 'GO25_KHATRU_READINESS: FAIL %s\n' "$1" >&2
  print_logs
  exit 1
}

trap cleanup EXIT
trap 'exit 130' INT TERM HUP

toolchain="$(GOTOOLCHAIN=local go version)"
case "$toolchain" in
  *' go1.25.'*) ;;
  *) fail "GOTOOLCHAIN=local did not select Go 1.25.x: $toolchain" ;;
esac

if ! awk '$1 == "go" && $2 == "1.25.0" { found = 1 } END { exit !found }' go.mod; then
  fail 'go.mod is not pinned at go 1.25.0'
fi

module_before="$(shasum -a 256 go.mod go.sum)"
GOTOOLCHAIN=local go mod verify
GOTOOLCHAIN=local go test ./...
GOTOOLCHAIN=local go build -o "$probe_tmp/khatru-relay" ./...
module_after="$(shasum -a 256 go.mod go.sum)"
if [[ "$module_before" != "$module_after" ]]; then
  fail 'go.mod or go.sum changed during verification/build'
fi

port="$(python3 -c 'import socket; sock = socket.socket(); sock.bind(("127.0.0.1", 0)); print(sock.getsockname()[1]); sock.close()')"
printf '%s\n' "$port" > "$probe_tmp/port"

"$probe_tmp/khatru-relay" \
  --port "$port" \
  --name fava-go25-readiness \
  --max-subscriptions "$max_subscriptions" \
  --max-message-length "$max_message_length" \
  --max-limit "$max_limit" \
  --max-subid-length "$max_subid_length" \
  > "$probe_tmp/relay.stdout" \
  2> "$probe_tmp/relay.stderr" &
relay_pid=$!
printf '%s\n' "$relay_pid" > "$probe_tmp/pid"

if ! readiness_record="$(python3 - \
  "$port" \
  "$relay_pid" \
  "$readiness_seconds" \
  "$max_document_bytes" \
  "$max_subscriptions" \
  "$max_message_length" \
  "$max_limit" \
  "$max_subid_length" \
  "$probe_tmp/nip11.json" \
  "$probe_tmp/nip11.headers" <<'PY'
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request

port = int(sys.argv[1])
pid = int(sys.argv[2])
deadline = time.monotonic() + int(sys.argv[3])
capacity = int(sys.argv[4])
expected = {
    "max_subscriptions": int(sys.argv[5]),
    "max_message_length": int(sys.argv[6]),
    "max_limit": int(sys.argv[7]),
    "max_subid_length": int(sys.argv[8]),
}
document_path = sys.argv[9]
headers_path = sys.argv[10]
started = time.monotonic()
last_error = "no response"

while True:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        raise SystemExit("relay exited before readiness")

    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise SystemExit(f"readiness timeout: {last_error}")

    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/",
        headers={"Accept": "application/nostr+json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=min(0.5, remaining)) as response:
            content_type = response.headers.get_content_type()
            body = response.read(capacity + 1)
            if len(body) > capacity:
                raise SystemExit(f"NIP-11 document exceeds {capacity} bytes")
            if content_type != "application/nostr+json":
                raise SystemExit(f"invalid NIP-11 content type: {content_type}")
            try:
                document = json.loads(body)
            except json.JSONDecodeError as error:
                raise SystemExit(f"invalid NIP-11 JSON: {error}") from error
            limitation = document.get("limitation")
            if not isinstance(limitation, dict):
                raise SystemExit("invalid NIP-11 document: limitation object missing")
            for key, value in expected.items():
                if limitation.get(key) != value:
                    raise SystemExit(
                        f"invalid NIP-11 {key}: expected {value}, got {limitation.get(key)!r}"
                    )
            with open(document_path, "wb") as output:
                output.write(body)
            with open(headers_path, "w", encoding="utf-8") as output:
                for key, value in response.headers.items():
                    output.write(f"{key}: {value}\n")
            elapsed_ms = int((time.monotonic() - started) * 1000)
            digest = hashlib.sha256(body).hexdigest()
            compact = json.dumps(document, sort_keys=True, separators=(",", ":"))
            print(
                f"ready_ms={elapsed_ms} content_type={content_type} "
                f"bytes={len(body)} sha256={digest} document={compact}"
            )
            break
    except (urllib.error.URLError, TimeoutError, ConnectionError) as error:
        last_error = str(error)
        time.sleep(min(0.05, max(0.0, deadline - time.monotonic())))
PY
)"; then
  fail 'relay failed bounded readiness or NIP-11 validation'
fi

stop_started_ns="$(python3 -c 'import time; print(time.monotonic_ns())')"
if ! kill -TERM "$relay_pid"; then
  fail "failed to terminate relay pid $relay_pid"
fi

(
  sleep "$stop_seconds"
  printf 'timeout\n' > "$probe_tmp/stop-timeout"
  kill -KILL "$relay_pid" 2>/dev/null || true
) &
watchdog_pid=$!

set +e
wait "$relay_pid"
relay_wait_status=$?
set -e
relay_pid=""

if kill -0 "$watchdog_pid" 2>/dev/null; then
  kill "$watchdog_pid" 2>/dev/null || true
fi
wait "$watchdog_pid" 2>/dev/null || true
watchdog_pid=""

if [[ -f "$probe_tmp/stop-timeout" ]]; then
  fail "relay survived the ${stop_seconds}s termination/reap ceiling"
fi

stop_finished_ns="$(python3 -c 'import time; print(time.monotonic_ns())')"
stop_ms=$(((stop_finished_ns - stop_started_ns) / 1000000))
if ((stop_ms > stop_seconds * 1000)); then
  fail "relay wait exceeded ${stop_seconds}s: ${stop_ms}ms"
fi

recorded_pid="$(<"$probe_tmp/pid")"
if kill -0 "$recorded_pid" 2>/dev/null; then
  fail "relay pid $recorded_pid survived wait"
fi

stdout_sha256="$(shasum -a 256 "$probe_tmp/relay.stdout" | awk '{print $1}')"
printf 'GO25_KHATRU_TOOLCHAIN: %s\n' "$toolchain"
printf 'GO25_KHATRU_MODULE: PASS go_mod=1.25.0 checksums=verified tests=passed build=passed\n'
printf 'GO25_KHATRU_PROCESS: pid=%s port=%s stdout_sha256=%s\n' \
  "$recorded_pid" "$port" "$stdout_sha256"
printf 'GO25_KHATRU_NIP11: %s\n' "$readiness_record"
printf 'GO25_KHATRU_TEARDOWN: term=sent wait_status=%s wait_ms=%s reaped=true\n' \
  "$relay_wait_status" "$stop_ms"
printf 'GO25_KHATRU_READINESS: PASS pid=%s port=%s readiness_limit_s=%s stop_limit_s=%s\n' \
  "$recorded_pid" "$port" "$readiness_seconds" "$stop_seconds"
