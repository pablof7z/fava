## Context

See `proposal.md` for motivation. `fava-query::identity` defines the opaque two-part token while `fava-observe` is its sole semantic owner and issuer. A relay `Slot` receives one token when its work begins and receives a fresh one when refusal or a physical reconnect supersedes that work; acquisition, request/close handoffs, the listen task, timers, reports, evidence, and diagnostics carry it back to the slot. A completion is accepted only when the token is still exact-current.

This token is not the transport-owned `RelaySessionGeneration`, which names a physical connection. It is not `PlanRevision`, which names a requested subscription plan, and it is not `OperationName`, which names one provider verb. Several distinct provider verbs execute under one token, so the current name and `generation` field labels misstate the scope. `RelayQueryEvidence` also currently calls it a transport connection generation.

## Goals / Non-Goals

**Goals:**

- Make the read-work fencing token's owner, lifetime, and distinction from session and plan identities evident at every public boundary.
- Make the intentional API break complete: type, issuer, exhaustion error, exports, carrier fields, accessors, documentation, tests, and diagnostic text use one vocabulary.
- Retain every existing validity, cancellation, uniqueness, and exhaustion guarantee exactly.
- Resubmit the changed public declarations through the repository Symbol Gate.

**Non-Goals:**

- Changing when an epoch is allocated, advanced, compared, cancelled, or exhausted.
- Changing `RelaySessionGeneration`, `PlanRevision`, `OperationName`, wire identifiers, persistence, or transport behavior.
- Adding a compatibility alias, conversion, adapter, feature flag, or dual-name period.

## Decisions

### Use `WorkEpoch` for the owner-authorized validity interval

`WorkEpoch`, `WorkEpochIssuer`, and `WorkEpochExhausted` replace the existing three public names. An epoch is the interval during which the observation owner authorizes a relay slot's work; its end makes every report from the prior interval inert. `Work` is deliberately neutral enough for `fava-runtime` to carry without acquiring an observe dependency, while the type's placement and callers keep its current read-side ownership explicit.

This is an architecture-vocabulary correction, not a new lifecycle:

- **Nearest existing concept:** `OperationGeneration`, the same opaque token.
- **Observable distinction and counterexample:** one slot uses the same token for `transport.acquire_session`, many transport sends, close/release work, its listen loop, and its admission timer. These are distinct `OperationName` values, so the token is not one operation's generation. A transport reconnect also creates a distinct `RelaySessionGeneration`; it causes a new work epoch but is not interchangeable with it.
- **Owner and lifecycle:** `fava-observe` mints the epoch, installs it in the relay slot, advances it when its current work is superseded, and compares it before state mutation. The runtime and providers only echo it.
- **Forcing requirement:** QUERY-010 and the architecture's exact owner/generation completion rule require a fresh owner-controlled fence for reopened demand and inert late completions.
- **Why the prior vocabulary is insufficient:** it conflates the fence with individual provider verbs and with independent session and revision generations, which has already produced a stale transport description in query evidence.
- **Executable falsifier:** the renamed issuer must still mint unequal values from independent authorities, refuse after its final sequence, reject direct construction/defaulting in compile-fail documentation, and leave a report tagged with the superseded epoch unable to mutate a current slot. Reintroducing an `OperationGeneration` public export or a `generation` carrier for this token must fail the Symbol Gate review and the focused source/API search.

`ProviderOperationId` is rejected because there is no per-call identity: two calls under one work epoch intentionally remain indistinguishable to the stale-work fence. `RelaySessionEpoch` is rejected because transport owns the physical session identity and its generation already has that name. `QueryOperationEpoch` is rejected because it retains the ambiguous `Operation` term and incorrectly describes the runtime carrier as a per-verb token.

### Rename carrier labels to `epoch`, preserve identity mechanics

Every field, parameter, local state name, accessor, error variant, user-visible error string, and comment that refers to this token becomes `epoch`. `authority()` and `sequence()` remain the token's opaque component accessors; its pair representation, ordering, display shape, non-cloneable issuer, checked atomic authority allocation, checked sequence allocation, and typed refusal are not changed.

This keeps the semantic boundary visible in composite records: `ProviderCompletion`, query relay evidence, relay diagnostics, owner reports, operation helpers, and slots carry a `WorkEpoch`. Transport structures retain their own `generation: RelaySessionGeneration` spelling. No generic `generation` rename is attempted outside the former `OperationGeneration` flow.

### Change every public surface atomically and re-sign it

The owning definition and exports in `fava-query` land together with all imports and re-exports in `fava-runtime`, `fava-observe`, `fava-diagnostics`, `fava-subscriptions`, and the `fava` facade. The runtime module and tests adopt the same name so callers cannot encounter a stale public spelling. Doctests, unit tests, integration tests, and Bazel-facing targets update in the same slice.

The changed declarations lose their prior Symbol Gate signature and require a fresh review/approval as `WorkEpoch` vocabulary attached to the existing observation/query-work concept; no additional term, provider contract, crate, persisted entity, or lifecycle owner is introduced.

### Repair current explanatory records without retaining the old API

Update `docs/spec/ARCHITECTURE.md`'s query-work ownership row and the focused issue records that describe this active identity (`0028`, `0039`, `0040`, and `0054`). They must call it a query work epoch, describe its relationship to `RelaySessionGeneration` accurately, and remove the old public spelling rather than narrating a rename. Historical audit snapshots remain historical evidence, not an API compatibility surface.

## Risks / Trade-offs

- **A partial rename leaves callers unable to see which identity they hold** → change definition-first and finish with whole-workspace searches for the old public symbols and old `generation` carriers, then compile every consumer.
- **A mechanical rename could alter fencing behavior** → retain and update the existing independent-authority, exhaustion, compile-fail, provider-completion, diagnostic, and stale-report evidence; run it once before and once after the rename with no changed assertions except vocabulary.
- **Concurrent transport/authentication work may touch shared relay documentation or tests** → rebase the focused rename after its owner changes and resolve only terminology conflicts; do not absorb any transport or authentication behavior.

## Migration Plan

1. Replace the identity family and its documentation in `fava-query`, then update all consumer imports, re-exports, field labels, and error text in the same focused change.
2. Update evidence, diagnostics, doctests, tests, and the current owner records; run a Symbol Gate review for the replacement public declarations.
3. Run focused crate tests, workspace checks, strict Clippy, formatting, the relevant Bazel targets, and whole-workspace stale-name searches.

There is no data migration, deployment sequencing, or compatibility period. Rollback is a full revert of the focused change; no released API depends on the old name.
