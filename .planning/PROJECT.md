# Fava

## What This Is

Fava is an embeddable Nostr client library implemented in Rust with first-class Swift and Kotlin SDKs. It gives applications a small public surface for coherent live queries, durable write intents, signing, routing, publication, receipts, protocol services, diagnostics, and statically selected providers while leaving product state, presentation, policy, and UX to the application.

The project is the complete rewrite defined by the authoritative specifications under `docs/spec/`. Delivery spans the evidence foundation through native-product release qualification; milestone completion means every documented exit gate has passed through the public Fava API and the required independent witnesses.

## Core Value

Applications can rely on coherent live queries and durable writes with exact, bounded, failure-isolated lifecycle and evidence semantics across replaceable provider compositions.

## Requirements

### Validated

- ✓ An independent real-relay evidence lab can publish and query a genuinely signed event, hard-kill and restart the relay against the same data directory, query the event again, and preserve reconstructable wire/process evidence — M0
- ✓ The canary remains independent of Fava internals and fails enabled scenarios rather than silently skipping unavailable prerequisites — M0
- ✓ M1 delivers deterministic local semantic state across independent event-cache and write-store authorities, including replacement, deletion, expiry, source removal, stable query identity, bounded observation, and public-facade evidence
- ✓ M2-M3 deliver exact explicit and multi-relay live queries with verified admission, scoped EOSE/provenance, reconnect generations, cancellation, diagnostics, and bounded observation through real-relay evidence
- ✓ M4 delivers ordered reactive routing and meaning-preserving subscription planning with immediate progress, explicit bypass, typed shortfall, and public route preview
- ✓ M5-M6 deliver durable explicit and automatic-route publication with optimistic local visibility, exact receipts, cancellation, process-kill recovery, immediate partial delivery, and later route expansion under one receipt

### Active

- [ ] Deliver replaceable-event edits and independent protocol crates without expanding the universal workload model beyond live queries and write intents
- [ ] Qualify authentication, hostile-relay behavior, typed limits, overload, ambiguous handoff, give-up, and resource boundedness with attributable failures
- [ ] Deliver truthful persistent and ephemeral cache/service profiles, restart guarantees, NIP-05 and NIP-11 services, and durable write recovery
- [ ] Prove provider substitution across every major seam using public contracts, external implementations, shared conformance corpora, and no private bypasses
- [ ] Release equivalent Rust, Swift, and Kotlin product profiles through real native platform processes after all preceding milestone gates pass

### Out of Scope

- Copying outside implementation code, compatibility layers, or hidden behavior — Fava is implemented from the authoritative source documents
- Application-owned product state, domain models, navigation, presentation, ranking, recommendation, moderation, and account/secret-storage UX — Fava is a library, not an application framework
- Runtime plugin discovery or dynamic provider registries — provider selection is explicit static application composition
- A generic common crate or duplicate domain values — shared facts belong to their owner
- Global relay or network completeness claims — evidence remains exact and source-scoped
- Silent compatibility paths, hidden runtime feature flags, and provider-specific bypasses around public contracts — invalid or unsupported use is represented or refused explicitly
- Treating public-relay reconnaissance as deterministic release evidence — public relays are evidence-only; repeatable milestone gates use controlled real-relay environments

## Context

- The Fava documents under `docs/spec/` are authoritative.
- Authority order is: behavioral goals and objectives, architecture, TDD/BDD testing guide, implementation plan, then the partial Rust query-semantics refinement where it does not conflict with the complete authorities.
- The current checkout implements completed M0-M6 product slices. M7-M11 remain specified target work; Phase 7 semantic writes and capability composition is next.
- The implementation uses focused Rust owner, contract, provider, lifecycle-owner, and facade crates. The canary and external provider falsifiers are separate workspaces so they can act as independent witnesses.
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
| Name the library Fava | Gives the implementation one product identity | Accepted |
| Treat current `docs/spec/` Fava documents as authoritative | Keeps one normative source for behavior and architecture | Accepted |
| Deliver the complete M0–M11 rewrite as the project scope | Release qualification requires the full public behavior, provider substitution, and native SDK evidence chain | — Pending |
| Keep Fava a library with a small public surface | Applications retain product and presentation policy while Fava owns reusable Nostr machinery and lifecycle correctness | — Pending |
| Select providers through static application composition | Replaceability is explicit and testable without runtime plugin machinery or hidden defaults | — Pending |
| Use live queries and write intents as the two long-lived workload concepts | Keeps cancellation, recovery, routing, observation, and diagnostics coherent | — Pending |
| Treat M0-M6 as complete and M7 as the next milestone | Focused issue records, milestone commits, preserved canary bundles, current validation, and retroactive phase verification support the completed claims | Accepted |

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
*Last updated: 2026-08-21 after M0-M6 planning-state reconciliation*
