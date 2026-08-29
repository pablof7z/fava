# 0050 — Independent simple-groups experiential REPL live proof

**Status:** harness implemented; complete NIP-29 application flow blocked on
missing REPL command file
**Owner:** `examples/simple-groups/live` owns only relay lab lifecycle,
application invocation, artifacts, and independent wire observation;
`examples/simple-groups` owns all command grammar and public-Fava event work
**Related:** [0045 private E2E REPL foundation](0045-e2e-repl-foundation.md),
[0031 simple-groups real-relay demo](0031-simple-groups-real-relay-demo.md)

## Decision

The live harness is a standalone Python standard-library utility, not a Fava
crate, provider, protocol API, or shell framework. It starts a real Croissant
NIP-29 relay for every authorization claim and a separate real
`nostr-rs-relay` for kind-10009 saved-list state. It gives both ephemeral
loopback endpoints, empty data directories, bounded readiness via an
independent WebSocket `REQ`/`EOSE`, bounded logs, app deadline, process-group
teardown, and one fresh artifact directory. Relay databases, relay logs,
applied commands, and child `TMPDIR` are ignored scratch state, removed
before retained artifacts are scanned on every exit path. The ordinary relay's
generated configuration is explicitly pinned to
and version-checked as `nostr-rs-relay 0.8.12`; both selected executable
SHA-256 values enter `result.json` and per-run JSONL.

The harness invokes the REPL with an ordinary command file and `--jsonl`. It
never builds an event, loads a signer, decides routes, or interprets a Fava
receipt as wire proof. It reads each relay directly through one bounded NIP-01
`REQ`, requires the matching `EOSE`, then compares exact event ID, author,
kind, content, and entire tag array. A negative assertion fails if its direct
relay query returns any event.

The reusable scenario format maps typed REPL JSONL result fields to these
independent assertions. An executable scenario needs at least one concrete
assertion: a positive one names exact `id`, `pubkey`, `kind`, `content`, and
whole `tags`; a negative one has a nonempty filter and direct absence result.
`required_facts` is blocked-only; changing only its status is refused before a
relay or artifact is created. Current event results expose public `author`
alongside `event_id`, so an assertion does not infer identity from a private
key or from an app cache.

## Current executable evidence

`smoke-create-content` runs the current commands:

```text
relay add group <NIP-29 URL>
relay add state <ordinary URL>
account use alice
group create <group-id> group
group event publish --kind 12345 "arbitrary-kind content"
dump
quit
```

It independently proves kind 9007 and kind 12345, the exact generated IDs and
Alice author, the sole `h` tag, exact arbitrary content, and a matching EOSE.
It starts the ordinary relay as a genuine separate state source but makes no
saved-list claim yet.

`secret-nondisclosure` supplies a deterministic nsec-shaped sentinel as a
short-lived command input. It expects the REPL to refuse that script, requires
the direct group-relay query to find no kind-12345 event, erases transient state
before retention, and scans every durable artifact. Any echo in app output,
inspection evidence, result, or harness JSONL is a failure.

The committed
[`2026-08-28 smoke bundle`](../../examples/simple-groups/live/evidence/2026-08-28-smoke/)
contains the selected binary hashes, app JSONL, direct REQ/EOSE inspection, run
result, and a manifest of artifact hashes. It contains no relay database, relay
log, command file, or temporary directory. It is historical evidence for the
named binary digests; a changed fixture requires a fresh run.

The Python contract tests falsify exact subscription matching, exact tag
comparison, unresolved command placeholders, ambiguous REPL evidence, and
non-deterministic JSONL field order. They use a local scripted wire peer, not
a fake success path for the live scenario.

## Explicit remaining application work

`full-nip29-contract` is blocked and its runner exits 2 before starting a
relay. The present REPL has no ordinary commands for metadata configuration,
member addition/removal/join, save/remove/rename kind-10009 state, event
deletion, or group deletion. It therefore cannot honestly demonstrate:

- authorized member write plus rejected non-member write and direct absence;
- relay-authored kinds 39000 through 39005 after configuration/member changes;
- kind-10009 published and read only from the ordinary relay;
- exact kind-9005 `h`/`e` deletion, its target's absence, kind-9008 deletion,
  and deleted-group state absence.

When application commands and JSONL fields exist, add their stable command
file beneath `examples/simple-groups/live/commands/`, replace `required_facts`
with concrete assertions, then mark the scenario executable. No harness command
parsing or Fava/event code belongs in that change.

## Local fixture discovery

This machine currently has a Croissant executable at
`/Users/pablofernandez/Work/croissant/croissant` and
`nostr-rs-relay 0.8.12` on `PATH`; `relay29` and `communities-relay` are not on
`PATH`. The harness requires explicit executable arguments to make a later
run's fixture selection visible. The Croissant source checkout is dirty, so
the harness records no source-cleanliness or cross-implementation claim.

## Falsifiers

Replacing relay inspection with REPL output, treating an open TCP socket as
ready, accepting an EOSE for another subscription, ignoring an extra tag,
exceeding total event/artifact/file/count bounds, leaving a descendant process
group alive after its parent exits, preserving the secret input, skipping the
failure-path retained scan, or turning a blocked full-flow contract into a pass
must fail focused tests or the live run. Replacing the NIP-29 relay with an
ordinary relay invalidates all authorization claims by construction.
