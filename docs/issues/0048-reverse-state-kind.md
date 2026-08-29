# 0048 — Simple-group state-kind reverse conversion

**Status:** implemented and validated, 2026-08-28

**Owner:** `fava-simple-groups` owns the closed NIP-29 simple-group state-kind family.

## Decision

`SimpleGroupStateEventKind` implements `TryFrom<Kind>` for exactly kinds 39000 through 39005. Each maps to its matching state-event variant. Every other `Kind` is refused as `Err(kind)` with the original generic value unchanged.

There is no error wrapper, alias, alternate reverse method, or range-based admission rule. The existing enum remains the only typed state-kind vocabulary.

## Consumer boundary

The simple-groups canary derives the complete request family from `SimpleGroupStateEventKind::ALL` and classifies received generic kinds through this conversion. It owns event handling, not the numeric mapping.

## Falsifier and evidence

`generic_kind_converts_only_exact_state_event_kinds` proves every accepted mapping and proves `Kind::from(39006)` is refused unchanged. It fails if a mapping changes, an out-of-family kind is accepted, or refusal loses the original value.

The canary semantic evidence uses the same public conversion, so replacing it with a numeric dispatch table would remove the intended consumer use of the owner API.

## Validation

- `cargo test -p fava-simple-groups --locked`
- `git diff --check`

## Scope

Only the reverse conversion, its canary consumer, and its API/vocabulary evidence are in scope.
