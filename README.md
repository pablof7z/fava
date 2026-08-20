# Fava

A from-scratch Nostr client engine.

Fava is an embeddable library built around two long-lived workloads:
declarative live queries and durable write intents. Applications assemble
focused providers at build time while Fava preserves universal event, evidence,
query, publication, cancellation, and lifecycle behavior.

The rewrite is being built as executable vertical milestones. M0's independent
evidence foundation passes through the pinned macOS local-process profile in
[local issue #2](docs/issues/0002-m0-evidence-foundation.md). A working M1
local-source tracer exists in [local issue #1](docs/issues/0001-local-source-merge.md),
but M1 remains incomplete until all supplied milestone gates pass.

The ordinary downstream acceptance application lives at [apps/canary](apps/canary).

## Specifications

The authoritative inputs live in [docs/spec](docs/spec/README.md). Architectural
concepts and public symbols are defined in
[docs/internals/vocabulary.toml](docs/internals/vocabulary.toml).

## Repository status

Do not push changes until Pablo explicitly authorizes publication.
