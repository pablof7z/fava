# NMP rewrite repository rules

This repository is a clean-room implementation of the NMP rewrite. The source
documents under `docs/spec/` are authoritative. Do not copy implementation
code or compatibility paths from the previous NMP repository.

## Authority

1. `docs/spec/FULL_NMP_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` owns required behavior.
2. `docs/spec/ARCHITECTURE.md` owns responsibilities, state, lifecycles, and replaceable boundaries.
3. `docs/spec/partial-spec-api-semantics.md` refines the Rust query-expression surface and source semantics.

When names or illustrative signatures differ, preserve the behavior and
ownership rule. Record any real contradiction in a focused local issue before
choosing an implementation.

## Delivery workflow

- One focused local issue, branch, validation set, and commit series per slice.
- Write observable behavior first, then executable evidence, then production code.
- Confirm new evidence fails before the implementation and under its named deliberate break.
- Build vertical slices through the public `nmp` API; do not stabilize empty provider frameworks.
- Keep unfinished behavior out of public claims and link it to a local issue.
- Do not add a Git remote or push this repository until Pablo explicitly authorizes it.

## Architecture gates

Every change must pass all six gates in proportion to its scope:

1. **Ownership:** one authority for every mutable fact and lifecycle.
2. **Dependency direction:** semantic values -> neutral contracts -> providers; universal owners use contracts, not standard implementations.
3. **Replaceability:** defaults have no private bypass and a competing implementation can use the public contract.
4. **Failure isolation:** blocking, failure, panic, cancellation, and stale completions remain scoped and attributable.
5. **Boundedness:** externally influenced inputs, outputs, queues, observations, and retained evidence have explicit bounds or typed refusal/shortfall.
6. **Behavioral proof:** public promises have falsifiable evidence at the owning component and, where required, through Rust, Swift, Kotlin, restart, or live platform paths.

## Rust conventions

- Keep the primary workload model to declarative event queries and write intents.
- Use semantic-owner crates for shared values; do not create a generic common bucket.
- Keep acquisition scope separate from result provenance authority.
- Never copy unpublished local events into the event cache; the write store is an independent query source.
- Make invalid use unrepresentable or refuse it before opening work.
- Use exact operation and generation identity for every late completion.
- No hidden runtime feature flags or silent compatibility behavior.

