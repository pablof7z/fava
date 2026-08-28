# Simple-groups live proof harness

This private harness is a relay lab for the experiential `simple-groups` REPL.
It owns only disposable process lifecycle, bounded application invocation and
artifacts, and an independent NIP-01 WebSocket `REQ`/`EOSE` reader. It never
constructs, signs, routes, or publishes an event. The REPL command files do
that through its public Fava assembly.

`discover` reports candidates without selecting one or claiming a live proof:

```sh
python3 examples/simple-groups/live/harness.py discover
```

Run the executable smoke scenario only with explicit real binaries and a fresh
artifact directory. It starts Croissant as the NIP-29 authorization relay and
`nostr-rs-relay` as the ordinary kind-10009 state relay. Both bind an ephemeral
loopback port, must complete their own bounded `REQ`/`EOSE` readiness read, and
are stopped as process groups on every exit path. The ordinary relay is pinned
to `nostr-rs-relay 0.8.12`; the harness refuses an unrecognised version before
it creates any relay data.

```sh
artifacts="$(mktemp -d examples/simple-groups/live/runs/smoke.XXXXXX)"
rmdir "$artifacts"
python3 examples/simple-groups/live/harness.py run \
  --nip29-bin /Users/pablofernandez/Work/croissant/croissant \
  --ordinary-bin "$(command -v nostr-rs-relay)" \
  --artifacts "$artifacts"
```

The smoke command file proves exact create and arbitrary-kind content events:
their IDs and authors come from typed REPL JSONL results, while the harness
independently reads the group relay and requires exact `id`, `pubkey`, `kind`,
`content`, and whole `tags` arrays before the matching `EOSE`. It does not use
the REPL's cache, receipts, or output as readback proof.

Executable scenarios require at least one explicit direct-relay assertion.
Positive assertions name exact `id`, `pubkey`, `kind`, `content`, and whole
`tags`; negative assertions have a nonempty filter and require no match. A
scenario carrying future `required_facts` is blocked by construction: merely
changing its status to `executable` is refused before artifacts or relays exist.

Artifacts are bounded: process stdout/stderr are capped at 1 MiB, command and
scenario inputs at 64 KiB, app runtime at 60 seconds, readiness and each REQ at
10 seconds, every REQ/EOSE read at 256 events and 1 MiB, and retained scanning
at 256 entries, 128 files, 2 MiB per file, and 40 MiB total. Rendered JSONL
uses sorted keys. Relay databases, relay logs, materialized commands, and child
`TMPDIR` live only in ignored scratch state. Scratch is removed before retained
artifacts are scanned on every exit path. `secret-nondisclosure` independently
requires no matching kind-12345 relay event and scans every retained artifact
for its sentinel.

```sh
artifacts="$(mktemp -d examples/simple-groups/live/runs/secret.XXXXXX)"
rmdir "$artifacts"
python3 examples/simple-groups/live/harness.py run \
  --scenario secret-nondisclosure \
  --nip29-bin /Users/pablofernandez/Work/croissant/croissant \
  --ordinary-bin "$(command -v nostr-rs-relay)" \
  --artifacts "$artifacts"
```

`scenarios/full-nip29-contract.json` is deliberately blocked, rather than a
fake proof. It records the exact future evidence required for metadata
configuration, member admission and rejection, relay-authored state,
kind-10009 saved lists on the ordinary relay, event/group deletion, and all
post-deletion absence checks. The current REPL has no commands for those
operations. When it does, add its ordinary command file under `commands/` and
replace `required_facts` with concrete assertions before marking it executable;
the harness already accepts ordinary command files and does not acquire a
command grammar.

## Canonical real-relay evidence

[`evidence/2026-08-28-smoke/`](evidence/2026-08-28-smoke/) is a compact,
secret-free successful smoke record. `result.json` records the selected binary
SHA-256 values and ordinary-relay version; `app-results.jsonl` is REPL output;
`inspections/` is independent direct REQ/EOSE readback; and `manifest.json`
hashes every retained record. It intentionally excludes run logs, relay logs,
relay databases, commands, and TMPDIR. Re-run the command above when either
binary changes; this bundle identifies a historical run, not a fresh proof.

Run the harness contract tests:

```sh
python3 -m unittest discover -s examples/simple-groups/live/tests -v
```
