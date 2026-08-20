# M2: explicit one-relay live query

**Status:** complete
**Authority:** `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, M2

## Product result

A live public `Query` with one exact explicit relay opens NIP-01 relay work,
admits only attributed verified matching events, updates the ordinary local
event view, preserves exact EOSE and terminal facts, and withdraws its exact
subscription on close.

## Architecture

- `fava-wire` owns exact NIP-01 message encoding and decoding.
- `fava-subscriptions` owns logical demand, exact wire plans, and attribution.
- `fava-subscriptions-no-grouping` emits one REQ per logical demand.
- `fava-transport` owns replaceable session and handoff contracts.
- `fava-transport-websocket` owns bounded WebSocket resources.
- `fava-ingest` owns subscription attribution, verification, filter matching,
  and ordered event-cache admission.
- `fava-diagnostics` owns bounded public relay and subscription facts.
- `fava` opens explicit relay work and binds its cancellation to `Observation`.

## Exit-gate evidence

- One real `nostr-rs-relay 0.8.12` path runs without an automatic router.
- Public diagnostics and the independent proxy agree on relay URL, exact
  subscription ID, REQ, EOSE where required, and CLOSE on one connection.
- The canary uses public Fava and provider APIs only.
- The transport conformance corpus covers handoff success, definite refusal,
  disconnect, and idempotent close.
- Cargo, strict Clippy, Bazel, vocabulary, and external-provider checks apply to
  every M2 crate and acceptance path.

## Canary evidence

- `explicit-read-eose`
- `explicit-read-live-after-eose`
- `explicit-read-cancel`

The EOSE canary also runs a hostile WebSocket witness that supplies a forged
matching EVENT. The event must remain absent from both the event cache and the
application snapshot.

## Falsifier evidence

Removing signature verification from relay ingest, event-cache admission, and
the memory provider made `explicit-read-eose` fail with
`forged-event refusal was not diagnosed`. Restoring verification made the same
hostile witness and complete real-relay scenario pass.
