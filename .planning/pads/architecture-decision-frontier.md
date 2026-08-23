# Goal

Settle the architecture and interface questions left open by the Fava
live-query remediation, so the remaining phases can be built against decided
positions rather than provisional ones.

Six are open. Two await agent input and must not be ruled on yet. Four can be
decided now.


# Constraints

- `docs/spec/ARCHITECTURE.md` outranks anything this remediation produces,
  including `FROZEN-CONTRACTS.md`. `AGENTS.md` permits departing from an
  illustrative signature, never from an ownership rule.
- No shims, adapters, compatibility paths, or deprecation layers. Pablo's
  explicit instruction: delete and rebuild.
- A feature change cannot approve its own vocabulary. New cross-crate nominal
  types, lifecycle owners, persisted entities, and adjective-qualified variants
  of existing nouns require separate approval.
- Grouping batches unsent demand only. A subscription that has already executed
  is never rewritten. Confirmed against nmp, which measured the alternative at
  90% waste and 1-to-20 subscription growth at twenty growth steps.
- `GOALS.md:426` (QUERY-010): reopening dropped demand MUST use fresh request
  identity, so a late EOSE cannot settle a new request.
- Every public promise needs a falsifier that fails before the fix and under a
  named deliberate break.


# Working model

The live-query ownership inversion is a symptom. The cause is that three
execution owners named by the architecture — `fava-runtime`, `fava-session`,
`fava-auth` — were approved in vocabulary and never built, so their
responsibilities fell to whichever component held the call stack. `fava::OpenedRelay`
is what that looks like in source.

It survived six milestones because `.github/workflows/` ran two Python steps and
never ran `cargo test`. Every green result in the project's history was one a
person chose to run and then described in a document.

`fava-runtime` and the reshaped transport, subscription, query-evidence, and
diagnostics contracts now exist. The observation owner is rebuilding against
them. What remains open is where four boundaries sit.


# Settled

Sharing is a planner decision. The observation owner retains one demand per
`(observation, branch)` and never collapses on filter equality; the planner
merges. `GOALS.md:296` permits sharing but forbids erasing distinct source
authority, relay access, freshness, or presentation-relevant evidence merely
because filters are equal — and a merged filter is equal to none of its inputs,
so an owner-side `(relay, filter)` key cannot represent the result at all.

The refcount lives on the installed wire subscription, N-to-1, via the
attribution fan-out. One grouped EOSE settles every logical demand it serves.

Grouping batches unsent demand only, behind a relay-level admission window. A
subscription that has already executed is never rewritten — not on join, not on
withdrawal. Later demand attaches to an existing subscription or opens a new one
carrying its full filter; incumbent coverage is never subtracted.

`OpenedRelay`, `crates/fava/src/{relay,live,routes}.rs`, and
`Fava::next_subscription` are deleted outright. No adapter, wrapper, or
compatibility path replaces them.


# Open positions

Recommended positions on the four decisions that can be taken now. Each is the
working model until Pablo rules otherwise.

## Router acquisition

`impl QuerySource for Fava` is deleted. Routers receive a narrow, non-recursive
local-query service over current cache and write-store state, rather than
re-entering the engine. This resolves the fabricated empty initial snapshot, the
recursion, and the impossibility of a nested Fava labelling its own source role
honestly, which are one problem rather than three.

## WRITE-027 terminality

A total router refusal leaves the receipt non-terminal, carrying a typed
shortfall. Asynchronous route expansion means destinations legitimately arrive
later under the same receipt, so settling on first refusal forecloses a state the
model expects to reach.

## `ObserveError::Relay`

Deleted rather than reshaped. If the owner returns a coherent local observation
without waiting for relays, no relay condition can refuse an open; relay
problems are post-open evidence, which `RelayQueryEvidence` and `RelaySourceState`
now carry. `ObserveError` gains the shutdown variant QUERY-003 requires and
which it lacks today.

## Expiry sweep

`EventCache` gains the specified `maintain()`, driven by `fava-runtime`'s timers.
Admission-only sweeping leaves an idle Fava serving expired events indefinitely.

