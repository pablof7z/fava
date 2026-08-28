# 0042 — EventBuilder reopens a raw unsigned payload

**Status:** implemented, 2026-08-28
**Owner:** `fava-write` for generic pre-custody event construction and local routing

## Decision

`EventBuilder` accepts a raw `UnsignedEvent` through a consuming public conversion. The conversion preserves the complete unsigned payload—author, kind, timestamp, ordered tags, and content—discards the payload's derived event id, and starts with `WriteRouting::Automatic` because an `UnsignedEvent` carries no local route authority.

The returned builder remains mutable until the final build or publication boundary. A later `.build()` or `fava.publish(builder)` reapplies generic bounds and computes exactly one event id from the final payload.

This generic conversion is independent of the simple-groups decision that protocol constructors normally return builders directly. It exists for exceptional reopening of already-finalized unsigned payloads, not as the normal protocol-composition path.

## Closest existing concepts and insufficiency

`EventBuilder::from_parts` can reconstruct the same payload only when every consumer manually copies five generic fields. That ceremony is omission-prone and makes callers repeat identity-reconstruction mechanics owned by `fava-write`.

`UnsignedEvent` contains a derived id but no local routing. Mutating or reusing that id after changing fields creates an invalid intermediate value. The consuming conversion makes the identity reset explicit and prevents a stale id from surviving into the mutable builder.

No wrapper, compatibility alias, unsigned-event mutation trait, or second builder type is introduced.

## Ownership and lifecycle

Before conversion, the caller owns a finalized unsigned payload. The consuming conversion moves its serialized fields into `EventBuilder`; `fava-write` owns subsequent mutation, bounds, identity computation, and neutral routing state. Publication later lowers the builder into the existing durable write lifecycle.

The conversion opens no relay, signer, write, receipt, subscription, or provider work.

## Observable distinction and counterexample

A caller can consume an unsigned event, append one tag, and build a new valid event without manually copying generic fields. The new event preserves every original payload field and ordered tag, contains the appended tag, and has a recomputed id distinct from the original.

Without this API, consumers must use `from_parts`; preserving the original id after mutation must fail verification.

## Executable falsifiers

- a focused `fava-write` public test consumes an unsigned event and proves exact preservation of author, kind, timestamp, content, and ordered original tags;
- appending a tag then building produces a valid recomputed id distinct from the original;
- `into_event_and_routing()` reports `WriteRouting::Automatic`;
- normal tag and byte bounds still refuse oversized final payloads;
- removing the identity reset, carrying a route, reordering tags, or retaining the old id makes focused evidence fail.

## Evidence

The causal RED run added the focused reopening tests before the conversion
existed. `cargo test -p fava-write --test event_builder` then failed to compile:
`EventBuilder::from` expected `EventBuilder`, not `UnsignedEvent`.

After implementing `impl From<UnsignedEvent> for EventBuilder`, focused crate
validation passed:

- `cargo test -p fava-write` — 21 tests and 4 doctests passed;
- `cargo fmt --check` and `git diff --check` passed;
- `python3 -m unittest tools/tests/test_vocabulary_check.py tools/tests/test_vocabulary_structure.py` — 63 tests passed.

As a deliberate falsifier, reversing the consumed tag iterator made
`reopening_an_unsigned_event_preserves_its_body_recomputes_identity_and_resets_routing`
fail with the original tags in reverse order. The mutation was reverted.

`python3 tools/check_vocabulary.py` remains inherited-red on unrelated
repository vocabulary and specified-but-unimplemented items; this slice adds
no new vocabulary diagnostic.
