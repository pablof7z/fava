# nmp vs fava — subscription grouping: execution model and merge semantics

**Date:** 2026-08-23
**Subject:** `/Users/pablo/Work/nmp` `crates/nmp-router` + `crates/nmp/src/{runtime,core}` vs
`/Users/pablo/Work/fava` `crates/fava-subscriptions-standard` against the contract in
`crates/fava-subscriptions`.
**Nature:** semantics comparison. Nothing was modified in either repository.

> **Read Part A first.** It is not about the merge rule. It is about *when* merging is
> allowed to happen at all, and it determines whether fava's contract shape is correct.
> Part B is the merge predicate. Part C is the ranked change list.

---

# Part A — The execution boundary and the debounce

## A.0 The one-sentence difference

**nmp never rewrites a subscription that has already been sent.** Grouping happens once,
over a small time-windowed cohort of demand that has *not yet reached the wire*. Once a
REQ is admitted it is immutable: later demand either attaches to it (if already covered)
or opens an additional REQ alongside it. It closes only when its last owner is gone.

**fava's planner recomputes a desired wire set for the whole relay on every call and
diffs it against what is installed.** A new demand that merges with an already-running
subscription changes that subscription's filter bytes, which changes its content-digest
id, which produces a `close` of the running subscription and an `open` of a replacement.
The relay re-serves the entire window for demand that was already settled.

That is a structural divergence, not a rule-level one, and it invalidates the shape of
the current contract rather than any single line of `grouping.rs`.

## A.1 nmp's pipeline

`docs/internals/subscriptions/identity-grouping-and-limits.md` §1 (`:46-96`):

```
app declares a query
   ↓
resolver   → demand atoms + immediate local projection
   ↓
pending admission (10ms from first uncovered app demand)
   ↓
router     → route and MERGE only that unsent cohort, per relay/context/source
   ↓
admission  → append immutable REQs, or attach to exact active coverage
   ↓
transport  → one REQ frame per filter, on a socket
```

And the paragraph that answers the coordinator's question directly:

> **App admission and global replanning are different transitions.** Opening an
> observation reads only that observation's canonical local projection. If an active REQ
> already carries its exact `CoverageKey`, the new observation simply attaches to it.
> Otherwise the first uncovered app request arms a 10ms, first-arrival-anchored deadline.
> More compatible observations may join that pending cohort without extending the
> deadline. When it expires, NMP routes the cohort and coalesces it inside each
> `(RelaySessionKey, SourceAuthority)` partition.
>
> **REQs become immutable when admitted.** A later cohort never widens, narrows, renames,
> or closes a still-useful incumbent; it opens additional REQs for the coverage still
> missing. Withdrawal closes a shared REQ only after its last active absorbed key is gone.
> **This avoids paying the relay to rerun a broad query merely because another screen
> element appeared.**

There is a second, separate transition, and it is deliberately rare:

> `Router::compile` still performs a whole-demand replan when the world actually
> invalidates routing — for example an active-account reroot, route-directory change, or
> relay-budget change. Those transitions may genuinely move existing demand between
> sessions.

So nmp has **two** transitions with **different** rules. fava has one, and it behaves
like nmp's rare one on every single demand change.

## A.2 The debounce — where, how wide, what arms it

**Constant** — `crates/nmp/src/runtime/mod.rs:137`:

```rust
const WIRE_ADMISSION_WINDOW: Duration = Duration::from_millis(10);
```

**State and arming** — `crates/nmp/src/runtime/mod.rs:4008-4032`:

```rust
#[derive(Default)]
struct WireAdmissionState {
    deadline: Option<Instant>,
}

impl WireAdmissionState {
    fn arm(&mut self, now: Instant) {
        if self.deadline.is_none() {
            self.deadline = Some(now + WIRE_ADMISSION_WINDOW);
        }
    }
    fn next_deadline(&self) -> Option<Instant> { self.deadline }
    fn take_due(&mut self, now: Instant) -> bool {
        if !self.deadline.is_some_and(|deadline| deadline <= now) { return false; }
        self.deadline = None;
        true
    }
}
```

The `if self.deadline.is_none()` guard is the whole semantics: **first-arrival-anchored,
never sliding.** A burst of a hundred queries arriving over 9ms all flush together at
`t0 + 10ms`; a query arriving at `t0 + 11ms` arms a fresh window of its own and forms a
new cohort. A demand arriving *after* the flush never joins the earlier cohort's REQs by
merging — it can only attach to one if the running filter already covers it exactly
(§A.4), otherwise it gets its own REQ.

**Scope.** The *timer* is one per engine runtime (a single `WireAdmissionState` in
`DispatchRuntime`), driven by the runtime's deadline-armed `recv_timeout` loop
(`runtime/mod.rs:5-11`). The *grouping locus* is not global — design note §11.5
(`:1287-1290`):

> The timer is not a sliding debounce, does not sit in the resolver, and does not postpone
> cache delivery. Its grouping locus is the router's existing per-relay/context/source
> partition, **so two queries are never combined merely because they happened to arrive
> together.** Cancellation before the deadline removes the pending demand without producing
> a REQ.

Fixed, not adaptive. §11.5 also states the three facts the split bought (`:1278-1286`):

> 1. Project the new observation from cache immediately and read no sibling.
> 2. If exact active coverage already exists, attach locally with no compile.
> 3. Otherwise hold only the unsent relay demand for 10ms from its first arrival,
>    route/coalesce that cohort once, and append immutable REQs.

and the honest limit of the mechanism (`:1291-1295`):

> It does **not** collect derived values revealed by relay events after an observation is
> already running. Those can be spaced by RTTs or minutes, and an interactive admission
> window cannot cover them.

**Where the cohort lives.** One flat map in the reducer, not per relay —
`pending_wire_atoms: BTreeMap<DemandKey, ContextualAtom>`
(`crates/nmp/src/core/mod.rs:2018`). The partitioning is applied later, by the router.

**Arming is an effect, and the no-extend rule is in its doc**
(`crates/nmp/src/core/mod.rs:1286-1288`):

> Arm one first-arrival-anchored wire-admission deadline. **Repeated arms while a deadline
> is pending do not extend it.**

Emitted only when `wire_admission_needed()` — i.e. `!pending_wire_atoms.is_empty()`
(`core/query.rs:1122-1124`) — from observation open, history open, and *after withdrawal*
(a close can free budget for a demand previously refused).

**Who joins the cohort** — `admission_incomplete`
(`crates/nmp-router/src/ownership/instrumentation.rs:136-142`):

```rust
/// Whether one exact demand still has relay work or routing shortfall
/// eligible for a later pending-only admission cohort.
pub fn admission_incomplete(&self, demand: DemandKey) -> bool {
    !self.physically_covers(demand)
        || self.prev_plan.limited_demands.contains(&demand)
        || self.uncovered_by_demand.contains_key(&demand)
}
```

Note the second and third disjuncts: **a demand refused for budget stays pending and is
retried in a later window** (`reconcile_pending_wire_cohort`, `core/query.rs:1586-1600`
removes flushed keys *unless* they are still in `plan().limited_demands`). That is the
graceful-degradation loop fava's `SubscriptionShortfall` needs an equivalent of — a
shortfall today is reported and then forgotten until something else triggers a replan.

**The flush transition, and the single best statement of the whole model**
(`crates/nmp/src/core/query.rs:382-390`):

> Compile exactly the currently-uncovered logical demand as one pending cohort. **Existing
> plan requests are coverage inputs, never merge or identity candidates, so this transition
> cannot rewrite them.**

Its call into the router passes `&BTreeSet::new()` as the `replacements` argument
(`query.rs:415-420`) — **the cohort path structurally cannot emit a replacement
transition.**

Tests: `crates/nmp/src/core/admission_tests/cohort.rs` —
`cache_seed_is_immediate_while_wire_execution_waits_for_admission_flush` (`:6`),
`later_uncovered_demand_opens_a_second_req_without_replacing_the_running_one` (`:89`),
`duplicate_running_demand_attaches_without_compile_or_sibling_projection` (`:123`,
asserts `router_compiles == 0` and no `ArmWireAdmission`),
`cancelling_a_pending_observation_before_flush_sends_nothing` (`:176`).
End to end over a real socket: `crates/nmp/tests/runtime_integration.rs:532-535`.

## A.3 What marks a subscription "executed"

The freeze happens at **admission**, before the bytes reach the socket — not at EOSE and
not at relay acknowledgement. `crates/nmp-router/src/admission.rs:1-5`, the module doc:

> A cohort is compiled in an empty incumbent namespace, then appended to the running plan.
> **Existing requests are therefore candidates for exact coverage reuse, never candidates
> for widening or identity reassignment.**

And `Router::admit`'s own doc (`admission.rs:58-60`):

> Admit one already-routed logical cohort **without rewriting running requests**. Exact
> coverage already present in the plan is a no-op.

The enforcement is mechanical and worth copying: `admit` **detaches the entire running
index set**, compiles the pending cohort against an *empty* incumbent namespace, then
re-attaches (`admission.rs:100-179`):

```rust
// Reuse the one canonical routing/coalescing compiler with an empty
// incumbent view. The running indexes are detached wholesale, not
// rebuilt: candidate compilation can visit only `pending`, while the
// monotonic token counter remains shared and therefore never rewinds.
let running_plan = std::mem::take(&mut self.prev_plan);
...
let _ = self.compile(&pending, facts, CompileBudget::with_relay_cap(usize::MAX));
```

The merge step therefore *cannot* see the running set. It is structurally impossible for
it to widen an incumbent. That is a much stronger guarantee than a policy comment.

`pending` itself is the cohort minus anything already served (`admission.rs:67-77`): an
atom is pending only if it is not an active demand, or has no request edge, or is
`limited`, or is currently uncovered.

## A.4 What happens to a demand that arrives after the freeze

Three outcomes, tried in order.

**(1) Exact `DemandKey` already active → pure refcount bump, no compile.**
`Router::activate` (`admission.rs:29-56`):

> Reactivate one exact logical owner already covered by an immutable physical request.
> Reattachment updates only that demand's retained request edges; **it never recompiles or
> mutates the request itself.**

**(2) An already-sent filter covers it → attach locally, no REQ.** Two index lookups
(`admission.rs:192-200` → `admission/metadata.rs`):
- `attach_exact_request_metadata` — hit on `request_by_exact_filter`, keyed
  `(session, source, filter)`. Byte-identical filter ⇒ hit.
- `attach_physically_covered_request_metadata` → `physical_filter_covers`
  (`metadata.rs:30-62`), a real containment test over kinds/authors/ids/tags/since/until.
  Its doc: *"Limited requests stay exact-only: their result-count boundary is not a set
  axis and cannot safely be reconstructed for a later owner."*

This is **subsumption, done at admission time rather than in the merge rule** — and it is
the answer to the "subsumption vs union" question in the original brief. nmp does not
subsume inside `try_merge`; it subsumes when deciding whether a new demand needs a REQ at
all. The candidate `WireReq` is consumed and never emitted; the incumbent's
`owner_demands` / `coverage_claims` / `provenance` are extended in place and a
`RequestMetadataUpdate` is issued. Its doc (`ownership.rs:185-189`):

> Metadata attached locally to one byte-identical incumbent request. **The transport
> request remains immutable.** Core consumes this transition to extend only the current
> execution generation and durable claim ownership.

**(3) Neither → a brand-new REQ with a brand-new token, alongside the incumbent.** No
close, no rename, no widening of anything already live.

**And the new REQ carries its FULL filter — nmp deliberately does not subtract the
incumbent's coverage.** `crates/nmp-router/tests/admission/coverage_behavior.rs:121-125`:

> `"until residual subtraction is proven safe, executing the full later filter prevents
> underfetch"`

The residual optimisation exists as a deliberately-disabled test
(`coverage_behavior.rs:133`):

```rust
#[ignore = "known violation #1341: representable incumbent residual is not yet subtracted"]
fn representable_running_filter_residual_is_executed_and_owned_as_one_lifecycle() {
```

This is worth copying wholesale. The tempting move — "the incumbent already covers author
A, so the new REQ only needs B" — is an *under*-fetch risk dressed as an optimisation, and
nmp declined it until it can be proven. Over-fetch is free; the local re-filter absorbs it.

One partial-coverage nuance: a demand already served on *some* of its required sessions is
dropped only from those sessions' candidates (`admission.rs:216-236`), and *"the retained
filter may be wider than its retained keys, which is safe under the router's existing
local-refilter contract."*

## A.5 Is a running subscription ever modified? No — and the reasoning is the prize

nmp never re-sends a REQ under an existing subscription id to change its filters. NIP-01
would allow it (a re-REQ on an existing id replaces the filter), and nmp used to rely on
it. It stopped, twice, for two different reasons.

**First, at the identity level (#899).** `crates/nmp-router/src/wire_id.rs:1-9`:

> Wire ids are allocated opaque tokens, not functions of the filter. **Every byte-identical
> filter keeps its token. Every byte-changed filter receives a fresh token**; structural
> matching identifies only the predecessor that Core must retire after the fresh request is
> locally accepted (#774). **It never authorizes an in-place overwrite.**

`crates/nmp-router/src/plan.rs:234-252`:

> A single wire operation. `Req` opens the named subscription; Router-planned byte changes
> use a fresh id and a typed replacement transition, while only exact zero-diff retains an
> existing id. `Close` withdraws a sub-id after the owning transition reaches its commit
> edge.
>
> Raw canonical deltas list `Close` before `Req`; a `CompileOutcome` separately names
> byte-changing replacement pairs so EngineCore **withholds each predecessor close until its
> fresh-id successor reaches the exact commit edge.**

Design note §4.1 (`:289-301`):

> An exact byte-identical request keeps its allocated id and emits no wire work. When the
> filter bytes change, structural component matching identifies the predecessor but the
> successor receives a fresh id. `CompileOutcome` records the typed predecessor/successor
> transition. **EngineCore offers the successor REQ first and retires the predecessor only
> after the exact successor handoff is accepted.** A local refusal keeps the predecessor
> live and owns one retry.

So even in the rare whole-demand replan, the wire model is **open-before-close with a
typed predecessor**, never overwrite-in-place. The reason an overwrite is unacceptable is
attribution: a re-REQ on the same id makes the following EOSE generation-ambiguous, and
`discard_sub`'s doc states the failure exactly
(`crates/nmp/src/core/attribution/completion.rs:149-156`):

> Were a discarded string ever re-registered, the FRESH FIFO underneath it would be popped
> by a straggler EOSE belonging to the request that was closed — crediting durable coverage
> for a request the relay has not finished serving.

**Second, at the admission level (#1340/#1341/#1343).** Even open-before-close is too
expensive to do casually, and that is what the 10ms cohort exists to avoid. §11.5
(`:1268-1276`):

> The common interactive burst is not derived growth. A render pass asks for a set of
> independent live queries — for example many kind:0 avatar profiles — one call at a time.
> Sending each call immediately creates one narrow REQ before the next call exists.
> **Replanning all live demand after each call groups them, but repeatedly rewrites
> already-running subscriptions** and, before #1340, also reread every sibling's canonical
> rows.

nmp *measured* the cost of the rewrite-in-place design it abandoned. §11.1
(`:1044-1060`), `overwrite` (one sub-id, cumulative re-REQ) vs `delta` (append a sibling):

| growth schedule | overwrite | delta | saved | concurrent subs |
|---|---|---|---|---|
| `9` — one step | 128 KB | 128 KB | **0.6%** | 1 → 1 |
| `5,1,3` | 286 KB | 128 KB | **55%** | 1 → 3 |
| `1` × 20 | 2.90 MB | 285 KB | **90%** | 1 → **20** |

> **The waste is real and it is quadratic in the number of GROWTH STEPS.** `overwrite`
> serves `E·(v₁ + v₁₊₂ + …)`; `delta` serves `E·n`.

§11.6 records the verdict — **#933 stays open, unbuilt.** Note the shape of the trade: the
overwrite is nearly free for a *single* growth step and catastrophic for twenty. An
interactive render pass that adds one avatar query at a time is the twenty-step case.

**Issue #774 is effectively a ready-made spec** for the transition, and fava can adopt its
required contract close to verbatim:

> - A byte-changed relay filter always receives a fresh, never-reused `SubId`.
>   Zero-difference recompilation retains the existing id; no compatibility path preserves
>   one-difference reuse.
> - Ordinary replacement is accepted-open-before-close. NMP dispatches the fresh request
>   first and sends CLOSE for the old id only after the exact new attempt is locally
>   accepted by the current transport generation.
> - If the new attempt is locally refused, stale, or cancelled, the old accepted request
>   remains live and no CLOSE is sent for it.
> - A late EOSE/CLOSED for the retired id is inert. EOSE for the fresh id can credit only
>   the fresh filter's exact claims.
> - Do not implement arbitrary FIFO truncation, EOSE-gated serialization, a same-id
>   generation guess, or a compatibility adapter.

and its stated motivation, which is precisely the ambiguity a shared id creates:

> If both versions were accepted and the relay later sends EOSE, the wire frame names only
> the shared subscription id; **it cannot say which filter generation it completed.** …
> Dropping an old snapshot is not safe: the eventual EOSE could belong to it. Waiting for
> EOSE before sending the new demand is also not safe because a silent relay can block that
> demand forever.

Note that fava's plan value has no vocabulary for open-before-close at all: `open` and
`close` are unordered buckets and nothing names a `(predecessor, successor)` pair. If C1
leaves any replacement path in place, that pairing has to be expressible.

The sibling test states the churn cost in one line
(`crates/nmp-router/tests/wire_id_allocation.rs:287`,
`withdrawing_a_sibling_does_not_move_the_survivors_sub_id`):

> `diff_plans` would emit a Close plus a Req for a BYTE-IDENTICAL filter — **the relay
> re-serves the whole window for nothing and the attribution FIFO splits across two
> identities, orphaning outstanding snapshots.**

That sentence is the exact failure fava's content-digest ids produce on every group
membership change. See C1.

## A.6 Withdrawal — refcount, never narrowing

`crates/nmp-router/src/withdrawal.rs:78-81`, `Router::withdraw`:

> Consume exact resolver-style closes without inspecting any sibling demand. **A physical
> request closes only when its incremental active owner count reaches zero.**

The refcount is `Router::active_by_request: BTreeMap<RequestId, usize>`, decremented per
withdrawn `DemandKey` via the `requests_by_demand` edge (`withdrawal.rs:117-135`):

```rust
*count = count.checked_sub(1)
    .expect("physical request active-owner count cannot underflow");
if *count == 0 { closing.insert(request); continue; }
```

Above zero, the outcome is a `RequestMetadataRemoval`, whose doc is unambiguous
(`ownership.rs:194-198`):

> **The wire filter and subscription id do not change.** Core prunes the exact current
> pending or accepted claim and owner membership, along with the future/reconnect metadata,
> while leaving older overwritten generations untouched.

So: a grouped subscription that loses one of its two members keeps running **unchanged and
over-broad**, with the surplus discarded by the local re-filter
(`crates/nmp-router/src/deliver.rs`). Narrowing is never worth a REQ.

**The refcount is two layers, and fava has only one.** Above the router, `EngineCore`
keeps `wire_owner_counts: BTreeMap<DemandKey, (ContextualAtom, usize)>`, so several
observations sharing one logical selection collapse to a single router-visible demand —
*"only genuinely ownerless atoms reach the router"* (`core/query.rs:1476-1479`). Below it,
`active_by_request` counts physical owners. fava's `DemandId` is `(ObservationId,
QueryBranchId)`, so two observations of the same query are two distinct demands all the way
down; the collapsing happens implicitly in `group()`'s exact-dedup instead. That is
workable, but it means fava has no place to answer "is this selection still wanted by
anyone" without re-running the planner.

Scale test: `crates/nmp-router/tests/admission/scale_withdrawal.rs:6` —
10 000 withdrawals across <50 physical requests emit exactly one CLOSE per request, at its
last owner, touching zero incumbent diagnostics. The design note's
retraction ruling (§8.1b, `:675-692`) is the one place a close-and-reopen is mandated, and
it is scoped to *incorrectness*, not to surplus:

> When a newer answer invalidates what we previously held, **close whatever is now known to
> be incorrect and open it again with the right values.** Correctness first; do not try to
> preserve a subscription whose demand has been contradicted. […] Demand that is genuinely
> gone still closes directly.

## A.7 Does the grouping function need the installed set? — the interface answer

**The merge step: no. Emphatically no, and nmp enforces it structurally.**

Design note §3.2 (`:158-172`):

> During ordinary app admission, coalescing runs over the **pending cohort**, not per query
> and not over already-sent requests. Two unrelated observations that arrive in the same
> cohort and land in the same relay/context/source partition can therefore combine.
> **Existing REQs participate only through their exact absorbed coverage keys: they can
> satisfy a new observation, but are never merge candidates.** A true routing invalidation
> still uses the whole-demand compiler described in §1.

Enforcement is the `std::mem::take` of every running index before
`self.compile(&pending, ...)` in `admission.rs:100-145`. `RuleRegistry::coalesce_with` is
a pure function of the cohort.

**The admission step: yes, for exactly three things, none of which is merging.**

1. **Attach** — `request_by_exact_filter` and `physical_claims_by_request` decide whether a
   candidate REQ is already covered and can be dropped in favour of extending an incumbent
   (`admission/metadata.rs:239-284`).
2. **Budget** — `apply_residual_budget` (`admission/preview.rs`) counts *existing* requests
   against the relay's declared `max_subscriptions` so the cohort only gets the residual.
3. **Refcount / withdrawal** — `active_by_request`, `requests_by_demand` (`withdrawal.rs`).

There is also a **read-only twin** worth stealing: `Router::preview_admission`
(`admission/preview.rs:171-181`) evaluates a cohort against the running plan and residual
capacity *"without changing live ownership, diagnostics, or wire state"*, and it does so by
forking an entirely fresh `Router` for the candidate compile
(`let mut candidate_router = Router::new(self.rules.fork());`, `preview.rs:222`). The
freshness gate uses it to ask "would this demand need the wire?" without touching anything.
fava's planner is already pure, so it gets this for free — but nothing currently exposes
"what would this cost?" as a question separate from "do it".

**Verdict for the fava contract.** `plan()` taking `installed` is not wrong — it is needed
for attach, budget, and withdrawal. What is wrong is *feeding `installed`'s demand back
into the grouping pass*. The correct shape is:

```
plan(relay, pending_demand, installed, constraints, revision) -> SubscriptionPlan
  where:
    pending_demand  = demand not already served by an installed subscription
    grouping        runs over pending_demand ONLY
    installed       is consulted for: attach (does an installed filter already
                    cover this demand?), budget (residual subscription count),
                    and withdrawal (which installed ids have lost every owner)
    open            = new REQs for coverage still missing
    retain          = every installed id that still has at least one owner —
                      including ones whose `serves` set grew or shrank
    close           = installed ids whose owner count reached zero
```

Under that shape, `close` is only ever "last owner gone", `WithdrawalReason::Regrouped`
becomes unreachable for app admission, and a running subscription is never reopened.

## A.8 Late joiners and EOSE — nmp's answer in full

A demand that attaches to a subscription which has already fired EOSE has missed the
stored-event replay. nmp handles this explicitly, and the mechanism is the most
transferable thing in the whole comparison.

**Coverage is recorded per narrow atom, never per merged filter.** `WireReq::coverage_claims`
(`crates/nmp-router/src/plan.rs:151-181`):

> every narrow demand atom's window-erased `CoverageKey` this (possibly coalesced) wire
> filter supersets — populated at revision (one key per pre-coalesce atom entry) and
> concatenated through every `coalesce_with` merge exactly as `provenance` already is.
> Because every merge in this crate is widen-only-proven, `wide ⊇ atom` holds for every key
> in `coverage_claims` BY CONSTRUCTION at the moment of revision — this is the
> containment rule the ruling requires, **discharged once, here, never re-derived at read
> time by subset-testing filters (banned by the ruling).**

**One EOSE credits every atom in the REQ**, and the interval is the intersection over every
outstanding snapshot on that wire id (`crates/nmp/src/core/attribution/completion.rs:216-284`):

> THE load-bearing rule: attribution is the INTERSECTION of every accepted snapshot
> currently outstanding on this exact wire id/filter generation — never just the newest.
> Replay or repeated accepted delivery can leave more than one outstanding attempt for the
> same immutable request, and a relay may EOSE the older attempt after a newer one was sent.
> Crediting only the current snapshot would attribute atoms the actual terminating REQ never
> asked for.

**An EOSE on a limited request proves nothing** (`completion.rs:47-51`):

```rust
coverage_authority: if filter.limit.is_some() {
    CoverageAuthority::Poisoned(CoveragePoison::LimitedRequest)
} else {
    CoverageAuthority::Eligible
},
```

If *any* outstanding snapshot on the FIFO is poisoned, the whole completion records nothing
for any key. Test: `crates/nmp/tests/core_headless/live_queries.rs:996`
`limited_fetch_never_records_coverage`.

**The late joiner is settled by claim transfer, not by a fresh REQ.**
`crates/nmp/src/core/query.rs:432-477` → `attribution.extend_current_request_claims`:

- **EOSE not yet fired** → the newcomer's coverage keys are appended to the live snapshot;
  the pending EOSE credits it normally.
- **EOSE already fired** → `transfer_finished_request_claims` (`query.rs:651-733`). Guards:
  the live request exists, its filter hash matches, `limit.is_none()`, and its state is
  `StoredEvents::Finished { committed_interval: Some(interval) }`. The **already-earned
  interval is then written for the newcomer's coverage keys** via
  `Effect::RecordCoverage(key, relay, interval)`. A store failure is retained in
  `pending_request_claim_transfers` with bounded retry, and cancelled if the request is
  superseded (`reconcile_request_claim_transfers_except`, `query.rs:834-853`).

Rows themselves come from the ordinary local-store projection at observation open
(`query.rs:249-256`). Tests:
`crates/nmp/src/core/admission_tests/claim_transfer_retry.rs:6` and `:148`;
`crates/nmp/src/core/admission_tests/request_filter_sharing.rs:7` — 207 observations, **one**
REQ.

So the newcomer never gets a replayed EOSE fact; it derives "stored events finished" from
`SourceStatus::FinishedStoredEvents`, which requires *every* wire request absorbing a
covered atom to be finished (`crates/nmp/src/core/evidence.rs:252-258`).

## A.9 What fava does today, measured against A.1–A.8

| nmp rule | fava today | where |
|---|---|---|
| Debounce cohort before grouping | **none.** No batching layer anywhere. The one production caller passes a **1-element** demand slice: `let demand = [demand_for_query(owner, QueryBranchId::ROOT, query)];` | `crates/fava/src/relay.rs:182-194` |
| Merge over unsent demand only | grouping runs over the **entire** `&[RelayDemand]` slice with no knowledge of `installed` | `crates/fava-subscriptions-standard/src/grouping.rs:22-45` |
| Running REQs are immutable | a candidate whose filter differs from the installed entry's produces `open` + `close` | `crates/fava-subscriptions-standard/src/diff.rs:42-53` |
| Membership change keeps the id | id is `FNV-1a(canonical filter JSON)`, so **any** membership change moves the id | `crates/fava-subscriptions-standard/src/wire.rs:24-40, 82-104` |
| Attach when an installed filter already covers new demand | only **byte-identical** attach exists (`reusable` requires `entry.filters == candidate.filters`); no containment test | `diff.rs:86-89` |
| Close only at refcount zero | `close` is "every installed id the plan no longer wants", including ids replaced by a regrouped successor (`WithdrawalReason::Regrouped`) | `diff.rs:117-154` |
| Per-narrow-atom coverage, limited-EOSE poison, claim transfer | none. `RelaySourceState::StoredEventsComplete` is **never constructed** anywhere in the repo; EOSE is diagnostics-only | `crates/fava/src/relay.rs:285-292`; `crates/fava-query/src/evidence.rs:140-143` |
| Per-demand re-match on delivery | production keeps only `BTreeMap<SubscriptionId, Filter>` — the merged filter — and matches against that; `attribution.serves()` is never called outside the testkit | `crates/fava/src/relay.rs:229-238`; `crates/fava-ingest/src/lib.rs:42-65` |

Two of those deserve emphasis.

**The runtime is pre-rewrite.** `crates/fava/src/relay.rs` passes
`InstalledSubscriptions::empty()` and `PlanRevision(1)` on every call including reconnect,
so `retain`/`close`/`Regrouped` never fire in production today, and no merging ever happens.
FROZEN-CONTRACTS §0 says that file is expected to be rewritten in Wave 3. So none of the
structural damage below is *currently* live — but it is what the current contract shape will
produce the moment an aggregator is wired in.

**The multi-filter REQ is already lossy.** `relay.rs:232-235` keeps only `filters.first()`,
so a `PlannedSubscription` with more than one filter silently loses the rest. Worth noting
that nmp rejected multi-filter REQs outright (design note §8.3): *"EOSE and CLOSE are
per-subscription, so a list coarsens per-filter completion and forbids independent
teardown."*

---

# Part B — The merge predicate

## B.1 nmp's rule, stated precisely

`ConcreteFilter` has exactly seven fields — `kinds`, `authors`, `ids`,
`tags: BTreeMap<TagName, BTreeSet<String>>`, `since`, `until`, `limit`. **There is no
`search` field**, so that question does not arise in nmp.

Component model (`crates/nmp-router/src/component.rs:5-9`):

```
since | until | kinds | authors | ids | ONE COMPONENT PER TAG NAME | limit
```

```
R0  REFUSE if a.limit.is_some() OR b.limit.is_some().
       Checked FIRST, before any component comparison, because two EQUAL limits
       produce no differing component and would otherwise sail through.

R1  Let D = { components in which a and b disagree }.
       None-vs-Some(..) DOES disagree, including authors:None vs authors:Some(∅).
       A tag NAME present on one side and absent on the other IS a disagreement.

R2  REFUSE if |D| != 1.
       |D| = 0  -> exact duplicate; owned by hash dedup, not by the rule.
       |D| >= 2 -> the CARTESIAN-CORNER refusal. A hard rule, not a default.

R3  c = the sole differing component.
       c ∈ {Since, Until}        -> REFUSE (bounds, not value sets).
       c == Limit                -> REFUSE (unreachable given R0; defence in depth).
       c ∈ {Kinds, Authors, Ids} -> REFUSE unless BOTH sides are Some(non-empty).
                                    Else merged.c := a.c ∪ b.c.
       c == Tag(name)            -> REFUSE unless BOTH sides have `name` PRESENT.
                                    An EMPTY value set on either side is ALLOWED.
                                    Else merged.tags[name] := a.tags[name] ∪ b.tags[name].

R4  REFUSE if the merged set exceeds a cap:
       Ids 256 (MAX_IDS_PER_FILTER), Tag 500 (MAX_TAG_VALUES_PER_FILTER).
       Over-cap unions SHARD into more REQs; no value is ever dropped.

R5  Every other field of merged is a's (== b's, by R2).
```

Pipeline around the rule (`RuleRegistry::coalesce_with`, `coalesce.rs:363-416`):

```
S1  Exact-canonical dedup by BLAKE3 DescriptorHash, applied REGARDLESS of limit.
    This also CANONICALISES INPUT ORDER (BTreeMap keyed by hash).
S2  Fixed-point pairwise merge, with prefix revalidation.
S3  Exact-canonical dedup AGAIN over survivors — a merge can RE-CREATE a filter
    the pool already holds.
```

## B.2 The four comments that are the point of the exercise

**Limit (`neither_limited`, `coalesce.rs:229-241`).** The refinement history is that this
*used to be* `a.limit == b.limit` and was tightened in commit `812c1d2f`:

> A relay-side `limit` caps the RESULT COUNT, not a predicate: two `limit:200` REQs for
> disjoint author sets each promise up to 200 rows (400 total), but a merged
> `{authors: a∪b, limit:200}` REQ still only promises 200 — the relay truncates the union,
> and the union silently under-fetches relative to what the two original REQs would have
> delivered. `matches(try_merge(a,b)) ⊇ matches(a) ∪ matches(b)` only holds for a
> bounded-COUNT filter when neither side is bounded at all; **requiring equal (rather than
> absent) limits looked like a safety guard but did not actually save the widening property.**

That commit's own note on why the old test could not catch it: *"the per-event `match_event`
property test this replaces could never model relay-side truncation, so this checks the
actual precondition instead."*

**Unconstrained operands (`both_constrain`, #900, commit `52de816c`).**

> `None` on `authors`/`kinds`/`ids` is not "the empty set", it is NO CONSTRAINT ON THIS AXIS.
> `unwrap_or_default()` silently converts that into `∅`, so the union of an unconstrained
> operand with a constrained one came out equal to the constrained one — a filter matching
> strictly FEWER events than its own first input. `Some(∅)` is refused for the same reason
> and not as belt-and-braces: `nostr`'s `match_event` treats an empty authors/kinds/ids set
> as unconstrained too, so folding it into a constrained sibling narrows identically.

And the meta-lesson, which is the durable half:

> #900 lived because the generators could not express an UNCONSTRAINED operand, so no
> generator could ever pair one against a constrained sibling — the single pairing that
> makes a union rule narrow. […] A widening property over pairs no rule accepts is
> vacuously green — that is the second, subtler reason #900 survived.

**Tag polarity (`coalesce.rs`, `Component::Tag` arm).**

> THE POLARITY INVERTS HERE, and getting it backwards reintroduces #900 on a new axis. On
> `authors`/`kinds`/`ids` the unconstrained shapes are `None` and `Some(∅)`. On tags the
> unconstrained shape is an ABSENT NAME (a filter that does not mention `#d` matches every
> event, tagged or not), while a PRESENT name with an empty value set matches NOTHING —
> `nostr`'s `match_event` evaluates `any()` over an empty set, which is false for tagged and
> untagged events alike.

> TAGS ARE ONE COMPONENT PER NAME. Tags are CONJUNCTIVE across names, so `{#e:X}` and
> `{#p:Y}` differ in TWO components and are refused. Had they been treated as one "tags"
> axis, the union would have demanded `#e:X` AND `#p:Y` together — a filter matching NEITHER
> operand. That is a narrowing, not a widening, and it is **the single most dangerous
> mistake available on this axis.**

**Post-merge dedup (`dedup_survivors`, commit `78fa32c7`).**

> Two byte-identical filters cannot be told apart by ANY identity scheme, so the only correct
> outcome is one req. Under the old derived identity they collided onto one id and
> `diff_plans` quietly kept one. Under allocation they would each get their OWN token and
> become two permanently-live duplicate REQs — the relay double-delivering every matching
> event forever, with `coverage_claims` split across two entries so neither is fully
> credited. **Strictly worse than the bug it replaced.**

**Fixed point (`merge_fixed_point`, #505).**

> a merge can UNLOCK a match between an UNTOUCHED earlier entry and the freshly-merged one
> that neither original operand qualified for. Concretely: merging `{authors:{a}}` and
> `{authors:{b}}` produces `{authors:{a,b}}`; a third entry `{kinds:{2}, authors:{a,b}}` is a
> TWO-component move from either input alone […] but a ONE-component move from the merged
> entry.

## B.3 fava's rule, stated the same way

`crates/fava-subscriptions-standard/src/grouping.rs`:

```
G0  REFUSE if anchor.bounds != item.bounds.          [no nmp analogue]
G1  ACCEPT as exact dedup if group_filter == item.filter.  (before the limit gate)
G2  REFUSE all axis merging if the relay DECLARED a default_filter_limit.  [no nmp analogue]
G3  REFUSE if either side carries a limit.
G4  AUTHORS axis: both Some(non-empty), every other nostr::Filter field equal
       (ids, kinds, search, since, until, limit, generic_tags) -> union authors.
G5  else ONE-TAG-VALUE-SET axis: every non-tag field equal, same tag key SET,
       exactly one key's values differ, both non-empty -> union that key.
G6  else REFUSE.
```

Grouping is a single greedy first-fit pass over the demand slice; a merged candidate is
stored as its member list and the filter is recomputed by `merged_filter()`, so splitting
undoes a merge rather than truncating a filter. Sizing and identity are outside the
predicate: `fit_message_bound` splits over a **declared** `max_message_bytes`,
`fit_subscription_count` drops over a **declared** `max_subscriptions`, and `wire::identity`
is `FNV-1a(serde_json(filter), salt)`.

## B.4 Rule-by-rule diff

Legend: **=** agree · **fava+** fava has a rule nmp lacks · **nmp+** the reverse.

| # | Hazard | nmp | fava | verdict |
|---|---|---|---|---|
| 1 | **Cross-product unsoundness** — `{k:0,a:1}` + `{k:1,a:2}` → `{k:[0,1],a:[1,2]}` | `sole_difference(a,b)?` returns `None` on ≥2 diffs | both axis functions require *every other field* of `nostr::Filter` to compare equal after removing their one axis | **= both correct. fava is sound. No work.** |
| 2 | **`limit`** | refuse if EITHER side carries one, even equal limits; exact-hash dedup of identical limited filters still allowed | G3 identical; G1 dedup identical | **= both correct, including the subtle part. No work.** |
| 3 | **Relay-applied *default* limit** | **absent.** `AdvertisedRelayLimits` reads only `max_subscriptions` and `max_subid_length`, so a relay declaring `limitation.default_limit` truncates nmp's merged unlimited REQs exactly like the same-limit case it guards | G2 refuses all axis merging | **fava+, fava correct. A real hole in nmp** — worth telling Pablo. |
| 4 | **`since`/`until`** | must be exactly equal; *"there is no union of two windows that is not either a narrowing or a widening far past both operands"*. Windows are also part of `DemandKey` (though erased from durable `CoverageKey`), so a differently-windowed demand gets its **own REQ** rather than attaching: *"an already-running live request does not backfill a newly-requested older page, and a bounded request is not interchangeable with an unbounded one"* (`plan.rs:35-42`) | exactly equal, **plus** the `QueryBounds` gate | **= both correct; fava strictly stricter, and nmp independently confirms the `QueryBounds` gate is the right idea.** No work. |
| 5 | **Tag filters** | one component per name; conjunctive refusal; absent name refused, empty value set allowed | one differing key, same key set required; empty value set on either side refused | **= sound. fava slightly stricter** (refuses `{#t:∅}` ∪ `{#t:[x]}`, a legal widening). Cosmetic. |
| 6 | **`ids`** | mergeable, unioned, capped at 256 | **not mergeable** | **nmp+.** Missed collapse. |
| 7 | **`kinds`** | mergeable, unioned | **not mergeable** | **nmp+.** Missed collapse, and the common one. |
| 8 | **`search`** | field does not exist | part of the "every other field equal" comparison, so never merged across | **fava+, fava correct.** Unioning two NIP-50 search strings is not expressible. |
| 9 | **Subsumption vs union** | **yes — but at admission, not in the rule.** `physical_filter_covers` (`admission/metadata.rs:30-62`) is a real containment test that lets a new demand attach to an already-sent broader filter with no REQ. Limited requests stay exact-only | **byte-identical only** (`diff.rs:86-89`) | **nmp+, and this is the item the original brief was right to flag.** nmp deliberately keeps subsumption *out* of `try_merge` (there it refuses `None`-vs-`Some`) and puts it in the attach path, where it costs zero REQs instead of one. |
| 10 | **Attribution after merge** | `deliver.rs` re-matches each event against each consuming atom's own filter; multi-atom events go to each | contract + testkit re-match per demand and return a *set* of `DemandId` | **= semantically identical in the contract.** But production does not do it — see C6. |
| 11 | **EOSE after merge** | one EOSE credits every atom; coverage recorded under **narrow atom keys**, proven over the **intersection** of outstanding snapshots; a `limit` poisons the whole attribution | `serves` doc: "An EOSE on this wire id settles every one of them"; nothing consumes it | **= sound in the contract for the merged-at-open case. nmp+ on narrow-key coverage and limited-EOSE poisoning.** |
| 12 | **Late joiner after EOSE** | claim transfer writes the already-earned interval to the newcomer's coverage keys (§A.8) | no coverage layer exists; the newcomer is silently un-settled | **nmp+, structural.** |

## B.5 Fixed point, determinism, and identity

**Fixed point.** nmp merges to a fixed point and re-dedups survivors; fava does neither.
Today this costs fava almost nothing, because with only the authors axis and a
one-tag-value-set axis two grown groups can essentially never become mergeable. **The
moment a kinds or ids axis is added the cross-axis unlock falsifier becomes live**, so C7
and C8 must land together.

**Determinism.** nmp canonicalises twice and independently: `coalesce_with` step S1 folds
into a `BTreeMap<DescriptorHash, Entry>`, and `wire_id::assign` re-sorts by canonical hash
because it *"does not trust the coalescer's emission order"* (`wire_id.rs:112-117`). fava's
`group()` first-fits in slice order, and the wire id is a digest of the resulting merged
filter, so a permuted input can churn the wire with no change in demand. The current caller
passes one element, so this is latent — but the planner is a published contract with a
conformance kit for competing providers and must not carry an unstated ordering
precondition.

**Identity.** Three things nmp's allocated token handles that an FNV-1a content digest
cannot.

*(a) Recycling.* `SubId::allocate` (`plan.rs:79-103`):

> `counter` is the router's own monotonic mint counter, so no token is ever recycled within
> a `Router`'s lifetime — **reuse would let a stale in-flight EOSE for a closed subscription
> land on a reopened one's attribution FIFO.**

A content digest recycles by construction, which collides head-on with fava's own frozen
requirement (`crates/fava-query/src/identity.rs:68-70`, GOALS:426/QUERY-010): *"Reopening
dropped demand MUST use fresh request identity so a late EOSE or event from the old request
cannot settle the new one."* `PlanRevision` and `OperationGeneration` exist in the contract
but neither reaches the wire id.

*(b) Injectivity, and the 64-bit problem.* nmp *had* FNV-1a and removed it
(`crates/nmp-grammar/src/concrete.rs:52-68`):

> A 256-bit BLAKE3 digest, NOT a 64-bit hash: `ConcreteFilter`'s contents are
> network-controlled (a hostile `kind:3`/`kind:10002` steers a `Binding::Derived` author
> set), so this value must resist DELIBERATE collision construction, not just accidental
> clashes. A 64-bit hash (the previous implementation used FNV-1a) is offline-constructible
> by a determined attacker.

fava's digest is over proper JSON, so it has no *framing* ambiguity — but it is 64 bits over
network-controlled tag values, and under a declared `max_subscription_id_chars` it is
**truncated** to as few as 1 hex char.

*(c) NIP-11 must not move an established id.* `SubId::allocate`:

> Deliberately NOT derived from anything mutable the relay advertises: no NIP-11 field
> (`max_subid_length` and friends) feeds this, so a relay changing its advertisement can
> never move an established id.

`fava::wire::identity` takes `constraints` and truncates to `max_subscription_id_chars`.

**What fava's scheme does better.** Stateless retention is real and valuable — nmp pays for
allocation with a greedy predecessor sweep it documents as knowingly suboptimal, and fava's
`WithdrawalReason::Regrouped { into }` names the successor explicitly, which is *more*
information than nmp's `predecessor` back-pointer. The fix is not to abandon the digest; it
is to stop feeding it NIP-11 values and to fold in a monotonic discriminator for
freshly-opened ids.

## B.6 Relay limits — fava is ahead

nmp reads two NIP-11 fields and enforces one. fava models five, refuses to invent any, and
turns each into typed in-plan shortfall:

- `max_message_bytes` → `fit_message_bound` splits a merged REQ into exact subsets. nmp
  hardcodes `MAX_IDS_PER_FILTER = 256` and `MAX_TAG_VALUES_PER_FILTER = 500`, which its own
  comments admit are operational guesses.
- `max_filter_limit` → typed `FilterLimitExceeded`, not a clamp.
- `max_subscription_id_chars` → enforced. nmp's `max_subid_length` is *"DIAGNOSED when
  present, never enforced."*

The mirror cost: when a relay declares nothing, fava applies **no** value cap, so a
300-value tag union ships as one unbounded REQ many relays will reject. nmp's hardcoded caps
exist for exactly that case. This is a deliberate consequence of RELAY-004, so it is an
accepted trade — but the shortfall vocabulary should be able to represent "the relay refused
it" so demand is not silently lost.

## B.7 Context partitioning

nmp partitions by `(RelaySessionKey, SourceAuthority)` — relay × `AccessContext` × source —
**before** coalescing, and `coalesce.rs` never learns the type exists. Four things break
without it: forged cross-context coverage rows; a re-aliased attribution FIFO; a NIP-42
visibility leak (Public and Nip42 are physically different sockets, so a merged filter is not
even sendable); and a `Pinned`-authority violation. fava's only partition key is
`QueryBounds`. If authenticated and unauthenticated demand can share a relay session in fava,
an isolation partition belongs above `group()`.

---

# Part C — Ranked change list

Severity counts: **Critical 1 · High 5 · Medium 4 · Low 5**, plus 6 points where fava is
already correct and 3 where fava is ahead of nmp.

Each item names the failing case as a test.

---

### C1 — CRITICAL (structural) — Grouping must run over unsent demand only; never rewrite a running subscription

**The failure.** Installed subscription `S = {authors:[A]}` serves D1 and has already fired
EOSE. Demand D2 = `{authors:[B]}` arrives. `group()` merges D1 and D2 into
`{authors:[A,B]}`; the digest changes; `diff::assemble` emits `open(S')` and
`close(S, Regrouped{into: S'})`. D1's completed subscription is torn down and re-run — the
relay re-serves every stored event for A, and D1 receives a second EOSE for work that was
already done. nmp's phrasing of exactly this: *"This avoids paying the relay to rerun a broad
query merely because another screen element appeared."*

**The mirror failure on withdrawal.** D2 goes away. `group()` now yields `{authors:[A]}`, a
different digest again, so the subscription serving D1 is closed and reopened a second time
— for a narrowing that buys nothing. nmp: *"A physical request closes only when its
incremental active owner count reaches zero"*, and the surplus is discarded by the local
re-filter.

**The change.** Restructure `StandardSubscriptionPlanner::plan` as A.7 specifies:
1. Partition input demand into *served* (some installed subscription's filters already cover
   it) and *pending*.
2. Run `grouping::group` over **pending only**.
3. `retain` every installed id that still has ≥1 owner, with its `serves` set updated.
4. `close` only ids whose owner count reached zero.
5. `open` only genuinely new REQs.

`WithdrawalReason::Regrouped` should become unreachable on this path. Keep it for the rare
whole-demand invalidation (reconnect, route change), which is nmp's `Router::compile`.

```rust
#[test]
fn new_demand_never_reopens_an_installed_subscription() {
    // installed: {authors:[A]} serving D1.  new demand set: [D1, D2={authors:[B]}]
    // assert!(plan.close.is_empty());
    // assert_eq!(plan.retain, vec![id_of_A]);
    // assert_eq!(plan.open.len(), 1);          // a separate REQ for B
}

#[test]
fn withdrawing_one_member_leaves_the_survivors_subscription_untouched() {
    // installed: {authors:[A,B]} serving {D1,D2}.  new demand set: [D1]
    // assert!(plan.close.is_empty());
    // assert_eq!(plan.retain, vec![id_of_AB]);
    // assert_eq!(plan.attribution.serves(&id_of_AB), &btreeset![D1]);
}
```

This is also a conformance rule the contract can enforce for *every* planner:
**CR-1 — no installed id may appear in `close` while any demand it serves is still in the
input demand set.**

---

### C2 — HIGH — Introduce the pending-admission cohort

Grouping has nothing to group without it. fava's one production caller hands the planner a
**one-element** slice (`crates/fava/src/relay.rs:182-185`), so today the entire merge
machinery is unreachable. nmp's mechanism is small and worth copying verbatim in shape:

- A **first-arrival-anchored, non-sliding** deadline: `arm()` sets it only
  `if self.deadline.is_none()`.
- **10ms** (`WIRE_ADMISSION_WINDOW`), fixed, not adaptive.
- The timer may be global; the **grouping locus** must be the per-relay-session partition,
  *"so two queries are never combined merely because they happened to arrive together."*
- Cancellation before the deadline removes the demand without producing a REQ.
- The window must not postpone local cache delivery — the observation projects from cache
  immediately and reads no sibling.
- A demand refused for budget stays in the cohort and is retried in a later window.

**Do not make it sliding.** nmp rejected that explicitly (design note §8.3): *"A sliding
deadline can starve under a steady arrival stream, and batching resolver atoms destroys
their coverage identity. The built admission window is different: fixed at 10ms from the
first uncovered app request, delays only unsent wire work, and groups only after routing
inside a relay/context/source partition."*

Belongs in the relay-session owner (Wave 3's replacement for `crates/fava/src/relay.rs`), not
in `fava-subscriptions-standard`. The planner stays a pure function; what changes is *how
often and with what* it is called.

```rust
#[test]
fn a_burst_of_queries_within_the_window_produces_one_grouped_req() { /* ... */ }

#[test]
fn a_query_arriving_after_the_window_opens_its_own_req_and_closes_nothing() { /* ... */ }

#[test]
fn cancellation_before_the_deadline_produces_no_req() { /* ... */ }
```

---

### C3 — HIGH — Add subsumption at the attach boundary, not in the merge rule

nmp's `physical_filter_covers` is a genuine containment test over kinds/authors/ids/tags/
since/until, used to decide whether a new demand needs a REQ at all. fava's equivalent
(`diff.rs:86-89`) only matches byte-identical filters, so a demand covered by a live broader
REQ still opens a second one.

Two constraints from nmp, both load-bearing:
- **Limited requests stay exact-only** — *"their result-count boundary is not a set axis and
  cannot safely be reconstructed for a later owner."*
- Subsumption belongs in *attach*, not in `try_merge`. Inside the merge rule, nmp deliberately
  refuses `None`-vs-`Some` (§B.2). The two are different operations with different costs: a
  merge costs a REQ rewrite, an attach costs nothing.
- **Do not subtract the incumbent's coverage from the new REQ.** If a demand is not fully
  covered, ship its *full* filter. nmp keeps the residual optimisation behind an `#[ignore]`d
  test: *"until residual subtraction is proven safe, executing the full later filter prevents
  underfetch."* Over-fetch is absorbed by the per-demand re-match; under-fetch is silent
  data loss.

```rust
#[test]
fn demand_covered_by_a_live_broader_subscription_opens_no_req() {
    // installed {kinds:[1]} (no authors).  new demand {kinds:[1],authors:[A]}
    // assert!(plan.open.is_empty());
    // assert_eq!(plan.attribution.serves(&installed_id).len(), 2);
}

#[test]
fn a_limited_subscription_never_absorbs_a_later_owner() { /* exact-only */ }
```

---

### C4 — HIGH — Never recycle a wire subscription id

`wire::identity` is a pure function of `(filters, declared id length, salt)`. Close a
subscription and re-demand the same filter and the same id comes back, so a late EOSE or
EVENT for the closed request settles the new one — which GOALS:426 (QUERY-010) forbids by
name.

Fix shape: keep `identity(filters, constraints, salt)` for the *retention* check against
`installed` (that is what makes stateless retention work), but mint **newly-opened** ids from
a digest that also folds a monotonic planner-owned epoch. nmp's `ALLOCATED_DOMAIN` byte is a
good pattern for keeping the two namespaces from ever colliding.

```rust
#[test]
fn a_reopened_filter_never_reuses_the_closed_subscription_id() {
    // plan {authors:[A]} -> X; execute; plan with empty demand -> close X;
    // plan {authors:[A]} again -> assert!(replan.open[0].id != X);
}
```

---

### C5 — HIGH — Fold byte-identical planned filters into ONE subscription

`merge_candidate` returns `None` on a `QueryBounds` mismatch **before** it reaches the
`filter == item.filter` dedup (G0 precedes G1). Two demands with identical filters and
different bounds therefore produce two candidates carrying identical `filters`;
`diff::resolve_identity` sees the digest collision and **salts to a second id** — two
byte-identical REQs live on one relay, the relay double-delivering every matching event, one
subscription slot burned. nmp's verdict on this exact outcome: *"strictly worse than the bug
it replaced."*

Fix shape: nmp's step S3. After `fit_message_bound`, fold candidates with equal `filters`
into the first occurrence, unioning `serves`. Sound irrespective of bounds — identical wire
bytes mean identical relay behaviour.

```rust
#[test]
fn two_demands_with_identical_filters_and_different_bounds_share_one_subscription() {
    // assert_eq!(plan.open.len(), 1);
    // assert_eq!(plan.attribution.serves(&plan.open[0].id).len(), 2);
}
```

Conformance rule **CR-2 — no two entries in `installed_after()` may carry equal `filters`.**
That would have caught this without anyone thinking of the bounds case.

---

### C6 — HIGH — Wire per-demand re-match and per-demand EOSE settlement into the runtime

The re-match is proven in the contract and the testkit, but production
(`crates/fava/src/relay.rs:229-238`) flattens `SubscriptionAttribution` to
`BTreeMap<SubscriptionId, Filter>`, discarding `serves` entirely and keeping only
`filters.first()`. `fava-ingest` matches against that single merged filter, so every member
of a group would see the whole union — an access-isolation break, and exactly what the
differential test `grouping_keeps_two_observations_isolated_from_each_other` asserts must not
happen. EOSE is diagnostics-only; `RelaySourceState::StoredEventsComplete` is never
constructed anywhere in the repository.

This is Wave 3 work and the planner is not at fault, but it is the gap that turns a sound
planner into an unsound system, and it should be tracked as a blocker on enabling grouping.

---

### C7 — MEDIUM — Add the `kinds` axis (with C8)

`{authors:[A],kinds:[0]}`, `{authors:[A],kinds:[3]}`, `{authors:[A],kinds:[10002]}` is three
REQs in fava and one in nmp — against a real-world ceiling of roughly 20 concurrent
subscriptions. Sound by the same one-component argument already implemented for authors, with
the same `Some(non-empty)` guard on both sides.

Borrow nmp's framing: the trio `AuthorUnion`/`KindUnion`/`IdUnion` was *"three copies of one
idea plus a missing fourth."* fava has two copies and two missing. A single
`merge_on_sole_differing_axis` over `{ids, authors, kinds, one tag name}` replaces
`merge_author_axis` and `merge_tag_axis` and cannot drift between them.

```rust
#[test]
fn three_kind_queries_for_one_author_share_one_wire_request() { /* -> 1 req, kinds [0,3,10002] */ }

#[test]
fn an_unconstrained_kinds_operand_is_never_folded_into_a_constrained_one() {
    // kinds:None and kinds:Some(∅) are BOTH unconstrained to nostr's match_event.
}
```

---

### C8 — MEDIUM — Merge to a fixed point, in the same commit as C7

`group()` is a single greedy first-fit pass. With only the authors axis the cross-axis unlock
is unreachable; **with C7 landed it is immediately reachable**, so shipping C7 alone
introduces a silent regression in collapse quality.

```rust
#[test]
fn a_merge_that_unlocks_a_third_group_reaches_the_fixed_point() {
    // {k:[1],a:[A]}, {k:[1],a:[B]}, {k:[2],a:[A,B]}  ->  ONE req {k:[1,2],a:[A,B]}
}
```

---

### C9 — MEDIUM — Canonicalise demand order inside the planner

`group()` first-fits in slice order; the wire id digests the resulting merged filter; so a
permuted input can churn the wire with no change in demand. nmp canonicalises inside the
coalescer *and* again inside `wire_id::assign`, explicitly refusing to depend on its caller.
Sort by `(canonical filter encoding, DemandId)` at the top of `group()`.

```rust
#[test]
fn grouping_is_invariant_under_demand_permutation() {
    // plan(demand) == plan(shuffled(demand)), ids included.
}
```

Conformance rule **CR-3 — the plan is a function of the demand *set*, not the demand
*sequence*.**

---

### C10 — MEDIUM — Stop feeding NIP-11 values into the wire id

`wire::identity` truncates to `constraints.max_subscription_id_chars`, so a NIP-11 refetch
moves every established id on the relay. Mint at a fixed width and enforce the declared bound
as an admission check with typed shortfall.

```rust
#[test]
fn a_changed_declared_id_length_does_not_move_installed_subscription_ids() { /* ... */ }
```

---

### C11 — LOW — Add the `ids` axis

Folded into C7's single axis function. Fetching N events by id is N REQs today.

---

### C12 — LOW — `merged_filter` returning `None` silently drops a whole group

`fit_message_bound`'s `let Some(filter) = ... else { continue; }` drops every member's demand
with **no** shortfall recorded — a C8 (`DemandUnaccounted`) violation that only `validate_plan`
would catch. Currently unreachable, but it guards an invariant C7/C8 are about to change. Make
it a shortfall, never a `continue`.

---

### C13 — LOW — Allow an empty tag value set to be unioned in

nmp allows it correctly: a present tag name with an empty value set matches nothing, so
unioning it in is a widening. Do it as part of C7's rewrite, and record *why* the polarity
differs from the authors axis — the two guards look contradictory and a future reader will
"fix" one of them.

---

### C14 — LOW — Record that a limited EOSE is not proof of completeness

nmp poisons the whole attribution when `filter.limit.is_some()`, and records coverage under
narrow atom keys only. fava has no evidence layer to poison yet, but the planner is the only
component that knows a REQ was merged and whether the relay declared a `default_filter_limit`.
`AttributedSubscription` should carry that fact forward rather than forcing the evidence layer
to re-derive it.

---

### C15 — LOW / open question — Access-context partitioning above the merge predicate

If authenticated and unauthenticated demand can share a relay session in fava, an isolation
partition belongs above `group()`, as nmp's `(RelaySessionKey, SourceAuthority)` bag is above
`coalesce_with`. If they cannot, record that fact so the question is not re-litigated.

---

## C.16 What fava should NOT change

- **Cross-product refusal is correct.** `{kinds:[0],authors:[1]}` + `{kinds:[1],authors:[2]}`
  cannot merge. No work.
- **The `limit` rule is correct, including the subtle part** — refusing when *either* side
  carries a limit, not merely when they differ, is exactly the rule nmp had to refine into
  place.
- **`since`/`until` handling is correct**, and the `QueryBounds` gate is strictly more
  conservative than nmp.
- **`search` handling is correct**, and nmp cannot even express the question.
- **The `default_filter_limit` guard is correct and nmp is missing it.** Do not weaken it.
- **Representing a candidate as its member list and recomputing the filter (`merged_filter`)
  is better than nmp's approach** — splitting undoes a merge instead of truncating a filter,
  and attribution cannot drift from the filter it describes.
- **The declared-limit model (five limits, never invented, typed shortfall) is substantially
  better than nmp's two-field `AdvertisedRelayLimits`.**
- **The differential testkit is stronger than nmp's oracle** — it asserts grouped ≡ ungrouped
  delivery over an event corpus, access isolation between two observations, EOSE settlement
  equivalence, and withdrawal equivalence. Extend it rather than replace it.

## C.17 Two things worth sending back to nmp

1. **No `default_filter_limit` guard.** A relay declaring `limitation.default_limit` truncates
   nmp's merged *unlimited* REQs exactly as the same-limit case `neither_limited` was written
   to prevent. `AdvertisedRelayLimits` reads only `max_subscriptions` and `max_subid_length`.
2. **A stale assertion message** left by the `absorbed` → `coverage_claims` rename in
   `1d5e47fc`: `crates/nmp-router/src/coalesce.rs` carries
   `"an unconstrained filter must not be coverage_claims"` (formerly "must not be absorbed").
