# 0050 — Independent simple-groups experiential REPL live proof

**Status:** application and harness complete; the sole full-contract gap is
Croissant hiding its acknowledged kind-9008 deletion event from direct queries
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

Protected `account import` is deliberately not a command-file exception. Its
explicit user-approved proof uses one isolated PTY only: the harness generates
one valid mutable nsec, feeds it only after the app's no-echo prompt, rejects a
PTY echo, returns only typed public fields, erases scratch, scans every
retained artifact for that exact nsec, and zeroes the input. The harness still
does not construct, sign, or publish an event. The application imports through
`Fava::add_signer`, and the harness separately proves the returned public key
authored exact-ID kind 9007 and 12345 events before matching `EOSE`.

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

## Full controlled result — 2026-08-29

The ordinary full REPL command file ran through four bounded checkpoints and
eighteen direct assertions against explicit Croissant and PATH
`nostr-rs-relay 0.8.12`. It proves create, metadata, invitation, join/leave,
member management, authorized content, outsider rejection and id absence,
relay-authored 39000–39003 with a common non-app author, kind-10009 only on
the ordinary relay, kind-9005 and target absence, and post-delete state/content
and id absences. It makes no 39004/39005 claim.

The final positive kind-9008 assertion correctly fails: its typed application
capture records acknowledgement, while an independent direct `REQ` by id gets
the matching `EOSE` with no event. Croissant removes the group before its query
visibility check and hides its events from unauthenticated readers. This is a
relay fixture visibility gap, not an application or harness bypass.

[`2026-08-29 Croissant gap evidence`](../../examples/simple-groups/live/evidence/2026-08-29-croissant-9008-retention-gap/)
is the bounded negative control. It retains typed captures, all 18 direct
`REQ`/`EOSE` inspections, and a hash manifest; no database, relay log, command,
or temporary directory is retained.

## Protected account-import result — 2026-08-29

The isolated PTY proof ran against explicit
`/Users/pablofernandez/Work/croissant/croissant` and PATH
`nostr-rs-relay 0.8.12`. The generated nsec imported successfully without echo;
the returned public key is the exact author of the Fava-created kind-9007 event
and Fava-published arbitrary kind-12345 content event. Each independent
group-relay query returned exactly that event and its matching `EOSE`. The
retained run scan used the actual nsec and passed.

[`2026-08-29 account-import evidence`](../../examples/simple-groups/live/evidence/2026-08-29-account-import-proof/)
contains only public captures, two direct inspections, result metadata, and
their hashes. It contains neither the nsec nor a PTY transcript.

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

For protected import, making the generated scalar invalid, accepting an echoed
nsec, losing the typed imported public key, allowing the create/content author
to differ from it, omitting either matching EOSE, or retaining any `nsec1`
material in canonical evidence fails a bounded unit or live assertion.
