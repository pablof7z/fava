# Phase 7: Semantic Writes and Capability Composition - Context

**Gathered:** 2026-08-21
**Status:** Ready for planning
**Mode:** User-directed discussion skip; authoritative-spec context

<domain>
## Phase Boundary

Deliver M7's replaceable-event-edit lifecycle and protocol-crate composition
through the public Rust `fava` facade. Protocol crates own event-kind meaning and
edit application; the write store owns durable custody and current
revision; publication owns generations, signing, routing, delivery, and
receipts. Native projections, profiles, and M8 hardening remain later phases.

</domain>

<decisions>
## Implementation Decisions

### Authoritative behavior
- `WriteIntent` gains the third authoritative accepted form: a bounded,
  persistable replaceable-event edit whose accepted custody freezes the author
  before revision; the edit itself carries no author.
- The edit's protocol crate owns its kind, optional addressable identifier,
  opaque change encoding, empty-state behavior, opposing operations, and
  application to qualified source state. The edit carries no version or stored
  inverse.
- First-value edits apply without a predecessor. Newer qualified source
  state reapplies every still-live edit while preserving unrelated source
  changes.
- One accepted operation, `WriteId`, and `ReceiptId` survive every
  revision generation. Exact generation, event, signer, route, relay
  session, and attempt identity make retired completions attributable and inert.
- The event cache never receives unpublished local revisions. Atomic
  replacement and retraction remain write-store query-source mutations.
- Protocol crates cannot sign, route, publish, deliver, own receipts, or depend
  on runtime, transport, store implementations, or standard routers.

### Capability proof
- Implement NIP-02 follow/unfollow and a separate bookmarks
  bookmark/unbookmark capability to challenge the shared contract.
- Both capabilities must pass one public conformance corpus, including first
  value, opposing operations, source change, deterministic composition, and bounds.
- Prove N+1 outside the workspace or selected product assembly: universal core
  behavior does not change, and raw arbitrary/future kinds remain constructible
  and publishable.

### Behavioral evidence
- Write observable public behavior first, confirm it fails before production
  implementation, and preserve a named deliberate-break failure for the
  generation or reapplication invariant.
- Include memory-store state-machine coverage, redb crash/reopen coverage,
  public-facade integration, dependency-negative compilation, and the external
  N+1 falsifier.
- Compilation is structural evidence only; completion requires behavioral
  revision, reapplication, stale-completion, query, receipt, and
  restart proof.

### the agent's Discretion
- Exact neutral value and applier trait names, internal module boundaries,
  observation wiring, generation token representation, and plan decomposition,
  provided all six architecture gates and the authoritative ownership split hold.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `fava-write` already owns `WriteIntent`, `WritePayload`, event construction,
  stable write/receipt ids, event values, publication evidence, and receipts.
- `fava-write-store` already owns atomic acceptance, exact signer/route/attempt
  mutations, recovery, and the independent query-source contract.
- Memory and redb providers already implement the same write-store contract and
  expose current revisions to ordinary queries.
- `fava-publication` already keeps signing and routing independent and correlates
  durable destination work through the ordinary receipt lifecycle.
- `fava-nip65` and the independent router crates demonstrate narrow
  protocol/provider crate boundaries and Bazel/Cargo metadata conventions.

### Established Patterns
- Semantic-owner crates define shared closed values; neutral contracts sit above
  replaceable implementations; defaults receive no private bypass.
- Mutable provider facts commit before observers see them; late completions carry
  exact operation/generation identity and cannot mutate newer state.
- Public-facade tests lead vertical slices. Provider corpora and external
  falsifiers prevent contracts from fitting only the standard implementation.

### Integration Points
- Extend `fava-write` values, `fava-write-store` mutations, both store providers,
  `fava-publication` orchestration, and `FavaBuilder` selected assembly.
- Add protocol crates and their Cargo/Bazel metadata without universal-core
  event-kind switches or dependencies on concrete providers.
- Drive source changes through the existing canonical query/cache/write-store
  path rather than a protocol-owned cache or hand-written store fixture.

</code_context>

<specifics>
## Specific Ideas

Use the implementation-plan canaries `replaceable-edit-first-value`,
`replaceable-edit-reapplication`, `replaceable-edit-opposing-operations`, and
`protocol-crate-n-plus-one` as the externally observable spine.

</specifics>

<deferred>
## Deferred Ideas

Swift/Kotlin projection, selected persistent/ephemeral profiles, authentication,
hostile boundary expansion, and release packaging remain Phases 8-11.

</deferred>
