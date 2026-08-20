# Fava rewrite repository rules

This repository is a clean-room implementation of Fava. The source documents
under `docs/spec/` are authoritative. Do not copy outside implementation code
or add compatibility paths.

## Authority

1. `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` owns required behavior.
2. `docs/spec/ARCHITECTURE.md` owns responsibilities, state, lifecycles, and replaceable boundaries.
3. `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` owns how behavior is specified and proved.
4. `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` owns delivery sequencing and milestone exit gates.
5. `docs/spec/partial-spec-api-semantics.md` refines the Rust query-expression surface and source semantics where it does not contradict the four complete authorities above.

When names or illustrative signatures differ, preserve the behavior and
ownership rule. Record any real contradiction in a focused local issue before
choosing an implementation.

## Communication

- Be laconic with Pablo: lead with the result, verdict, or decision; use the
  fewest words the subject permits; end with the immediate next action only
  when one exists.
- Do not add preambles, restate the request, append closing summaries, or offer
  generic follow-up. Use prose unless structure is itself necessary.
- Brevity never overrides rigor. Preserve actionable distinctions, measured
  results, uncertainty, and verified evidence; never claim absence without a
  search that returned empty.

## Delivery workflow

- One focused local issue, branch, validation set, and commit series per slice.
- Complete M0 before claiming M1 or later milestones complete; milestone names mean every documented exit gate has passed.
- Write observable behavior first, then executable evidence, then production code.
- Confirm new evidence fails before the implementation and under its named deliberate break.
- Build vertical slices through the public `fava` API; do not stabilize empty provider frameworks.
- Keep unfinished behavior out of public claims and link it to a local issue.
- Do not add a Git remote or push this repository until Pablo explicitly authorizes it.

## Architecture gates

Every change must pass all six gates in proportion to its scope:

1. **Ownership:** one authority for every mutable fact and lifecycle.
2. **Dependency direction:** domain values -> neutral contracts -> providers; universal owners use contracts, not standard implementations.
3. **Replaceability:** defaults have no private bypass and a competing implementation can use the public contract.
4. **Failure isolation:** blocking, failure, panic, cancellation, and stale completions remain scoped and attributable.
5. **Boundedness:** externally influenced inputs, outputs, queues, observations, and retained evidence have explicit bounds or typed refusal/shortfall.
6. **Behavioral proof:** public promises have falsifiable evidence at the owning component and, where required, through Rust, Swift, Kotlin, restart, or live platform paths.

## Architectural vocabulary

- Architectural vocabulary is closed by default. `docs/internals/vocabulary.toml` is the source of truth for concepts, public Rust symbols, specified public Rust symbols, and crate names.
- Prefer established Nostr vocabulary whenever it precisely names the concept. A Fava term must identify the nearest Nostr concept and the exact Fava-owned distinction.
- A new crate, public or cross-crate nominal type, provider contract, persisted entity, configuration concept, or lifecycle owner is a vocabulary change.
- A synonym, wrapper, alternate representation, or adjective-qualified variant of an existing noun is also a vocabulary change.
- Vocabulary approval requires the closest existing concept, observable distinction, counterexample, owner and lifecycle, forcing requirement, reason existing state is insufficient, and an executable falsifier.
- Vocabulary changes use a separate focused architecture change approved by Pablo. A feature change cannot approve its own new vocabulary.
- Documentation describes the current model only. Replace superseded concepts completely; do not retain migration narration, aliases, or rejected-design commentary in authoritative docs or code.
- Run `python3 tools/check_vocabulary.py` and its unit tests for every architectural or public-API change.

## Rust conventions

- Code files have a 500-line soft limit and an 800-line hard limit. The limits
  apply only to code, not documentation or other artifacts. Crossing 500 lines
  requires a concrete cohesion reason; no code file may cross 800 lines.

- Keep the primary workload model to declarative event queries and write intents.
- Keep shared values in the crate that owns their meaning; do not create a generic common bucket.
- Keep acquisition scope separate from result provenance authority.
- Never copy unpublished local events into the event cache; the write store is an independent query source.
- Make invalid use unrepresentable or refuse it before opening work.
- Use exact operation and generation identity for every late completion.
- No hidden runtime feature flags or silent compatibility behavior.
- Separate a replaceable contract from its implementation crate early, even with only one implementation. Delaying the split couples the interface to that one mechanism; the contract crate exists to force an abstract, externally usable interface with a sound shape rather than one that only fits how the first implementation happens to work. This is not "stabilizing an empty provider framework": a contract carrying its first real implementation is not empty. Do not suggest collapsing or deferring a contract/impl split as simplification.
