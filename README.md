# Fava

A from-scratch Nostr client engine.

Fava is an embeddable library built around declarative live queries and durable
write intents. Its current query path merges local sources, verifies live relay
events, tracks exact provenance, reconnects with fresh request identity, and
reacts to an ordered automatic router chain. Applications select independent
cache, query, routing, subscription-planning, and transport providers.
Its write path durably accepts unsigned or verified signed events, routes them
through an application-selected ordered router chain or exact relay set,
delivers immediately to known relays while discovery remains unresolved,
records exact per-relay receipts, supports pre-handoff cancellation, and
resumes accepted work after process death.

The ordinary downstream acceptance application lives at [apps/canary](apps/canary).

## Specifications

The authoritative inputs live in [docs/spec](docs/spec/README.md). Architectural
concepts are defined in [docs/spec/ARCHITECTURE.md](docs/spec/ARCHITECTURE.md).
Public declarations are approved out of repository by Symbol Gate; its policy and
trusted keys live in [.symbol-gate](.symbol-gate).
