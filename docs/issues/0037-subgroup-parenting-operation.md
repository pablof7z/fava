# 0037 — Subgroup parenting is not a create-group constructor

**Status:** design required; incorrect kind-9007 constructor removed, 2026-08-27
**Owner:** `fava-simple-groups`

## Defect

The removed `create_subgroup(author, child, parent_id)` constructor emitted one
kind-9007 create-group event containing `h = <child>` and `parent = <parent>`.
That gives kind 9007 a parenting contract it does not own. NIP-29 assigns
subgroup relationship changes to kind-9002 edit-metadata events; kind 9007
creates an ordinary group.

The constructor also had no authoritative specification, vocabulary decision,
unit or doctest falsifier, generated API inventory, or relay evidence. Adding a
vocabulary term would have documented the wrong behavior rather than repaired
it.

## Required design before implementation

A future subgroup API must express the actual sequence and owners: create the
child as an ordinary group, then set or remove its parent through metadata
editing, including the corresponding child-list semantics required by NIP-29.
It must decide whether Fava exposes those primitive metadata edits or a
higher-level sequenced operation with partial-failure evidence. One unsigned
kind-9007 event cannot claim the whole operation.

Approval requires exact set/detach behavior, authorization and ordering,
partial-failure semantics, vocabulary ownership, decoder interaction, and
executable wire evidence against a relay implementation.
