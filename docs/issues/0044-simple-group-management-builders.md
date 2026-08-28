# 0044 — Simple-groups management constructors return self-routed builders

**Status:** implemented and focused-validated, 2026-08-28
**Owner:** `fava-simple-groups` for NIP-29 management-event construction
**Depends on:** 0042 — `EventBuilder` reopens a raw unsigned payload; 0043 — NIP-29 management target cardinality

## Decision

All nine NIP-29 management constructors (`create_group`, `edit_metadata`,
`invite`, `join_request`, `put_user`, `remove_user`, `delete_event`,
`delete_group`, `leave_group`) return `Result<EventBuilder, WriteIntentError>`
with the group's relays already embedded as explicit routing. Callers publish
with `fava.publish(builder)`. The old `fava.to([relay]).publish(unsigned_event)`
pattern is removed from all management callers.

`WriteIntentError` (already owned by `fava-write`) is the return error for
relay-bound construction. Universal tag and byte limits remain exclusively
`fava-write`'s `EventBuildError` behavior after the completed body is
constructed. No management-local error type is introduced.

Issue 0043 explicitly deferred this embedded-routing decision to its own builder
slice. This issue resolves it.

## Invite and join-request NIP-29 authority

NIP-29 upstream is authoritative. `invite` takes an exact required `code: &str`
and emits `h` and `code` tags only — no `p` tags and no relay tag. Invitee `p`
tag handling belongs to relay-acceptance policy, not this constructor. The
previous 0043 formulation ("invitee `p` tags remain optional; callers reopen via
0042 to append code") is superseded: the constructor now accepts and emits the
code directly, and no invitee slice is accepted.

`join_request` takes `code: Option<&str>`. When `Some`, it emits the exact
`code` tag alongside `h`; when `None`, no code tag appears. An optional reason
stays ordinary builder `.content(...)` on the returned builder; no reason
parameter is added to this constructor.

## Vocabulary reconciliation

All nine constructor meanings are updated from "unsigned event" to
"self-routed event builder" with `fava.publish(builder)` as the publication
path. `invite`'s meaning removes the invitee slice and relay, replacing it with
exact required code and `h`/`code`-tags-only. `join_request`'s meaning adds the
optional code parameter.

No new nominal error, wrapper, route concept, or invite-code type is introduced.
No compatibility overload or backwards-compatibility path is introduced.

## Falsifiers and evidence

- Unit evidence (`cargo test -p fava-simple-groups management::tests --lib`)
  proves each constructor produces a valid `EventBuilder` and that invite emits
  `h` and `code` tags only with no `p` tags. Changing `invite` to accept a `p`
  slice, or removing the required code parameter, makes these tests fail.
- The bounded local `nostr-rs-relay` readback
  (`cargo test -p fava-simple-groups --test management_e2e gate4_all_constructors_accepted
  -- --ignored --nocapture`) submits all nine builders via `fava.publish(builder)`,
  requires its matching EOSE, and reads back stored events. Invite code readback
  and ordered `p` rows for `put_user` and `remove_user` are asserted exactly.
  Replacing `fava.publish(builder)` with the old `fava.to(relay).publish(event)`
  pattern makes the relay not receive correctly-routed events.
- `cargo test -p fava-simple-groups --test management_e2e --no-run` — e2e file
  compiles;
- `cargo check --manifest-path examples/simple-groups/Cargo.toml` — demo compiles
  with `fava.publish(builder)` throughout;
- `cargo check --manifest-path apps/canary/Cargo.toml` — canary compiles;
- `python3 tools/crate_readme_api.py check fava-simple-groups` — README API
  inventory current.

The causal RED run changed management callers to `fava.publish(builder)` before
the constructors returned `EventBuilder`. It failed because the constructors
still returned `UnsignedEvent`, producing type mismatches at every call site.

## Scope

Carry only the management API, its tests and public callers, README/API
catalogue, vocabulary.toml management term meanings, and this decision. Exclude
main checkout changes to signing, query/edit/saved/people doctests, lockfiles,
skills, and all unrelated test work.
