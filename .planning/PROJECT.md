# Fava

## What This Is

Fava is the clean-room rewrite and successor of NMP: an embeddable Nostr client engine implemented in Rust with first-class Swift and Kotlin SDKs. It gives applications a small public surface for coherent live queries, durable write intents, signing, routing, publication, receipts, protocol services, diagnostics, and statically selected providers while leaving product state, presentation, policy, and UX to the application.

The project is the complete rewrite defined by the authoritative specifications under `docs/spec/`. Delivery spans the evidence foundation through native-product release qualification; milestone completion means every documented exit gate has passed through the public Fava API and the required independent witnesses.

## Core Value

Applications can rely on coherent live queries and durable writes with exact, bounded, failure-isolated lifecycle and evidence semantics across replaceable provider compositions.

## Requirements

### Validated

- ✓ An independent real-relay evidence lab can publish and query a genuinely signed event, hard-kill and restart the relay against the same data directory, query the event again, and preserve reconstructable wire/process evidence — M0
- ✓ The canary remains independent of Fava internals and fails enabled scenarios rather than silently skipping unavailable prerequisites — M0
- ✓ A narrow M1 tracer merges independent event-cache and write-store contributions into one current local view, preserves their separate authority, and exposes the path through the public facade — existing tracer only; not an M1 completion claim

### Active

- [ ] Complete M1 local semantic state: stable equivalent-query identity, deterministic deletion and expiry, full source-removal behavior, shared provider corpora, and every M1 exit gate
- [ ] Deliver explicit and multi-relay live queries with exact source-scoped evidence, bounded observation, cancellation, reconnect generations, removals, and real-relay proof
- [ ] Deliver ordered asynchronous routing and subscription planning where known work starts immediately, later route contributions expand work safely, and explicit routing bypasses automatic routers
- [ ] Deliver durable explicit and automatic-route publication with local visibility, signing, exact receipts, partial delivery, cancellation, recovery, and reattachment under one write identity
- [ ] Deliver semantic writes and independently composable protocol capabilities without expanding the universal workload model beyond live queries and write intents
- [ ] Qualify authentication, hostile-relay behavior, typed limits, overload, ambiguous handoff, give-up, and resource boundedness with attributable failures
- [ ] Deliver truthful persistent and ephemeral cache/service profiles, restart guarantees, NIP-05 and NIP-11 services, and durable write recovery
- [ ] Prove provider substitution across every major seam using public contracts, external implementations, shared conformance corpora, and no private bypasses
- [ ] Release equivalent Rust, Swift, and Kotlin product profiles through real native platform processes after all preceding milestone gates pass

### Out of Scope

- Copying implementation code, compatibility layers, or hidden behavior from the previous NMP repository — this is a clean-room rewrite from the authoritative source documents
- Application-owned product state, domain models, navigation, presentation, ranking, recommendation, moderation, and account/secret-storage UX — Fava is a library, not an application framework
- Runtime plugin discovery or dynamic provider registries — provider selection is explicit static application composition
- A generic common crate or duplicate semantic values — shared facts belong to their semantic owner
- Global relay or network completeness claims — evidence remains exact and source-scoped
- Silent compatibility paths, hidden runtime feature flags, and provider-specific bypasses around public contracts — invalid or unsupported use is represented or refused explicitly
- Treating public-relay reconnaissance as deterministic release evidence — public relays are evidence-only; repeatable milestone gates use controlled real-relay environments

## Context

- The project began as a from-scratch rewrite of NMP and is now named Fava. The supplied NMP documents are source/reference material; the current Fava documents under `docs/spec/` are authoritative.
- Authority order is: behavioral goals and objectives, architecture, TDD/BDD testing guide, implementation plan, then the partial Rust query-semantics refinement where it does not conflict with the complete authorities.
- The current checkout contains a completed M0 real-relay evidence foundation and an intentionally narrow M1 local-source tracer. M1 is incomplete; M2–M11 remain specified target work.
- The implementation uses focused Rust semantic-owner, contract, provider, lifecycle-owner, and facade crates. The canary and external provider falsifiers are separate workspaces so they can act as independent witnesses.
- The repository is actively carrying the NMP-to-Fava rename. Existing source and build changes are user-owned work and must be preserved; planning work must not reset, absorb, or silently reinterpret them.
- Behavior features preserve app-visible meaning. Executable owner tests drive implementation. Canary and native capstones prove additional public behavior only when they exercise real boundaries.
- The five later product decisions left open by the normative specification—windowing, partial-handoff cancellation, outage backfill, full delivery history, and the recommended persistent event-cache profile—remain decisions for their owning milestones rather than initialization guesses.

## Constraints

- **Behavioral authority**: `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` owns required behavior — implementation and planning must not weaken its distinctions
- **Architecture authority**: `docs/spec/ARCHITECTURE.md` owns responsibilities, state, lifecycles, dependency direction, and replaceable boundaries — each mutable fact and lifecycle has one owner
- **Evidence authority**: `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` owns TDD, BDD, mutation, and evidence discipline — new evidence must fail before implementation and under its named deliberate break
- **Sequencing authority**: `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` owns M0–M11 sequencing and exit gates — milestone names are earned only when every gate passes
- **Architecture gates**: every slice must satisfy ownership, dependency direction, replaceability, failure isolation, boundedness, and behavioral proof in proportion to scope
- **Vertical delivery**: build complete behavior through the public `fava` API before stabilizing generalized provider frameworks
- **Workload model**: keep the primary public workload model to declarative live event queries and durable write intents — supporting operations must not become parallel workload systems
- **Source authority**: acquisition scope and result-provenance authority remain separate; unpublished local events remain in the write store and never pollute the event cache
- **Identity**: late completion, cancellation, reconnect, retry, and recovery use exact operation and generation identity
- **Rust size**: code files have a 500-line soft limit and an 800-line hard limit — crossing 500 requires a concrete cohesion reason
- **Repository hygiene**: one focused local issue, branch, validation set, and commit series per slice; do not add a remote or push without explicit authorization

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Name the clean-room NMP successor Fava | Distinguishes the new implementation while preserving the original product intent | — Pending |
| Treat current `docs/spec/` Fava documents as authoritative and supplied NMP documents as source/reference | The checkout contains substantively updated Fava specifications and an active rename | — Pending |
| Deliver the complete M0–M11 rewrite as the project scope | Release qualification requires the full public behavior, provider substitution, and native SDK evidence chain | — Pending |
| Keep Fava a library with a small public surface | Applications retain product and presentation policy while Fava owns reusable Nostr machinery and lifecycle correctness | — Pending |
| Select providers through static application composition | Replaceability is explicit and testable without runtime plugin machinery or hidden defaults | — Pending |
| Use live queries and write intents as the two long-lived workload concepts | Keeps cancellation, recovery, routing, observation, and diagnostics coherent | — Pending |
| Treat M0 as complete and M1 as the next incomplete milestone | Current evidence passes M0; the local-source tracer proves only a subset of M1 | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `$gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `$gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-08-20 after initialization*
