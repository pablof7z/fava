# M1: local event state and merged query sources

**Status:** complete
**Authority:** `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, M1

## Product result

A public `Query` observes one deterministic current event view assembled from
independent `EventCache` and `WriteStore` sources. Local writes remain outside
the event cache. Removal, deletion, expiration, write cancellation, and source
revision changes update the same open observation.

## Required behavior

- Event identity and replaceable-event selection are deterministic; equal
  timestamps select the lowest event ID.
- Authorized NIP-09 deletion retracts matching events and prevents their
  resurrection; another author cannot delete them.
- NIP-40 expiration retracts an event at its declared timestamp.
- The event cache accepts only verified signed relay events.
- The write store exposes unsigned and signed local events independently.
- Equal event IDs merge into one `EventRecord` with relay and publication
  evidence.
- A query-matching local replaceable event shadows a matching cached predecessor; cancellation reveals
  the predecessor without changing the cache.
- Query opening is all-or-nothing.
- Equivalent query construction produces equal values and hashes.
- Observation delivery retains bounded latest state for slow consumers.

## Exit-gate evidence

- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo test --manifest-path apps/canary/Cargo.toml`
- `cargo clippy --manifest-path apps/canary/Cargo.toml --all-targets -- -D warnings`
- `bazel test //...`
- `python3 tools/check_vocabulary.py`
- `python3 -m unittest tools/tests/test_vocabulary_check.py`
- The same add/remove source corpus passes unchanged against
  `MemoryEventCache` and `MemoryWriteStore`.
- No relay, transport, routing, runtime-networking, HTTP, or WebSocket dependency
  occurs in the M1 crates.

## Canary evidence

- `local-source-merge`: one application-visible event with merged relay and
  publication evidence.
- `local-replaceable-shadow-and-cancel`: local successor visible, then cached
  predecessor visible after cancellation.
- `local-source-removal`: expired event removed from the same open query.

All three scenarios execute queries and writes through the public `Fava`
facade. Cache mutation is used only to seed or remove relay-observed state.

## Falsifier evidence

Deliberately emitting the merged event twice makes
`same_signed_event_merges_relay_and_publication_evidence` fail with two records
instead of one. Restoring exact-ID merge makes the full corpus pass.
