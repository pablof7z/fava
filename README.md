# NMP

A from-scratch implementation of the Nostr Multi-Platform client engine.

NMP is an embeddable library built around two long-lived workloads:
declarative live queries and durable write intents. Applications assemble
focused providers at build time while NMP preserves universal event, evidence,
query, publication, cancellation, and lifecycle semantics.

The rewrite is being built as executable vertical slices. The local-source
merge foundation is complete in [local issue #1](docs/issues/0001-local-source-merge.md).

## Specifications

The authoritative inputs are vendored unchanged in [docs/spec](docs/spec/README.md).

## Repository status

This repository is local-only. It intentionally has no Git remote.
