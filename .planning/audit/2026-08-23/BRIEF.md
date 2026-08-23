# Fava full-workspace architecture deviation audit — shared brief

**Date:** 2026-08-23
**Mode:** READ-ONLY audit. Do not modify any production source, test, or spec file.
Your only writes are your own report file under `.planning/audit/2026-08-23/`.

## Why this audit exists

A confirmed systemic deviation was found in the live-query path: the thin `fava`
facade privately owns relay session establishment, subscription planning, `REQ`
handoff, reconnect, and cancellation — all of which `docs/spec/ARCHITECTURE.md`
assigns to `fava-observe`, `fava-transport`, and `fava-runtime`. Three public
falsifiers now fail deterministically. The existing test corpus did not catch it
because evidence was written to match the implementation instead of the authority.

Assume the same failure mode exists elsewhere. Your job is to find it.

## Authority order (higher wins)

1. `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` — required behavior
2. `docs/spec/ARCHITECTURE.md` — responsibilities, owned state, lifecycles, replaceable boundaries
3. `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` — how behavior must be proved
4. `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` — sequencing and milestone exit gates
5. `docs/spec/partial-spec-api-semantics.md` — Rust query surface refinement
6. `AGENTS.md` — repository gates, vocabulary policy, Rust conventions

Implementation code, `.planning/` records, completed issues, commit messages, and
existing tests are NOT authority. Where they disagree with the specs above, the
specs win and the disagreement is a finding.

## The six architecture gates (AGENTS.md)

1. **Ownership** — one authority for every mutable fact and lifecycle.
2. **Dependency direction** — domain values -> neutral contracts -> providers; universal owners use contracts, not standard implementations.
3. **Replaceability** — defaults have no private bypass; a competing implementation can use the public contract to achieve the same result.
4. **Failure isolation** — blocking, failure, panic, cancellation, and stale completions remain scoped and attributable.
5. **Boundedness** — externally influenced inputs, outputs, queues, observations, retained evidence have explicit bounds or typed refusal/shortfall.
6. **Behavioral proof** — public promises have falsifiable evidence at the owning component, through the real public path.

## Vocabulary policy (AGENTS.md)

`docs/internals/vocabulary.toml` is the source of truth. A new crate, public or
cross-crate nominal type, provider contract, persisted entity, configuration
concept, or **lifecycle owner** is a vocabulary change — and so is a synonym,
wrapper, alternate representation, or adjective-qualified variant of an existing
noun. Note: `tools/check_vocabulary.py` only scans `pub struct|enum|trait|type`,
so it is blind to `pub(crate)`, `pub(super)`, and private lifecycle nouns. Treat
that blindness as a known gate hole and look through it.

## What counts as a finding

For each finding you MUST supply all of:

- **id** — short kebab slug, unique within your report
- **gate** — which of the six gates (or `vocabulary`) it violates
- **severity** — `critical` (contradicts owned-state/lifecycle assignment or a
  named behavioral requirement), `major` (bypass, unbounded, unisolated, or
  unprovable public promise), `minor` (convention/cohesion)
- **authority** — exact quote + `docs/spec/FILE.md:LINE` proving what should be true
- **implementation** — exact `crates/.../file.rs:LINE` proving what is true instead
- **observable distinction** — how an application could tell the difference from
  outside, through the public API. If you cannot state one, downgrade or drop it.
- **proposed falsifier** — a concrete Rust test (name + 3-6 line sketch) that
  would fail today and pass after correct implementation
- **confidence** — `confirmed` (you read both sides and they contradict) or
  `suspected` (needs deeper work to settle)

Reject anything you cannot ground in both an authority line and a code line. Do
not pad the report. A short list of confirmed critical findings is far more
valuable than a long list of speculation. Explicitly say what you checked and
found conforming — absence claims must come from a search that actually ran.

## Known-good baseline (already found; do not re-report as new)

- `fava` facade owns relay session lifecycle, subscription planning, `REQ` handoff,
  reconnect, cancellation (`crates/fava/src/live.rs`, `relay.rs`, `routes.rs`)
- `fava::OpenedRelay` is an unapproved private lifecycle owner
- `Fava::observe` blocks the handle on relay establishment (violates QUERY-004)
- Equivalent observations do not share relay work
- Partial-open cancellation leaks an opened session
- `WebSocketTransport::open_session` has no Fava-owned establishment deadline
- `crates/fava-observe/src/lib.rs` lacks observation identity, registry, demand
  set, shared-work refcount, desired plan, route session, provider generation
- `impl QuerySource for Fava` starts a recursive `Fava::observe` from a fabricated
  empty EventCache snapshot
- `fava-runtime` and `fava-session` crates do not exist at all

You MAY report *new consequences* of these in your own area, and you MUST report
anything of the same shape you find elsewhere.

## Output

Write `.planning/audit/2026-08-23/<your-area-slug>.md` with:

```
# <Area> audit
## Scope checked
(files/specs actually read)
## Findings
### <id> — <severity> — <gate>
authority / implementation / observable distinction / proposed falsifier / confidence
## Conforming (verified, not merely unexamined)
## Open questions
```

Then return to the orchestrator ONLY: your report path, a count by severity, and
a max-10-line list of your critical findings as one-liners. Do not paste the report.
