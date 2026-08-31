# 0039 — Make query and subscription identities owner-minted

**Status:** implemented; focused gates complete
**Owner:** `fava-observe` owns issuance; `fava-query` and
`fava-subscriptions` own the opaque value contracts

## Decision

`Round` and `PlanRevision` are opaque identities containing an
authority namespace and a non-zero sequence. The observation engine owns one
non-cloneable issuer for each identity family. Providers and subscription
planners receive identities to echo; they cannot construct, advance, default,
wrap, saturate, or reuse an identity supplied by the owner.

Creating an issuer and advancing its sequence are checked operations.
Exhaustion returns `RoundsExhausted` or `PlanRevisionExhausted`.
Before live relay work exists, query evidence represents the absence of an
operation generation as `None`; it never fabricates generation zero.

`PlanRevision` is included in every derived wire subscription id. Both its
authority and sequence therefore separate reopened plans across engine
lifetimes as well as within one engine.

## Forcing requirement and falsifier

Two independent owners' first identities must differ. Issuing the maximum
sequence once must make the next allocation a typed refusal. Compile-fail
evidence rejects direct construction, `Default`, and caller-driven advance.
The live access-isolation test reconnects one relay, injects an event under the
superseded request's exact wire id, and proves that event is inert while the
successor request still accepts its own event.

Removing the authority namespace aliases two owners' first identity. Restoring
saturation reissues the maximum identity. Restoring public tuple fields or
`Default` makes the compile-fail evidence compile. Removing exact revision or
generation comparison admits the stale event.

## Validation

- `cargo test -p fava-query`
- `cargo test -p fava-subscriptions -p fava-subscriptions-standard -p fava-subscriptions-no-grouping -p fava-subscriptions-testkit`
- `cargo test -p fava-observe --test access_work_isolation`
- `cargo test -p fava-diagnostics --test ownership_graph`
- strict Clippy for the owning and implementation crates
- `cargo check --workspace --all-targets`

