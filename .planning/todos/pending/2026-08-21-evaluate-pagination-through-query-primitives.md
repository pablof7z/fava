---
created: 2026-08-21T15:51:04.606Z
title: Evaluate pagination through query primitives
area: docs
severity: major
files:
  - /Users/pablofernandez/Downloads/pagination-and-underlying-primitives.md:1
  - docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:486
  - docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1584
  - docs/spec/ARCHITECTURE.md:547
  - docs/spec/ARCHITECTURE.md:1294
  - docs/spec/partial-spec-api-semantics.md:312
  - docs/internals/vocabulary.toml:188
---

## Problem

Fava deliberately leaves the public windowing API, cursor semantics, and
restart behavior open. The target behavior is already constrained: growable
historical acquisition must remain part of the existing live-query lifecycle,
remain separate from presentation, preserve application-authored bounds, and
never manufacture a global completeness claim.

The external draft proposes resolving that decision through two reusable query
primitives plus an optional pagination consumer. It is detailed enough to merit
focused architecture work, but it is not ready to become an authoritative
specification:

- it uses superseded project and crate names;
- it proposes new public/cross-crate vocabulary and a new crate, which require
  a separate vocabulary-approved architecture change;
- its one-shot acquisition path must be reconciled with the requirement that
  windowing use the same live-query lifecycle rather than a parallel history
  workload;
- it assumes acquired events can update the original observation through
  ordinary ingestion, including with a null or ephemeral event cache, but that
  ownership path has not been proved;
- its source identity, dynamic-route reconciliation, canonical-depth counting,
  same-timestamp handling, and restart semantics remain design hypotheses; and
- its cost budgets are provisional and need a falsifiable prototype rather
  than API commitment.

The source draft lives outside the repository. This capture preserves its goals
and design in current Fava terms so the intent survives independently of that
file without importing its obsolete or unapproved vocabulary.

## Solution

Treat this as a focused architecture investigation for `OPEN-001`, not as an
implementation plan or public API approval.

### Goals to preserve

1. Grow one observation's acquisition window backward while its live demand
   remains active.
2. Deliver historical events through the same `QuerySnapshot`/`EventRecord`
   view; do not create a second page-event stream.
3. Keep `Query` declarative and pagination-agnostic; pagination is acquisition
   state, not query selection or a decorator.
4. Preserve exact `since`, `until`, ordering, authority, access, and limit
   semantics authored by the application.
5. Retain per-source truth: EOSE, refusal, auth, silence, disconnect, and stale
   generations remain distinct and scoped to the exact request.
6. Never expose global `has_more`, global end-of-history, or authoritative
   emptiness.
7. Reuse public, generally useful query capabilities instead of private relay,
   planner, transport, cache, or observation hooks.
8. Keep retained state, queues, observations, FFI work, and same-timestamp
   recovery explicitly bounded.
9. Preserve planner replaceability: physical subscription grouping may change
   wire shape but not logical attribution, bounds, evidence, or failure truth.
10. Decide the public shape only after structural and empirical cost gates pass.

### Design hypothesis to evaluate

#### 1. Bounded current source evidence

An open `Observation` exposes bounded current evidence for each logical relay
request before physical subscription grouping. The minimum candidate facts are:

- exact query branch/part, relay, relay access, filter, and request generation;
- stored-response phase versus later live occurrences;
- actual EOSE or exact terminal refusal/failure/auth state;
- admitted occurrence count and newly attributed relay-provenance count; and
- oldest/newest `created_at` observed during that stored-response phase.

This must extend the existing `QueryEvidence`/`SourceEvidence` model rather than
silently introducing a synonym. It is current state, not a transcript; it must
not retain event bodies or unbounded event-id history. Causal terminal facts
cannot be lost even if intermediate counter updates coalesce.

#### 2. Exact-source settled query operation

Evaluate a public operation that runs an ordinary query against an exact set of
relay/access sources, bypassing automatic routing while reusing the ordinary
planner, transport, admission, cache/query-source, evidence, cancellation, and
failure machinery. It settles each supplied source only on EOSE or another
explicit terminal result. Timeout and silence remain unresolved outcomes.

The operation must be useful independently of pagination. Its relationship to
the owning live observation is the central question: prove whether it is a
valid leg of one growable live-query lifecycle or an impermissible parallel
history query in disguise.

#### 3. Optional window-growth owner

If the primitives qualify, evaluate an optional higher-level owner attached to
one existing `Observation`. It would own:

- requested historical depth below a fixed attachment-time top;
- one bounded position per exact logical relay request;
- coalesced concurrent depth increments;
- exact older acquisition attempts and their generation identity;
- dynamic source addition/removal reconciliation;
- bounded same-timestamp overlap/probing; and
- typed per-source stalls, refusals, and shortfalls.

The owner would not own events, query meaning, routing, planner grouping,
transport, admission, storage, evaluation, ordering, or a persistent opaque
cursor. A caller asks to increase desired depth; the result reports acquisition
progress, while the original observation remains the sole event-delivery API.

On restart, the application would reopen the query and request a desired
historical depth. Whether this is sufficient, and whether any resume token is
needed, remains an explicit decision.

### Questions that must be answered

1. Which existing owner distributes an admitted event from an exact acquisition
   leg into the already-open observation when the selected event cache retains
   nothing?
2. Can the acquisition leg be modeled as demand owned by the original
   observation, rather than a separate query whose cache side effects happen to
   refresh it?
3. What is the stable logical-source identity when routing, derived selections,
   access, or filter shape changes? Generation must identify attempts, not
   overwrite stable source identity.
4. Does counting current canonical `EventRecord`s provide a stable depth target
   under replacement, deletion, expiry, cache eviction, and derived-selection
   shrink, or is a different acquisition-window measure required?
5. How is progress distinguished from duplicate/provenance-only progress
   without retaining an unbounded boundary event-id set?
6. What bounded refusal is returned when more events share one timestamp than
   a relay will return and NIP-01 provides no event-id continuation bound?
7. When may EOSE plus a short page establish source exhaustion, given silent
   relay caps? The proposal conservatively requires an empty older page or the
   application `since` floor; this needs executable falsification.
8. Which evidence fields are always maintained, observed on demand, or reserved
   for diagnostics so ordinary queries do not pay pagination costs?
9. Which contract/implementation crates and public symbols are truly forced?
   Any additions require the repository's separate vocabulary-approval record.
10. Which milestone owns the architecture decision, prototype, and eventual
    cross-platform proof without claiming a later milestone before its gates?

### Prototype and decision gates

Write behavior and falsifiers first. Prototype only enough public query
evidence and exact acquisition to test the model. Compare at least:

1. baseline source status;
2. richer source evidence maintained but unobserved;
3. window owner attached and idle;
4. active backward acquisition; and
5. evidence observed from Rust and, when available, through FFI.

Exercise representative and adversarial schedules across logical-source count,
wire grouping, event volume, duplicate rate, observation count, dynamic routes,
reconnect generations, never-EOSE sources, slow consumers, repeated loads,
teardown, and same-timestamp saturation.

Reject or revise the design if it:

- scans all logical sources per event after attribution;
- copies filters/source identity per update;
- deep-copies full evidence into every event snapshot;
- serializes evidence across FFI when nobody requested it;
- retains history proportional to requests or events;
- uses unbounded retries or boundary ID sets;
- leaks tasks, queries, subscriptions, references, or descriptors on teardown;
- changes logical meaning under planner grouping;
- fails to update the original observation with a null event cache; or
- requires private access to query-owner, router, planner implementation,
  transport, or cache implementation internals.

The architecture decision may accept, reduce, or replace the hypothesis. Only
after the forcing requirements, ownership, lifecycle, boundedness, public
falsifiers, and measured costs are approved should authoritative specifications,
vocabulary, crates, or public API change.
