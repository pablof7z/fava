# NMP

A from-scratch implementation of the Nostr Multi-Platform client engine.

NMP is an embeddable library built around two long-lived workloads:
declarative live queries and durable write intents. Applications assemble
focused providers at build time while NMP preserves universal event, evidence,
query, publication, cancellation, and lifecycle semantics.

The rewrite is being built as executable vertical milestones. M0's independent
evidence foundation passes through the pinned macOS local-process profile in
[local issue #2](docs/issues/0002-m0-evidence-foundation.md). A working M1
local-source tracer exists in [local issue #1](docs/issues/0001-local-source-merge.md),
but M1 remains incomplete until all supplied milestone gates pass.

The ordinary downstream acceptance application lives at [apps/canary](apps/canary).

## Specifications

The authoritative inputs are vendored unchanged in [docs/spec](docs/spec/README.md).

## Repository status

This repository is local-only. It intentionally has no Git remote.
