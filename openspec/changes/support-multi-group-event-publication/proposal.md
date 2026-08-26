## Why

Simple-group publication currently binds one group tag to an unsigned event and requires the application to repeat that group's hosts through `fava.to(...).publish(...)`. That shape cannot express one event belonging to several groups while deriving the complete exact relay route from the selected `SimpleGroup` values.

## What Changes

- **BREAKING**: make `EventBuilder` carry bounded neutral publication routing separately from the Nostr fields that determine event identity and signature.
- Add a `fava-simple-groups` extension trait whose `.simple_group(...)` method appends one exact `h` tag, contributes the group's hosts through the existing generic explicit-route accumulator, returns the same concrete `EventBuilder`, and propagates its generic route refusal directly.
- Permit repeated `.simple_group(...)` calls so one unsigned event contains several distinct group contexts and one first-occurrence-ordered, deduplicated relay union.
- **BREAKING**: accept `EventBuilder` directly through `fava.publish(builder)`; use its embedded explicit route instead of requiring `fava.to(...).publish(...)`.
- Refuse conflicting external and builder-carried explicit routes before signing or durable custody.
- Refuse event-only building when doing so would silently discard builder-carried publication routing.
- Replace the unsigned single-group `prepare` publication surface and its authoritative documentation with the multi-group builder composition model; keep pre-signed validation as a distinct unchanged-event path and add no compatibility alias or shim.

## Capabilities

### New Capabilities

- `simple-groups/multi-group-publication`: Fluent construction and publication of one event across a bounded set of simple groups, with signed group contexts and neutral exact relay routing kept under their existing owners.

### Modified Capabilities

None. This repository has no existing OpenSpec capability baseline; the authoritative Fava specifications will be updated by the implementation change.

## Impact

- Public Rust surfaces in `fava-write`, `fava-simple-groups`, and the `fava` facade.
- Event construction, explicit-route validation, publication admission, receipts, restart recovery, and simple-group examples/tests.
- `docs/spec/` statements that currently require exactly one `h`, `SimpleGroup::prepare`, and `fava.to(simple_group.hosts()).publish(...)`.
- `docs/internals/vocabulary.toml` and generated vocabulary evidence for the new extension-trait surface and removed publication symbols.
- Relay interoperability evidence for events carrying more than one `h` group context.
