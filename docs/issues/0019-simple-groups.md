# `fava-simple-groups` multi-relay NIP-29 capability

**Approved by:** Pablo, 2026-08-21
**Owning phase:** 07.1.1

## Delivered model

`SimpleGroup` is a pure value containing one opaque NIP-29 simple group id and
an application-selected, non-empty, bounded host-relay set. One host is the
ordinary relay-local case. Several hosts let an application present related
relay-local forks together while each relay remains independently authoritative
for the records it served.

- Content queries add exact lowercase `h`, request the complete host set, and
  retain `AnyLocal` authority so local write-store events stay visible.
- Record queries add exact lowercase `d`, select kinds 39000 through 39005,
  and use `OnlyRelays` authority for the configured host set.
- `SimpleGroupSnapshot` deduplicates content by event id while retaining every
  actual `RelayEvidence` observation.
- Each host's newest valid complete record is selected independently by Nostr
  timestamp and event id.
- Disagreement compares complete optional per-host records. An unobserved host
  supplies no positive record and makes no absence or completeness claim.
- `SimpleGroupSnapshot::at` applies an explicit application host choice; no
  helper lets one relay speak for another.

## Query and publication composition

Every helper is inert. Reads return ordinary `Query` values. Unsigned and
signed events pass through pure `SimpleGroup::prepare`. Saved-list changes
return ordinary `ReplaceableEventEdit` values.

Applications compose publication only through the facade:

```rust
let prepared = simple_group.prepare(payload)?;
let write: Write = fava.to(simple_group.hosts())?.publish(prepared)?;

let edit = SimpleGroups::save_simple_group(&simple_group, Some("Photography"))?;
let write: Write = fava
    .by(author)
    .to(simple_group.hosts())?
    .publish(edit)?;
```

The complete first-occurrence host sequence is the exact explicit destination
set. Publication remains kind-blind. Invalid signed simple group context is
refused by pure preparation before Fava custody or wire interaction. Kinds
9002 and 9010 remain complete author-bearing events and never enter
`ReplaceableEventEdit`.

## Typed records, discovery, and edits

- `SimpleGroupMetadata`, `SimpleGroupAdmins`, `SimpleGroupMembers`,
  `SimpleGroupRoles`, `SimpleGroupParticipants`, and `SimpleGroupPins` decode
  signed relay-authored records.
- `PinnedItem`, `SavedSimpleGroup`, and `SavedRelay` retain source order and
  exact protocol values without presentation or routing policy.
- `SimpleGroups` creates bounded ordinary saved/admin/member discovery queries
  and projects saving authors from exact simple-group-id and selected-host
  pairs.
- Kind-10009 saved-simple-group and saved-relay changes preserve opaque
  content, foreign and malformed rows, unrelated order, and exact other-host
  rows.

All externally influenced host, id, query, tag, row, string, projection, and
discovery inputs have explicit bounds or typed refusal.

## Approved vocabulary

The capability owns exactly these public nominal values:

- `SimpleGroup`, `SimpleGroupError`, `SimpleGroupRecords`,
  `SimpleGroupSnapshot`, and `SimpleGroups`.
- `SimpleGroupMetadata`, `SimpleGroupAdmins`, `SimpleGroupMembers`,
  `SimpleGroupRoles`, `SimpleGroupParticipants`, and `SimpleGroupPins`.
- `PinnedItem`, `SavedSimpleGroup`, and `SavedRelay`.

`RelayUrl`, `Query`, `QuerySnapshot`, `EventCoordinate`, `PublicKey`, `Write`,
`ReplaceableEventEdit`, and the event values remain owned by their established
crates. No simple-group-specific row wrapper, relay role, provider,
configuration, runtime, observation, publication, delivery, cancellation, or
receipt value is approved.

## Architecture boundaries

1. `fava-simple-groups` normal dependencies are exactly `fava-query`,
   `fava-state`, and `fava-write` in Cargo and Bazel.
2. The capability owns no engine, signer, router, store, publisher, transport,
   runtime, observation, publication, delivery, cancellation, or receipt
   lifecycle.
3. Repeated preparation, query, record, projection, parser, discovery, and edit
   helpers retain no hidden mutable state or provider handle.
4. Universal Fava owners have no production capability dependency, NIP-29
   constant branch, or simple-group-id semantic branch. Generic kind and tag
   handling remains protocol-neutral.
5. Application and canary edges are test-only or app-owned.

## Executable falsifiers

- `cargo test -p fava-simple-groups --test public_api`
- `cargo test -p fava-simple-groups --test architecture`
- `cargo test -p fava --test simple_groups`
- `python3 tools/check_vocabulary.py`

The architecture target fails if a normal facade dependency, retained helper
state, protocol-specific universal branch, wrong management edit path, simple
group lifecycle value, or unregistered public export appears.
