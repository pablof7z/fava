# `fava-simple-groups` NIP-29 value capability

**Status:** implementation reconciled; 19 unapproved and 3 stale owner signatures pending
**Owning phase:** 07.1.1

This issue records the implemented event-local, saved-list, and operation
composition boundaries. It is not an approval source. The approved constructor
boundary is recorded in
`docs/issues/0027-simple-group-relay-input-boundary.md`.

its exact compiler-derived per-term records are pinned in
identities bind 113 public API records and require current structurally bound
owner signatures. Nineteen are `unapproved`; three are `stale`. The append-only
approval history contains earlier text-only events for `SimpleGroupMetadata`,
`SimpleGroupAdmins`, and `SimpleGroupStateEventKind`; the structural-approval
contract intentionally makes those events stale without deleting them. This
issue and commit series do not fabricate or claim replacement signatures.

## Delivered model

`SimpleGroup` retains a non-empty opaque id plus normalized
application-selected relays. `new(id, relays)` accepts a finite owned
`Vec<RelayUrl>` and returns `SimpleGroupConstructionError::EmptyId` or
`EmptyRelays` for exactly those invalid inputs. Later duplicates collapse in
first-occurrence order. Parsing remains with `RelayUrl`; there is no string
constructor, arbitrary-iterator input, shared relay owner, compatibility API,
or private numeric bound.

- `events` must preserve an ordinary query, constrain lowercase `h` to exactly
  the group id without broadening an existing axis, and use `from_relays`.
  It delegates exact narrowing to query-owned `Query::intersect_tag_values`;
  disjoint axes remain present-empty match-nothing queries. Issue 0028 records
  Pablo's architecture approval, not vocabulary approval.
- `meta_events` delegates its kind input to `Query::kinds`, adds exact
  `d = id`, and uses `only_from_relays` without a private limit.
- `prepare` accepts only `UnsignedEvent`, preserves all existing tags, and
  appends one matching `h` tag only when no first value already matches.
- Applications publish with `fava.to(simple_group.relays()).publish(payload)`;
  the capability owns no work or lifecycle.

## Event-local decoding

`SimpleGroupMetadata`, `SimpleGroupAdmins`, `SimpleGroupMembers`,
`SimpleGroupRoles`, `SimpleGroupLivekitParticipants`, and `SimpleGroupPins`
decode one ordinary `EventValue` each. They check the exact kind and the first
`d` tag's first value. Unknown tags, unused extras, and later `d` tags are
ignored. Repeated semantic entries remain ordered; malformed entries remain
local `Result` failures. Pin `e` and `a` tag entries remain interleaved as
the existing `EventCoordinate` value.

The decoders do not verify ids or signatures, interpret relay evidence, choose
replacement winners, project snapshots, compare relays, or impose generic
bounds.

## Saved group lists

`saved_group_lists(authors)` returns the ordinary kind-10009 query or the query
owner's exact provisional resource refusal.
`SavedGroupList::from_event` creates one list per event and exposes ordered
saved-group and relay entry results. Repetitions survive and malformed siblings
do not erase valid entries.

Crate-root save, rename, remove, and relay functions return pure
`ReplaceableEventEdit` values. `saved_group_list_materializer()` integrates the
private edit codec with Fava's ordinary semantic-write lifecycle. Edits
preserve opaque content, foreign tags, malformed entries, unused trailing
values, repetitions according to the exact operation, and unrelated order.

## Current approval identities

- `SimpleGroup`, `SimpleGroupConstructionError`, and
  `SimpleGroupStateEventKind`.
- `SimpleGroupMetadata`, `SimpleGroupAdmins`, `SimpleGroupMembers`,
  `SimpleGroupRoles`, `SimpleGroupLivekitParticipants`, `SimpleGroupPins`, and
  `SimpleGroupDecodeError`.
- `SavedSimpleGroup`, `SavedGroupList`, `SavedGroupListMaterializer`, and
  `SavedGroupListDecodeError`.
- Crate/function identities `fava_simple_groups`, `save_simple_group`,
  `remove_saved_simple_group`, `rename_saved_simple_group`, `save_relay`,
  `remove_saved_relay`, `saved_group_list_materializer`, and
  `saved_group_lists`.

These 22 owner-scoped packets have zero review problems and zero public-API
retains 354 inherited unique findings, and terminal-name plus external
owner-signature gates remain independently red.

`RelayUrl`, `Query`, `QuerySnapshot`, `EventCoordinate`, `PublicKey`,
`UnsignedEvent`, `Write`, and `ReplaceableEventEdit` remain owned by their
established crates. No compatibility aliases, group-specific provider,
snapshot, projection, disagreement, management, discovery, verification,
bounds, observation, publication, cancellation, or receipt value exists.

## Architecture boundaries

1. Normal dependencies are exactly `fava-query`, `fava-state`, `fava-write`,
   and `nostr`; callers use the established `RelayUrl` parser.
2. Generic owners retain bounds, verification, provenance, replacement,
   projection, routing, storage, signing, delivery, and lifecycle policy.
   Query builders delegate kind, tag-value, exact tag-axis intersection, and
   relay inputs to bounded `fava-query` constructors and return the resulting
   `QueryError` directly.
   `SimpleGroup` construction owns only its exact empty-id and empty-relay
   refusals through `SimpleGroupConstructionError`. Write routing independently
   owns its operation bound and exact `WriteIntentError`; the capability defines
   no query-refusal wrapper, and `fava-state` owns no application
   relay-selection value.
3. Universal Fava owners contain no NIP-29 semantic branch or production
   dependency on this capability.
4. The facade and canary consume only the ordinary queries, event values,
   edits, observations, writes, and receipts.

## Query-error ownership

`events` and `meta_events` return `QueryError` from the query owner without
translation. No group-owned conflict error is permitted. The README catalog is
compiler-derived from the current surface and does not constrain architecture.

## Executable falsifiers

- `cargo test -p fava-simple-groups`
- `cargo test -p fava --test simple_groups`
- `cargo check --manifest-path apps/canary/Cargo.toml --all-targets`
- `python3 tools/crate_readme_api.py check fava-simple-groups`

The architecture target fails when a removed nominal surface, duplicate
generic owner, private bound, verification path, or lifecycle value appears.
Constructor compile-fail and first-occurrence evidence is included in the crate
tests and rustdoc examples required by Pablo's decision recorded in issue 0027.
