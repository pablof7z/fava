# Fava rewrite repository rules

Clean-room implementation of Fava. `docs/spec/` is authoritative. Do not copy outside implementation code or add compatibility paths.

## Authority

1. `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` owns required behavior.
2. `docs/spec/ARCHITECTURE.md` owns responsibilities, state, lifecycles, replaceable boundaries.
3. `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` owns how behavior is specified and proved.
4. `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` owns delivery sequencing and exit gates.
5. `docs/spec/partial-spec-api-semantics.md` refines the Rust query-expression surface where it does not contradict the four above.

Where names or signatures differ, preserve behavior and ownership. Record a real contradiction in a focused local issue before implementing.

## Stage and interface posture

Early design, no public consumers. Break interfaces, architecture, design, expectations freely; never hold back for compatibility. No compat paths, deprecations, shims, aliases. Public API breaks are routine. A changed public declaration drops its Symbol Gate signature and has to be reviewed and signed again; the bar is the design decision, not compatibility.

## Communication

Laconic with Pablo: lead with result, verdict, or decision; fewest words the subject allows; end with the immediate next action only when one exists. No preamble, restatement, closing summary, or generic follow-up. Prose unless structure is necessary. Brevity never overrides rigor: preserve actionable distinctions, measured results, uncertainty, verified evidence; never claim absence without an empty search.

## Delivery workflow

One focused local issue, branch, validation set, commit series per slice. Complete M0 before claiming M1 or later complete; milestone names mean every documented exit gate passed. Observable behavior first, then executable evidence, then production code. Confirm new evidence fails before the implementation and under its named deliberate break. Build vertical slices through the public `fava` API; do not stabilize empty provider frameworks. Keep unfinished behavior out of public claims; link it to a local issue.

## Architecture gates

Every change passes all six, in proportion to scope:

1. Ownership: one authority for every mutable fact and lifecycle.
2. Dependency direction: domain values -> neutral contracts -> providers; universal owners use contracts, not standard implementations.
3. Replaceability: defaults have no private bypass; a competing implementation can use the public contract.
4. Failure isolation: blocking, failure, panic, cancellation, stale completions stay scoped and attributable.
5. Boundedness: externally influenced inputs, outputs, queues, observations, retained evidence have explicit bounds or typed refusal/shortfall.
6. Behavioral proof: public promises have falsifiable evidence at the owning component and, where required, through Rust, Swift, Kotlin, restart, or live platform paths.

NIP document numbering never defines crate boundaries. Crates follow coherent domain responsibilities: one crate may compose semantics defined across several NIPs, and one NIP may involve several owners. Never create, split, or name a crate merely because a NIP document exists.

Higher-level crates compose owning primitives and implement only their named domain semantics. They do not repeat event-id construction, signature verification, serialization, generic bounds, routing validation, storage admission, or lifecycle policy. A second check requires a genuinely distinct owned invariant and an explicit forcing requirement; “defense in depth” is not ownership.

## Architectural vocabulary

Closed by default. Prefer established Nostr vocabulary; a Fava term identifies the nearest Nostr concept and the exact Fava-owned distinction.

A new crate, public or cross-crate nominal type, provider contract, persisted entity, configuration concept, or lifecycle owner is an architecture change, and so is a synonym, wrapper, alternate representation, or adjective-qualified variant of an existing noun. Justifying one means naming the closest existing concept, the observable distinction, a counterexample, the owner and lifecycle, the forcing requirement, why existing state is insufficient, and an executable falsifier. These use a separate focused architecture change approved by Pablo; a feature change cannot approve its own.

Documentation describes the current model only. Replace superseded concepts completely; no migration narration, aliases, or rejected-design commentary in authoritative docs or code.

## Rust conventions

Code files: 500-line soft limit, 800-line hard limit. Applies to code only.

Keep the workload model to declarative event queries and write intents. Keep shared values in the crate that owns their meaning; no generic common bucket. Keep acquisition scope separate from result provenance authority. Never copy unpublished local events into the event cache; the write store is an independent query source. Make invalid use unrepresentable or refuse it before opening work. Use exact operation and generation identity for every late completion. No hidden runtime feature flags or silent compatibility behavior.

Nostr input is adversarial. Protocol decoders extract the required semantic positions and continue: ignore unknown tags, unused extra values, and repetitions unless the protocol assigns them meaning. Malformed optional or sibling material stays scoped and never erases valid decoded data. Do not turn harmless junk into whole-event failure or invent stricter protocol rules. Validate only invariants owned by the decoder's domain. Preserve foreign values for their owner; never parse or reject a generic protocol value merely to provide a convenient typed result.

Separate a replaceable contract from its implementation crate early, even with one implementation. Delaying the split couples the interface to that one mechanism; the contract crate forces an abstract, externally usable interface with a sound shape, not one that only fits how the first implementation happens to work. This is not stabilizing an empty provider framework: a contract carrying its first real implementation is not empty. Do not suggest collapsing or deferring a contract/impl split as simplification.