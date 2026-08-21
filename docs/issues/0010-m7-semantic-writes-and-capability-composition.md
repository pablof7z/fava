# M7: semantic writes and capability composition

**Status:** in progress
**Branch:** `milestone/m7-semantic-writes`
**Authority:** `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, M7

## Problem

The completed M6 publication lifecycle accepts only finalized unsigned or
pre-signed events. It cannot durably retain a protocol-owned replaceable-event
edit, materialize it against current qualified source state, or replace that
materialization when newer source state arrives. Protocol crates therefore
cannot yet express edits such as follow/unfollow without applications manually
rebuilding whole events.

## Scope

- Add a neutral, bounded, persistable replaceable-event-edit value and public
  materializer contract without teaching universal owners event-kind meaning.
- Extend memory and redb write-store custody with stable operation, receipt, and
  materialization-generation identity.
- Extend publication with first-value materialization, source-driven
  rematerialization, and exact stale-generation rejection across signing,
  routing, publishing, and delivery.
- Add two unrelated protocol crates: NIP-02 follows and a bookmarks capability.
- Prove the complete behavior through the public `fava` API, a shared
  capability corpus, restart evidence, dependency-negative checks, and an
  external N+1 falsifier.

## Non-goals

- Native Swift/Kotlin projection, product profiles, authentication, and M8+
  hostile-boundary expansion.
- Protocol-specific branches in `fava`, publication, query, routing, stores,
  transport, signer, publisher, or delivery owners.
- A second receipt, signing, routing, retry, transport, cache, or optimistic-row
  lifecycle owned by a protocol crate.

## Exit gates

- Every CAP-01 through CAP-09 behavior has direct falsifiable evidence.
- A first-value follow operation is accepted and visible through the ordinary
  write-store query source and receipt before relay acknowledgement.
- A newer qualified source rematerializes a live edit atomically, preserves
  unrelated source changes, keeps the same write/receipt identity, and makes
  retired completions inert but attributable.
- Follow/unfollow and bookmark/unbookmark pass one public conformance corpus.
- Adding an external capability changes only that crate and selected assembly
  metadata; arbitrary/future raw kinds remain usable.
- The full proportional validation set passes and the focused issue records the
  deliberate-break evidence.
