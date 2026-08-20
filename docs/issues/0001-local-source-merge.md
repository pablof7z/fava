# Local source merge vertical slice

**Status:** implemented tracer; milestone M1 incomplete
**Spec slice:** Architecture Part XIII, Slice 1  
**Branch:** `rewrite/0001-local-source-merge`

## Why

The rewrite needs one real end-to-end path before its provider contracts can
stabilize. The first path must prove that relay-observed cache state and local
accepted-write state remain independent authorities while applications observe
one coherent event view.

## Outcomes

- O-1: opening a query returns a complete local snapshot without relay work.
- O-2: an accepted unsigned local event is visible through the write-store source and never inserted into the event cache.
- O-3: the same signed event from cache and write store appears once with merged relay and publication evidence.
- O-4: a current local replaceable event overlays a cached predecessor; cancellation reveals the predecessor naturally.
- O-5: source policy distinguishes acquisition-only explicit relays from provenance-constrained explicit relays.
- O-6: slow observation consumers receive bounded exact latest state.

## Invariants

- I-1: `EventCache` retains only signed relay-observed events.
- I-2: `WriteStore` owns local materializations and publication evidence.
- I-3: query evaluation is the only merge authority.
- I-4: source open establishes one coherent initial snapshot plus continuous later revisions.
- I-5: source/result authority and access context remain part of query identity.

## Exclusions

- Relay transport, wire frames, subscriptions, automatic routers, signing, durable publication, restart recovery, Swift, and Kotlin are later slices.
- The five product decisions in the normative specification remain open.

## Reconciliation with the supplied implementation plan

The implementation and named evidence below remain valid for the behaviors
they claim. The complete M1 milestone is not yet earned: M0 must pass first,
and M1 still needs stable equivalent-query identity, deterministic
deletion/expiry semantics, the `local-source-removal` canary, and the declared
shared semantic corpus across both memory providers.

## Proof

- Component tests for canonical source merge, replacement, evidence, and source policy.
- Public `nmp` acceptance tests for initial local state, local visibility, cancellation, and latest-state observation.
- Deliberate breaks named in `features/local-source-merge.feature`.

Verified on 2026-08-20:

- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo test --manifest-path falsifiers/external-null-cache/Cargo.toml`
- `cargo clippy --manifest-path falsifiers/external-null-cache/Cargo.toml --all-targets -- -D warnings`

The deliberate-break pass confirmed that evidence fails when write-store
contributions, relay-evidence merge, replaceable winner selection, or either
explicit source-policy distinction is removed or inverted.

## Architecture result

- `nmp-query` is the sole merge authority.
- Event-cache and write-store providers expose independent continuous source revisions.
- Post-open source termination is source-scoped evidence and does not erase the other source's valid state.
- The public query identity stores acquisition and result authority separately.
- An outside-workspace null event cache assembles through public contracts with no private bypass.
