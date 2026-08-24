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

Early design, no public consumers. Break interfaces, architecture, design, expectations freely; never hold back for compatibility. No compat paths, deprecations, shims, aliases. Public API breaks are routine — run `python3 tools/check_vocabulary.py` and update `docs/internals/vocabulary.toml` to the new truth. Vocabulary changes still need a focused architecture change approved by Pablo; the bar is the design decision, not compatibility.

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

## Architectural vocabulary

Closed by default. `docs/internals/vocabulary.toml` is the source of truth for concepts, public Rust symbols, specified public Rust symbols, crate names. Prefer established Nostr vocabulary; a Fava term identifies the nearest Nostr concept and the exact Fava-owned distinction.

A new crate, public or cross-crate nominal type, provider contract, persisted entity, configuration concept, or lifecycle owner is a vocabulary change. A synonym, wrapper, alternate representation, or adjective-qualified variant of an existing noun is also a vocabulary change. Approval requires: closest existing concept, observable distinction, counterexample, owner and lifecycle, forcing requirement, reason existing state is insufficient, executable falsifier. Vocabulary changes use a separate focused architecture change approved by Pablo; a feature change cannot approve its own new vocabulary.

Documentation describes the current model only. Replace superseded concepts completely; no migration narration, aliases, or rejected-design commentary in authoritative docs or code. Run `python3 tools/check_vocabulary.py` and its unit tests for every architectural or public-API change.

## Rust conventions

Code files: 500-line soft limit, 800-line hard limit. Applies to code only.

Keep the workload model to declarative event queries and write intents. Keep shared values in the crate that owns their meaning; no generic common bucket. Keep acquisition scope separate from result provenance authority. Never copy unpublished local events into the event cache; the write store is an independent query source. Make invalid use unrepresentable or refuse it before opening work. Use exact operation and generation identity for every late completion. No hidden runtime feature flags or silent compatibility behavior.

Separate a replaceable contract from its implementation crate early, even with one implementation. Delaying the split couples the interface to that one mechanism; the contract crate forces an abstract, externally usable interface with a sound shape, not one that only fits how the first implementation happens to work. This is not stabilizing an empty provider framework: a contract carrying its first real implementation is not empty. Do not suggest collapsing or deferring a contract/impl split as simplification.