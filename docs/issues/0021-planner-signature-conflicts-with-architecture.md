# 0021 — The implemented `SubscriptionPlanner` signature departs from ARCHITECTURE.md

**Status:** open, needs Pablo's decision
**Raised:** 2026-08-23, by the Wave 1 subscriptions implementer
**Blocks:** nothing today; the implemented shape is merged and green

## The conflict

`docs/spec/ARCHITECTURE.md:1483` specifies:

```rust
fn plan(
    &self,
    relay: &RelaySessionKey,
    demand: &[RelayDemand],
    constraints: &RelayReadConstraints,
) -> Result<SubscriptionPlan, SubscriptionPlanError>;

pub struct SubscriptionPlan {
    pub wire: Vec<PlannedSubscription>,
    pub attribution: SubscriptionAttribution,
    pub shortfalls: Vec<SubscriptionShortfall>,
}
```

`.planning/audit/2026-08-23/FROZEN-CONTRACTS.md` §2.2 specifies, and the merged
implementation carries, five parameters — adding the currently installed
subscriptions and a plan revision — and returns
`{relay, revision, open, retain, close, attribution, shortfalls}`.

The frozen document justified this from `ARCHITECTURE.md:1511`, which lists
"plan diff values" among the planner's owned meaning. The implementer's
objection is that this names owned *meaning*, not a signature, and that
`AGENTS.md` places `ARCHITECTURE.md` above any document produced by this work.

## Why it is not obviously a violation

`AGENTS.md` says: "When names or illustrative signatures differ, preserve the
behavior and ownership rule." That clause exists precisely so an implementation
may depart from an illustrative signature. So the question is not whether the
signature may change — it is whether **ownership** still holds.

## Why it might be one

The single-owner map at `ARCHITECTURE.md:2979` reads:

> Wire subscription plan | `fava-observe` owns desired plan; planner computes it | transport executes it

If `fava-observe` owns the desired plan, then diffing desired against installed
is the owner's work, and handing `installed` to the planner gives a replaceable
provider a view of owner-owned state. Against that, `:1511` does put "plan diff
values" inside the planner's owned meaning.

## The alternative that needs no departure

The merged implementation mints wire subscription ids as a **content digest** of
the canonical filter (FNV-1a, `fava-{16 hex}`), explicitly so retention works
without the planner holding state. That property makes the three-parameter shape
sufficient: the planner returns the desired `wire` set, and `fava-observe` diffs
by wire id — same id means the same subscription, so retain; absent means close;
new means open. The diff is then computed by the owner the ledger names, and the
planner stays a pure function of `(relay, demand, constraints)`.

## Decision required

1. **Amend `ARCHITECTURE.md`** to the five-parameter diff-returning shape, and
   move "owns desired plan" from `fava-observe` to the planner in the ledger; or
2. **Revert the contract to the specified three parameters**, returning the
   desired `wire` set, and let `fava-observe` diff by content-digest wire id.

Option 2 requires no authority-document change and keeps the ownership ledger
intact. Option 1 is what is merged today.

Either way the ledger row at `:2979` and the trait at `:1483` must end up saying
the same thing as the code. The failure this whole remediation exists to correct
was an implementation quietly disagreeing with the ownership ledger, so this must
not be resolved by leaving both texts standing.

## Related unresolved items from the same implementer

- **C5 has a hole.** No conformance rule relates `PlannedSubscription.serves` to
  `AttributedSubscription.serves`, so a plan can satisfy C1–C11 while its own two
  records of what a subscription serves contradict each other; ingest and
  settlement would then disagree. The implementation enforces consistency under
  C5 — a strengthening that rejects no correct plan. Should become an explicit
  rule.
- **`QueryBounds.since`/`until` are unreachable.** `fava_query::Query` has no time
  bounds, so `demand_for_query` can only ever produce `None`. Either `Query` gains
  them or two of three fields stay dead.
- **No `RelayReadConstraints` producer exists.** NIP-11 acquisition is deferred, so
  every production call site passes `unknown()` and declared-limit handling is
  proven only by unit falsifiers.
