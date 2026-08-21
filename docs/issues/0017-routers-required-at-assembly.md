# An assembly that cannot route is refused at `build()`

**Status:** proposed (awaiting Pablo approval)
**Authority:** `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`
WRITE-027 (`:970`), WRITE-011 (`:808`), WRITE-012 (`:819`); `AGENTS.md:72`
("Make invalid use unrepresentable or refuse it before opening work").

## Problem

`FavaBuilder::build` validates five provider roles and never looks at the router
chain:

```rust
// crates/fava/src/lib.rs:368-381
let publication_selected =
    self.publisher.is_some() || self.delivery.is_some() || !self.signers.is_empty();
let publication = if publication_selected {
    let publisher = self.publisher.ok_or(BuildError::MissingPublisher)?;
    let delivery = self.delivery.ok_or(BuildError::MissingDeliveryPolicy)?;
    let transport = self.transport.clone().ok_or(BuildError::MissingPublicationTransport)?;
    let publication = Publication::new(…, self.routers.clone())
```

`Publication::new` (`crates/fava-publication/src/lib.rs:36-58`) checks only for
duplicate signer public keys. An assembly with zero routers builds cleanly, and
the failure surfaces much later as a `ReceiptOutcome::NoDestination` on a write
that never had a chance.

That conflates three different facts under one outcome, and the middle one is the
only one `NoDestination` was specified for. WRITE-027 (`:970-976`):

> If the selected automatic router chain settles with no destination, the write
> MUST expose a typed no-destination outcome naming the unresolved/absent route
> reasons that led there. The write MUST NOT silently disappear, substitute an
> unconfigured relay, or treat an indexer/discovery relay as a generic destination.

With no chain there are no route reasons to name, so the outcome degenerates into
exactly the silent disappearance that requirement forbids.

| condition | belongs |
|---|---|
| no router chain configured | `BuildError::MissingRouter` |
| chain configured, settles contributing nothing | `ReceiptOutcome::NoDestination`, naming the reasons |
| chain configured, still unresolved | receipt stays open (WRITE-027, final line) |

## Change

```rust
/// Publication selected without an automatic router chain.
#[error("Fava publication assembly requires one router")]
MissingRouter,
```

Refused unconditionally when `publication_selected` is true. There is no
explicit-routing-only mode and no opt-out: an application that publishes only to
relays it names is one whose router is an app-relay router pointing at those
relays. That is a configuration, not a special case, and the crate already exists
— `fava-router-app-relays`, used as `AppRelayRouter::new("app-relays", [relay])`
in `crates/fava/tests/automatic_publication.rs:57`.

WRITE-012 says an assembly "MAY configure several routers", which is permissive
about *how many* and about *which* policies are selected. It does not license an
assembly that cannot serve the routing mode WRITE-011 makes the ordinary one.

## Blast radius

Three assemblies select publication with zero routers and each gains one router:

- `crates/fava/tests/explicit_publication.rs`
- `apps/canary/src/publication.rs`
- `apps/canary/src/publication_child.rs`

Nine other zero-router assemblies select no publication providers, so
`publication_selected` is false and they are unaffected:
`crates/fava/tests/explicit_live.rs`, `local_source_merge.rs`, `multi_relay.rs`,
`observation_bounds.rs`, and `apps/canary/src/{grouping,hostile,live,local,multi}.rs`.

The router chain is engine-wide, not write-specific: the same chain serves query
routing (`crates/fava/src/lib.rs:196`) and write routing (`:224`). This issue
changes only the publication-selected condition; whether an equivalent gate
belongs on the query side is out of scope and is not assumed either way here.

## Exit gates

- Building with publication selected and zero routers fails with
  `BuildError::MissingRouter`.
- `ReceiptOutcome::NoDestination` remains reachable only through a configured
  chain that settles empty, and the receipt names the route reasons (WRITE-027).
- The three assemblies above build and pass with an explicit `AppRelayRouter`.
- `python3 tools/check_vocabulary.py` passes.
