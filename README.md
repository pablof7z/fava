# Fava

A from-scratch Nostr client engine.

Fava is an embeddable library built around declarative live queries and durable
write intents. Its current query path merges local sources, verifies live relay
events, tracks exact provenance, reconnects with fresh request identity, and
reacts to an ordered automatic router chain. Applications select independent
cache, query, routing, subscription-planning, and transport providers.

The ordinary downstream acceptance application lives at [apps/canary](apps/canary).

## Specifications

The authoritative inputs live in [docs/spec](docs/spec/README.md). Architectural
concepts and public symbols are defined in
[docs/internals/vocabulary.toml](docs/internals/vocabulary.toml).
