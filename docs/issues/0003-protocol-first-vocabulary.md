# Protocol-first architectural vocabulary

**Status:** complete
**Scope:** repository rules, public query identity, replaceable-event edits, protocol crates, specifications, and automated vocabulary review

## Why

Fava should be understandable through ordinary Nostr concepts. Its own terms
exist only for behavior or state Fava actually owns, and every public
architectural symbol must resolve to one defined concept.

## Outcomes

- O-1: `Query` is the sole public query value and equivalent construction order produces equal identity.
- O-2: `ReplaceableEventEdit` names durable edits that can be reapplied to the latest event at their coordinate.
- O-3: protocol crates own protocol meaning and compose the generic `EventBuilder` without a generic extension layer.
- O-4: `EventBuilder` knows only generic Nostr event fields and validated tags.
- O-5: the authoritative specifications consistently treat addressable events as replaceable events.
- O-6: every public architectural Rust symbol resolves to a definition under `docs/internals/`.
- O-7: an automated check rejects undocumented public symbols and suspicious new architectural vocabulary.

## Invariants

- I-1: protocol vocabulary is preferred whenever it precisely names the concept.
- I-2: query validity is established during construction, before source or relay work opens.
- I-3: concrete protocol crates use the ordinary query, event-building, and write paths.
- I-4: application terms remain distinct from provider mechanisms.
- I-5: architectural vocabulary changes require explicit human approval.

## Proof

- query identity and invalid-construction tests;
- public facade tests using `Query` directly;
- vocabulary-check unit fixtures and a clean repository scan;
- repository-wide searches for vocabulary that contradicts the current model;
- workspace tests, Clippy, formatting, and public-contract conformance.

## Evidence

- Red: `crates/fava-query/tests/query_identity.rs` initially failed to compile because the public `Query` value did not exist.
- Red: the replaceable-coordinate subset test initially failed because a replaceable coordinate did not accept an addressable identifier.
- Red: the documentation-vocabulary fixture initially passed with an unregistered public specification symbol.
- Green: `cargo test --workspace --all-targets`.
- Green: `cargo clippy --workspace --all-targets -- -D warnings`.
- Green: `cargo fmt --all -- --check`.
- Green: `cargo test --manifest-path apps/canary/Cargo.toml` and its Clippy gate.
- Green: `bazel test //...`, including `//crates/fava-query:query_identity`.
- Green: `python3 tools/check_vocabulary.py`.
- Green: `python3 -m unittest tools/tests/test_vocabulary_check.py`.
- Audit: all code files are at or below 500 lines, and repository vocabulary matches the current model recorded here.
