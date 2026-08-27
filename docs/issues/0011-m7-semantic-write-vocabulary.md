# M7 semantic-write architectural vocabulary

**Status:** complete
**Branch:** `architecture/m7-semantic-vocabulary`
**Approved by:** Pablo, 2026-08-21

## Scope

Approve the minimum cross-crate vocabulary required by M7 before the feature
change uses it. This architecture slice changes no runtime behavior.

## Approved vocabulary

### `ReplaceableEventEdit`

- **Closest concept:** Nostr replaceable event plus coordinate.
- **Observable distinction:** preserves a semantic intention before and across
  immutable event bodies; an ordinary event cannot be reapplied to newer source.
- **Counterexample:** `follow(Bob)` must preserve a newer unrelated `Carol`
  contact when rematerialized; replaying the old whole kind-3 event erases it.
- **Owner/lifecycle:** `fava-write` value; accepted and retained by `WriteStore`;
  interpreted only by the selected protocol materializer.
- **Forcing requirement:** WRITE-002/003/006 and CAP-01 through CAP-05.
- **Why existing state is insufficient:** `WritePayload` currently carries only
  finalized unsigned or signed immutable events.
- **Falsifier:** newer qualified source state cannot preserve unrelated changes
  by resending the predecessor materialization.

### `ReplaceableEventMaterializer`

- **Closest concept:** deterministic construction of a Nostr replacement event.
- **Observable distinction:** a replaceable protocol provider applies one
  protocol-owned edit to qualified source or defined empty state without owning
  custody, signing, routing, publication, delivery, or receipts.
- **Counterexample:** a NIP-02-specific branch inside publication makes a
  bookmarks or N+1 capability require universal-core edits.
- **Owner/lifecycle:** neutral contract in `fava-write`; implementations live in
  protocol crates and are selected by application assembly before recovery.
- **Forcing requirement:** PROTO-001 through PROTO-004 and CAP-07/CAP-08.
- **Why existing state is insufficient:** no public replaceable provider contract
  can interpret durable opaque edit formats after acceptance or restart.
- **Falsifier:** a capability using only public contracts must materialize
  current and empty source without editing universal owners.

### `MaterializationId`

- **Closest concept:** immutable Nostr event id within one accepted write.
- **Observable distinction:** identifies the exact materialization generation
  under stable write/receipt identity even before or after its event is signed.
- **Counterexample:** a late signer, route, publisher, or delivery completion
  correlated only by receipt can mutate a newer generation.
- **Owner/lifecycle:** allocated and persisted by the selected `WriteStore`;
  carried through publication effects and validated at every completion commit.
- **Forcing requirement:** WRITE-006/007/022 and CAP-05/CAP-06.
- **Why existing state is insufficient:** `ReceiptId`, route revision, relay
  session, and attempt number do not identify which immutable event generation
  authorized an effect.
- **Falsifier:** remove one store-side `MaterializationId` currentness check;
  releasing the retired completion must corrupt current receipt/query facts.

## Registry repair

The existing registry had several valid public symbols shifted onto adjacent
terms (`EventBuilder` under `ReplaceableEventEdit`, publisher under
`UnsignedEvent`, delivery under `EventBuilder`, and signer under `Publisher`).
This slice restores each symbol and crate to its actual approved noun so the new
edit symbols do not inherit false ownership.

## Validation

- `python3 tools/check_vocabulary.py`
- `python3 -m unittest tools.tests.test_vocabulary_check`
- `git diff --check`
