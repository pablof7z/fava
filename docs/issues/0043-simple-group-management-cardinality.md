# 0043 — NIP-29 management target cardinality

**Status:** implemented and focused-validated, 2026-08-28
**Owner:** `fava-simple-groups` for NIP-29 management-event body syntax
**Depends on:** 0042 — `EventBuilder` reopens a raw unsigned payload

## Decision

`invite`, `put_user`, and `remove_user` take an ordered slice of public keys
and emit one exact `p` tag for every supplied key, preserving duplicates and
order. One call therefore builds one NIP-29 management event for one or many
targets; callers do not loop and rebuild the common group body themselves.

An empty slice is encoded exactly as no `p` tags for all three constructors.
`fava-simple-groups` performs no target-cardinality or target-count policy;
relay acceptance remains outside its event-body construction. Universal tag and
byte bounds remain exclusively `fava-write`'s `EventBuildError` behavior after
the completed body is constructed.

Kind 9009 `create-invite` requires a `code` tag; invitee `p` tags remain
optional. This constructor does not yet accept the code, so callers must reopen
its unsigned body with issue 0042's `EventBuilder::from` and append the exact
`code` tag before publication. That temporary composition/DX gap is generic
body composition, not a management-specific error, route, or invite-code type.

The later embedded-routing `EventBuilder` decision belongs to its own builder
slice. These constructors return unsigned event bodies and neither attach,
derive, nor document automatic routing behavior.

## Vocabulary reconciliation

`invite`, `put_user`, and `remove_user` remain the established NIP-29
constructor terms. Their meanings change from one target to ordered target
cardinality, including zero. No public nominal error, wrapper, route concept,
invite-code type, or compatibility overload is introduced.

Counterexample: adding a management-local empty-slice refusal, count maximum,
or NIP-29-specific error would duplicate a policy Pablo assigned to the relay.
Using a local count check to restate `fava-write`'s universal limits would also
give this crate a generic construction-bound owner.

## Falsifiers and evidence

- Unit evidence proves exact one/many target order, repetitions, common roles,
  and zero-target encoding without `p` tags for all three constructors.
- Caller compilation proves the old singleton shape is removed.
- The bounded local `nostr-rs-relay` readback submits one multi-target body per
  constructor, requires its matching EOSE, and asserts exact ordered, repeated
  stored `p` rows plus the invite `code`. Timeout, closure, and WebSocket
  failure are typed test failures. It is relay storage evidence only, not
  generic relay or NIP-29 authority.
- Dropping a target, reordering a repeated target, changing common roles, or
  adding a local empty-slice policy makes focused evidence fail.

The causal RED run changed the management-owner tests first, then ran
`cargo test -p fava-simple-groups management::tests --lib`. It failed with the
old singleton `&PublicKey` signatures and one-target `p` encoding where ordered
target slices and repeated `p` rows were required.

The GREEN evidence is:

- `cargo test -p fava-simple-groups management::tests --lib` — focused
  management encoding evidence passed;
- `cargo test -p fava-simple-groups --test management_e2e
  gate4_all_constructors_accepted -- --ignored --nocapture` — when
  `nostr-rs-relay` is available, acknowledges all nine bodies and boundedly
  reads back the exact ordered repeated `p` rows for invite, put-user, and
  remove-user. The invite body gains its required `code` through 0042
  reopening before submission;
- `cargo test -p fava-simple-groups --test management_e2e --no-run`,
  `cargo check --manifest-path examples/simple-groups/Cargo.toml`, and
  `cargo check --manifest-path apps/canary/Cargo.toml` — public callers compile;
- `python3 -m unittest tools/tests/test_vocabulary_check.py

backlog of unrelated nominal and specified vocabulary. Croissant source is
present but no external binary was available; no Croissant or NIP-29 relay
claim is made here.

## Scope

Carry only the management API, its tests and public callers, README/API
catalogue, this decision, and the existing DX-review signature sample. Exclude main checkout changes to
signing, query/edit/saved/people doctests, lockfiles, skills, and all unrelated
test work.
