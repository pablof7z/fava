# 0035 — Bound raw explicit routing before normalization

**Status:** implemented and focused gates complete
**Owner:** `fava-write` for neutral route construction; `fava` for the public publication door

## Decision

Every public explicit-write route accepts a finite owned relay vector, never an
arbitrary iterator. `fava-write` refuses more than 1,024 raw relay occurrences
before duplicate normalization. It separately preserves the existing maximum
of 256 distinct destinations after first-occurrence normalization.

`EventBuilder` counts raw relay occurrences cumulatively across `to_relays`
calls. Repeating one relay 1,025 times therefore returns
`WriteIntentError::TooManyRawExplicitRelays`; repeating it 1,024 times consumes
one destination slot, not 1,024. The raw count is transient construction work
and is not serialized into durable `WriteRouting`.

## Forcing requirement and falsifier

An infinite iterator must be unrepresentable at the public route boundary, and
a finite hostile input must refuse before normalization, custody, signing, or
provider work. Removing the pre-normalization check makes the focused raw-route
tests fail. Counting duplicates against destination capacity makes the existing
distinct-identity routing test fail.

## Validation

- `cargo test -p fava-write --test routing_order --test event_builder`
- `cargo test -p fava --test publication_scopes`
- `cargo test -p fava-simple-groups --test public_api --test architecture`
- `cargo test -p fava-write --doc`
- `cargo check --workspace --all-targets`
- `cargo clippy -p fava-write --all-targets -- -D warnings`

The focused tests, doctests, workspace check, and owning-crate Clippy gate pass.
The global vocabulary checker still reports the repository's pre-existing
architectural-symbol backlog; it reports no bounded-routing-specific finding.
