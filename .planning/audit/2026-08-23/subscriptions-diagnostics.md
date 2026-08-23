# Subscriptions & diagnostics audit

**Area slug:** `subscriptions-diagnostics`
**Date:** 2026-08-23
**Mode:** read-only

## Scope checked

Source read in full:

- `crates/fava-subscriptions/src/lib.rs` (117 lines)
- `crates/fava-subscriptions-standard/src/lib.rs` (175 lines)
- `crates/fava-subscriptions-no-grouping/src/lib.rs` (56 lines)
- `crates/fava-diagnostics/src/lib.rs` (223 lines)
- `crates/fava-subscriptions/tests/{kinds,tag_values}.rs`,
  `crates/fava-subscriptions-standard/tests/grouping.rs`,
  `crates/fava-subscriptions-no-grouping/tests/plan.rs`,
  `crates/fava-diagnostics/tests/relay_facts.rs`
- Consumer sites: `crates/fava/src/relay.rs`, `crates/fava/src/live.rs`,
  `crates/fava/src/routes.rs`, `crates/fava/src/lib.rs`
- `apps/canary/src/grouping.rs` (the RELAY-003 300-query acceptance evidence)
- `crates/fava-ingest/src/lib.rs` (`admit_subscription_event` signature)
- `crates/fava-query/src/selection.rs` (what `demand_for_query` must carry)

Authority read:

- `docs/spec/ARCHITECTURE.md` — `## fava-subscriptions` (1476-1521),
  `## fava-subscriptions-standard` (1524-1552), `## fava-ingest` inputs
  (2028-2050), `## fava-observe` owned state (2061-2110),
  `## fava-diagnostics` (2305-2336), ownership ledger (2960-3010),
  dependency edges (3050-3070), contract matrix (3100-3112),
  external-provider rules (3140-3158), Falsifier J (3321-3331),
  crate inventory (3595-3660), builder examples (28-60)
- `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` — RELAY-001..012
  (1031-1145), QUERY-010/011/013 (420-465), OPS-001..004 (1387-1437)
- `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` — 3.6 mechanism-disable (94-110),
  `### fava-subscriptions` differential rule (279-282), 9.3 independent
  witnesses (342-352)
- `docs/internals/vocabulary.toml` — `SubscriptionPlanner` (668-693),
  `Diagnostics` (856-869), `Group` (191-215)

Searches actually run (results used for absence claims):

- `grep -rn "SubscriptionPlanner|subscription_planner|fava_subscriptions" crates --include=*.rs --include=*.toml`
  -> exactly **one** `SubscriptionPlanner::plan` call site in the whole
  workspace: `crates/fava/src/relay.rs:181`.
- `grep -rn "fava_diagnostics|Diagnostics" crates` -> `Diagnostics` mutating
  methods are called **only** from `crates/fava/src/relay.rs` and
  `crates/fava/src/routes.rs`. No other crate publishes a diagnostic fact.
- `grep -rn "nip11|NIP-11|RelayInformation" crates` -> **zero hits**. No NIP-11
  service crate exists; no relay-advertised limit reaches any planner.
- `ls crates` -> `fava-subscriptions-testkit` (ARCHITECTURE.md:3653) does not
  exist. Neither does any planner conformance kit.
- `grep -rn "fava-subscriptions-standard" --include=*.toml .` -> the standard
  planner is depended on only by `apps/canary` and
  `falsifiers/external-semantic-capability`; it is **never** wired into
  `Fava::builder()` in any test in `crates/fava/tests/`.

---

## Findings

### planner-contract-shape — critical — ownership

**Authority.** `docs/spec/ARCHITECTURE.md:1483-1503`:

```rust
pub trait SubscriptionPlanner: Send + Sync {
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        constraints: &RelayReadConstraints,
    ) -> Result<SubscriptionPlan, SubscriptionPlanError>;
}

pub struct RelayDemand {
    pub owner: ObservationId,
    pub branch: QueryBranchId,
    pub filter: Filter,
    pub bounds: QueryBounds,
}

pub struct SubscriptionPlan {
    pub wire: Vec<PlannedSubscription>,
    pub attribution: SubscriptionAttribution,
    pub shortfalls: Vec<SubscriptionShortfall>,
}
```

and `docs/spec/ARCHITECTURE.md:1508-1514` — owned meaning includes
"logical-to-wire attribution; **plan diff values**; **relay-limit
shortfalls**; **withdrawal identity**; the conformance rules that define
semantic equivalence."

**Implementation.** `crates/fava-subscriptions/src/lib.rs:53-65`:

```rust
pub trait SubscriptionPlanner: Send + Sync {
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
    ) -> Result<SubscriptionPlan, SubscriptionPlanError>;
}
```

with `crates/fava-subscriptions/src/lib.rs:13-18`:

```rust
pub struct RelayDemand {
    pub subscription_id: SubscriptionId,
    pub filter: Filter,
}
```

and `crates/fava-subscriptions/src/lib.rs:33-42` — `SubscriptionPlan { relay,
messages: Vec<ClientMessage<'static>>, attribution: BTreeMap<SubscriptionId,
Filter>, demand: BTreeMap<SubscriptionId, Vec<SubscriptionId>> }`.

Gap by gap against the specified model:

| Required capability | Spec locus | Present? |
|---|---|---|
| Retained aggregate per-relay logical demand | `plan(.., demand: &[RelayDemand], ..)` with `owner: ObservationId` per item, ARCH:1487/1493 | **No** — demand identity is a bare wire `SubscriptionId`; there is no observation owner, so the planner cannot tell two observations' demand apart from one observation's two branches |
| Relay-declared read constraints | `constraints: &RelayReadConstraints`, ARCH:1488 | **No** — parameter absent; limits are frozen into planner construction (`StandardSubscriptionPlanner::bounded`, standard/src/lib.rs:31) and therefore identical for every relay |
| Desired-plan revision + diff vs installed plan | "plan diff values", ARCH:1511; "Wire subscription plan \| `fava-observe` owns desired plan; planner computes it", ARCH:2979 | **No** — `plan()` returns an absolute plan with no revision and no relationship to any previously installed plan |
| Stable unchanged subscriptions across replans | implied by diff + ARCH:2092 "subscription-plan changes ... routed to the exact affected observations" | **No** — every `plan()` produces fresh `ClientMessage::req` frames for everything; nothing marks a subscription as already installed |
| Refcounted withdrawal | "withdrawal identity", ARCH:1513; "ownership/refcounts for shared work", ARCH:2072 | **No** — `SubscriptionPlan` cannot express a CLOSE at all (`messages` is validated to be REQ-only, see `validate-plan-private-conformance`), and there is no per-wire-subscription refcount |
| Precise relay-limit shortfall | `shortfalls: Vec<SubscriptionShortfall>`, ARCH:1502; "report typed shortfall when exact execution does not fit", ARCH:1536 | **No** — shortfall is an all-or-nothing `Err(SubscriptionPlanError)` that discards the whole plan; a planner cannot say "these 60 fit, these 4 did not" |

**Required shape** (minimal, derived from the authority above):

```rust
pub trait SubscriptionPlanner: Send + Sync {
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],              // aggregate, per relay, all observations
        constraints: &RelayReadConstraints,  // per-relay, NIP-11 derived, unknown-able
        installed: &InstalledSubscriptions,  // what is currently live on this session
    ) -> Result<SubscriptionPlanDiff, SubscriptionPlanError>;
}

pub struct RelayDemand {
    pub owner: ObservationId,      // logical identity, not a wire id
    pub branch: QueryBranchId,
    pub filter: Filter,
    pub bounds: QueryBounds,
}

pub struct SubscriptionPlanDiff {
    pub revision: PlanRevision,
    pub open: Vec<PlannedSubscription>,   // wire id allocated by the plan, never a logical id
    pub retain: Vec<SubscriptionId>,      // proves stability across replans
    pub close: Vec<SubscriptionId>,       // withdrawal identity; only when refcount reaches zero
    pub attribution: SubscriptionAttribution, // wire id -> {ObservationId, QueryBranchId, filter}
    pub shortfalls: Vec<SubscriptionShortfall>, // partial plan + typed, attributable deficit
}
```

The three non-negotiable shape changes are: (a) demand identity must be
logical (`ObservationId`), separate from the wire `SubscriptionId` the planner
allocates; (b) the result must be a diff against installed state, not an
absolute plan, or `retain`/`close`/refcount is unexpressible; (c) shortfall
must be a value inside a successful plan, not only an error that annihilates
it.

**Observable distinction.** An application composing two live queries against
the same relay cannot, through any public API, obtain a plan in which one
subscription is retained and one is added — every replan re-issues every REQ.
And a competing planner cannot report "I planned 60 of your 64 filters"
because the return type has nowhere to put the remaining 4.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn replanning_retains_unchanged_wire_subscriptions() {
    let (fava, script) = fava_with_scripted_transport_and_two_router_revisions();
    let _a = fava.observe(query_alice()).await.unwrap();
    let _b = fava.observe(query_bob()).await.unwrap();   // same relay
    let reqs = script.frames_matching("REQ");
    assert_eq!(reqs.len(), 2, "second observation must not re-issue the first REQ");
}
```

**Confidence.** confirmed.

---

### singleton-demand-per-plan — critical — ownership

**Authority.** `docs/spec/ARCHITECTURE.md:1478` — "map **logical read demand
assigned to one exact relay session**" (plural demand for the session, not for
one query). `docs/spec/ARCHITECTURE.md:2978-2979` — "Query demand for one
relay | `fava-observe` | subscription planner" and "Wire subscription plan |
`fava-observe` owns desired plan; planner computes it".
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1041` — "The selected
subscription planner maps **all logical demand for one relay session** into
semantically equivalent wire subscriptions."

**Implementation.** `crates/fava/src/relay.rs:179-183`:

```rust
let subscription = allocate_subscription(next_subscription)?;
let plan = planner
    .plan(session_key, &[demand_for_query(subscription, query)])
    .map_err(|error| error.to_string())?;
validate_plan(session_key, &plan)?;
```

This is the only `plan()` call site in the workspace. It is reached once per
`OpenedRelay` (`crates/fava/src/relay.rs:40`, from `live.rs:34` and
`routes.rs:144`), and each `OpenedRelay` is one query on one relay. The slice
literal `&[..]` has length 1 unconditionally. `fava-observe` retains no demand
set at all (already-known baseline), so no aggregate exists to pass.

**Observable distinction.** Every grouping, deduplication, coalescing, and
cross-observation limit decision the contract exists to make is unreachable:
the planner is structurally never shown two demands. Two applications' worth
of overlapping queries on one relay produce N separate REQs on N separate
sockets regardless of which planner is selected.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn one_relay_receives_aggregate_demand_from_two_observations() {
    let planner = RecordingPlanner::default();   // records every demand slice
    let fava = fava_with(planner.clone(), scripted_transport());
    let _a = fava.observe(live_query_kind1_author_alice()).await.unwrap();
    let _b = fava.observe(live_query_kind1_author_bob()).await.unwrap();
    assert!(planner.slices().iter().any(|s| s.len() == 2),
        "planner was never shown aggregate per-relay demand: {:?}", planner.slices());
}
```

**Confidence.** confirmed.

---

### plan-demand-attribution-discarded — critical — behavioral proof

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1043` — "The
planner MUST preserve attribution from every wire request back to the logical
queries it serves." `docs/spec/ARCHITECTURE.md:2037` — `fava-ingest` inputs
include "current subscription attribution plan"; `:2044` — ingest must
"attribute an event to an accepted wire subscription **and logical demand**".

**Implementation.** `SubscriptionPlan.demand: BTreeMap<SubscriptionId,
Vec<SubscriptionId>>` (`crates/fava-subscriptions/src/lib.rs:41`) is computed
by both planners, structurally checked by `validate_plan`
(`crates/fava/src/relay.rs:228-229`) — and then **thrown away**:
`crates/fava/src/relay.rs:211` returns `Ok((session, plan.attribution))`, and
`OpenedRelay` stores only `attribution: BTreeMap<SubscriptionId, Filter>`
(`crates/fava/src/relay.rs:26`). Consequently
`crates/fava/src/relay.rs:269-277` calls

```rust
admit_subscription_event(cache, session.key(), &id, &id, filter, ...)
```

passing the wire id as both `expected_subscription` and
`actual_subscription`, and `crates/fava/src/relay.rs:284` records
`diagnostics.eose(key, generation, id)` with the wire id only.

**Observable distinction.** If grouping ever fired, an application could not
learn which of its logical queries an EOSE settled — the wire→logical map is
never carried past `validate_plan`. QUERY-010's "exact current
subscription/request identity" resolves to a wire id shared by an unknown set
of logical demands.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn eose_on_a_grouped_wire_subscription_settles_every_logical_demand() {
    let fava = fava_with(StandardSubscriptionPlanner::default(), scripted_relay_eose_once());
    let a = fava.observe(live_tag_query("x")).await.unwrap();
    let b = fava.observe(live_tag_query("y")).await.unwrap();  // groups with a
    scripted_relay_eose_once().emit_eose_for_single_wire_subscription();
    assert!(a.current().evidence.eose_from(relay_key()));
    assert!(b.current().evidence.eose_from(relay_key()), "grouped EOSE lost logical attribution");
}
```

**Confidence.** confirmed.

---

### validate-plan-private-conformance — critical — replaceability

**Authority.** `docs/spec/ARCHITECTURE.md:1514` — `fava-subscriptions` owns
"the conformance rules that define semantic equivalence."
`docs/spec/ARCHITECTURE.md:3148` — an external provider must "pass the same
conformance kit as the standard provider." `docs/spec/ARCHITECTURE.md:3653`
lists `fava-subscriptions-testkit`. Gate 3: "defaults have no private bypass;
a competing implementation can use the public contract to achieve the same
result."

**Implementation.** `crates/fava/src/relay.rs:224-248`. The conformance rules
live privately in the *consumer*, not in the contract crate, are enforced by a
`Result<(), String>`, and are documented nowhere. `fava-subscriptions-testkit`
does not exist (`ls crates` verified).

Enumerated assumptions, with whether each is specified:

| # | Assumption (relay.rs line) | Specified anywhere? |
|---|---|---|
| 1 | `plan.relay == expected` (225) | **Yes** — ARCH:1518 "Routing has already selected the relay"; legitimate scoping check |
| 2 | `!plan.attribution.is_empty()` (226) | **No** — and it makes a withdrawal-only plan (all demand retracted) structurally impossible, contradicting ARCH:1513 "withdrawal identity" |
| 3 | `!plan.messages.is_empty()` (227) | **No** — same consequence; a diff plan whose only content is `close` is refused |
| 4 | `plan.demand.keys() == plan.attribution.keys()` exactly (228) | **No** — forbids attributing a wire subscription that serves demand recorded under a different key, and forbids retaining installed subscriptions with no new attribution entry |
| 5 | no `plan.demand` value is empty (229) | **No** |
| 6 | every message is `ClientMessage::Req` (234-240) | **No** — and it forbids a planner from emitting `CLOSE`, `AUTH`, or `COUNT` as part of a plan. The spec's plan type is `wire: Vec<PlannedSubscription>` (ARCH:1500), not `Vec<ClientMessage>`, precisely so this decision is not the facade's |
| 7 | each REQ carries **exactly one** filter (241) | **No** — NIP-01 permits `["REQ", id, f1, f2, ...]`. A competing planner that expresses "these three filters serve one subscription" is refused outright. Nothing in ARCHITECTURE.md or RELAY-002/003/004 requires one filter per REQ |
| 8 | `plan.attribution[wire_id] == filters[0]` (242) | **No** — combined with #7 this hardcodes a one-filter attribution model. The spec's `SubscriptionAttribution` (ARCH:1501) is a named type precisely because attribution is richer than one filter |
| 9 | refusal is an untyped `String` (231, 239, 244) that is then `.to_string()`-ed again at live.rs:51 / routes.rs:162 | **No** — contradicts gate 4 (attributable failure) and OPS-001 |

Assumptions 2, 3, 6, 7, and 8 are the replaceability gate failures: a planner
that is *correct under the specified contract* is rejected by the facade.

**Observable distinction.** Compose `Fava::builder().subscription_planner(...)`
with a planner that emits one REQ carrying two equivalent filters. Every
`Fava::observe` against that relay fails with
`ObserveError::Relay("subscription planner attribution does not match its
REQ")` — a wire shape the Nostr protocol and the Fava contract both allow.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn a_multi_filter_req_planner_is_accepted() {
    let fava = fava_with(MultiFilterReqPlanner, scripted_transport());
    let obs = fava.observe(live_query_two_kinds()).await;
    assert!(obs.is_ok(), "facade rejected a NIP-01-legal plan: {:?}", obs.err());
}
```

**Confidence.** confirmed.

---

### no-nip11-invented-planner-limits — critical — boundedness

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1055` —
"When fresh NIP-11 information advertises read/write limits that Fava can
interpret deterministically, planning ... MUST either honor them or surface
exact source-scoped shortfall", including "maximum subscriptions; maximum
message length; subscription-id length; maximum/default filter limits".
`:1069` — "Missing, stale, malformed, or unsupported claims remain unknown
rather than **becoming invented defaults**."
`docs/spec/ARCHITECTURE.md:1534` — the standard planner must "account for
NIP-11 message-size, subscription-count, subscription-id, **default-limit**,
and result-limit constraints".

**Implementation.** `crates/fava-subscriptions-standard/src/lib.rs:19-26`:

```rust
impl Default for StandardSubscriptionPlanner {
    fn default() -> Self {
        Self::bounded(
            NonZeroUsize::new(64).expect("constant is non-zero"),
            NonZeroUsize::new(1_048_576).expect("constant is non-zero"),
        )
    }
}
```

64 and 1 MiB are invented, are fixed at construction, and apply identically to
every relay the engine ever contacts — the `plan()` signature has no
per-relay constraints argument (`crates/fava-subscriptions/src/lib.rs:60-64`).
`grep -rn "nip11|NIP-11|RelayInformation" crates` returns zero hits: no NIP-11
document is ever acquired, so no advertised limit could be honored even in
principle. Neither subscription-id length nor default filter limit is modelled
at all — `SubscriptionPlanError` (`crates/fava-subscriptions/src/lib.rs:69-95`)
has no variant for either.

The **default-limit** gap is a meaning bug, not only a missing feature.
`merge_candidate` (`crates/fava-subscriptions-standard/src/lib.rs:119-128`)
refuses to merge only when an *explicit* `limit` is present:

```rust
if left.limit.is_some() || right.limit.is_some() { return None; }
```

A relay that applies its own default result limit per REQ will apply it once
to the merged union rather than once per logical filter, so 300 grouped
tag-value queries can return fewer events than 300 ungrouped ones. That is
exactly RELAY-003's prohibited case: "combinations whose local refiltering
cannot reproduce the original result/evidence"
(`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1049`).

**Observable distinction.** Point Fava at a relay advertising
`limitation.max_message_length: 512` and open a query whose REQ encodes to 900
bytes. Fava hands the frame off with no shortfall, because the 1 MiB default
is invented rather than unknown. Symmetrically, against a relay with a default
result limit of 100, a grouped 300-value query returns strictly fewer events
than the same demand run through `fava-subscriptions-no-grouping` — which
Falsifier J (`docs/spec/ARCHITECTURE.md:3329`) forbids.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn advertised_message_length_produces_shortfall_not_an_oversized_req() {
    let relay = scripted_relay_with_nip11(json!({"limitation":{"max_message_length":512}}));
    let fava = fava_with(StandardSubscriptionPlanner::default(), relay.transport());
    let err = fava.observe(query_encoding_to_900_bytes()).await.unwrap_err();
    assert!(matches!(err, ObserveError::SubscriptionShortfall(_)));
    assert!(relay.frames().is_empty(), "oversized REQ was handed off anyway");
}
```

**Confidence.** confirmed.

---

### subscription-shortfall-untyped-and-misattributed — critical — failure isolation

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1067` — "Fava
MUST NOT silently truncate, clamp, collide identifiers, or **claim omitted
work was completed**." `:1391-1394` — diagnostics must expose facts about
"query demand and shortfall" and "resource limits or explicit loss".
`docs/spec/ARCHITECTURE.md:2317` — diagnostics inputs include
"subscription-plan limits"; `:2331` — output carries
`limits: Vec<LimitDiagnostic>`. `docs/spec/ARCHITECTURE.md:3331` — "A planner
that silently drops demand to fit relay limits fails."

**Implementation.** The typed `SubscriptionPlanError::TooManySubscriptions {
required, maximum }` / `FrameTooLarge { bytes, maximum }` is destroyed at both
consumption sites:

- `crates/fava/src/relay.rs:182` — `.map_err(|error| error.to_string())?`,
  then `crates/fava/src/live.rs:51` — `ObserveError::Relay(error)`. The
  explicit path loses `required`/`maximum` and the fact that this was a
  *planning* refusal rather than a socket failure.
- `crates/fava/src/routes.rs:160-162` — the automatic path files it as a
  **route** shortfall:

  ```rust
  Err(error) => providers
      .diagnostics
      .route_shortfall(revision, format!("{relay:?}: {error}")),
  ```

  A subscription-plan limit is recorded under `route_shortfalls: Vec<(u64,
  String)>` (`crates/fava-diagnostics/src/lib.rs:24`), keyed by route revision,
  as free text. `Fava::observe` then returns `Ok(observation)`
  (`crates/fava/src/routes.rs:61`) — a live handle for a query whose demand
  never reached that relay. The application is told the query is open; nothing
  in the returned `Observation` records the missing source.

There is no `limits` category in `DiagnosticsSnapshot`
(`crates/fava-diagnostics/src/lib.rs:17-40`) and no subscription-scoped
shortfall category at all.

**Observable distinction.** With an automatic-routing query over three relays
where one plan refuses, the application receives an `Ok` observation, three
route destinations recorded at `diagnostics().routes`, but only two relays
actually subscribed — and the only trace is a `Debug`-formatted string in
`route_shortfalls` that cannot be matched, counted, or attributed to the
subscription planner rather than to a router.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn a_refused_subscription_plan_is_typed_and_attributed_to_the_relay() {
    let fava = fava_with(planner_refusing_relay_b(), three_relay_router());
    let obs = fava.observe(live_query()).await.unwrap();
    let short = fava.diagnostics().limits;
    assert!(short.iter().any(|l| l.relay == relay_b() && l.required == 2 && l.maximum == 1),
        "subscription shortfall was stringified into route_shortfalls: {:?}", fava.diagnostics());
    assert!(obs.current().evidence.missing_source(&relay_b()));
}
```

**Confidence.** confirmed.

---

### too-many-subscriptions-inert — major — boundedness

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1424-1428` —
OPS-004 requires bounds for "active relay sessions" and "**wire
subscriptions**"; "Exceeding a bound MUST produce refusal, backpressure, or
exact shortfall."

**Implementation.** `crates/fava-subscriptions-standard/src/lib.rs:67-72`
compares `groups.len()` against `max_subscriptions` — but `groups.len()` is
derived only from the demand slice handed to that one call, and
`crates/fava/src/relay.rs:181` always hands over a one-element slice
(`singleton-demand-per-plan`). `required` is therefore always 1 and the bound
is arithmetically unreachable through `Fava::observe`. Nothing else in the
workspace counts wire subscriptions per relay session: `OpenedRelay`
(`crates/fava/src/relay.rs:17-27`) holds one attribution map per query and is
constructed independently per observation.

**Observable distinction.** Open 1,000 live queries against one relay with
`StandardSubscriptionPlanner::bounded(NonZeroUsize::new(4)?, ..)`. Fava opens
1,000 sessions and 1,000 wire subscriptions and never produces
`TooManySubscriptions`. The configured bound has no effect on any observable
behaviour.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn the_configured_wire_subscription_bound_actually_refuses() {
    let fava = fava_with(StandardSubscriptionPlanner::bounded(nz(2), nz(1<<20)), scripted());
    let _a = fava.observe(distinct_live_query(0)).await.unwrap();
    let _b = fava.observe(distinct_live_query(1)).await.unwrap();
    let third = fava.observe(distinct_live_query(2)).await;
    assert!(third.is_err(), "wire-subscription bound never fires; {} REQs sent", scripted().req_count());
}
```

**Confidence.** confirmed.

---

### grouping-collides-wire-and-logical-identity — major — failure isolation

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1067` — "Fava
MUST NOT ... **collide identifiers**". `docs/spec/ARCHITECTURE.md:1493-1494` —
`RelayDemand` identity is `owner: ObservationId` / `branch: QueryBranchId`,
distinct from the wire `SubscriptionId` the plan allocates.

**Implementation.** `crates/fava-subscriptions-standard/src/lib.rs:60-64`:

```rust
groups.push(Group {
    wire_id: item.subscription_id.clone(),
    filter: item.filter.clone(),
    logical: vec![item.subscription_id.clone()],
});
```

The wire subscription id **is** the first logical demand's id. After grouping
300 demands, wire id `tag-logical-000` serves 300 logical queries, and
`plan.demand["tag-logical-000"] = ["tag-logical-000", ..., "tag-logical-299"]`
— the key is also an element of its own value. `RelayDemand.subscription_id`
is typed `fava_wire::SubscriptionId`
(`crates/fava-subscriptions/src/lib.rs:15`), so the type system cannot keep the
two apart.

**Observable distinction.** Once refcounted withdrawal exists, closing the
observation that happened to be planned first would `CLOSE` the wire id that
299 other observations still depend on. Even today it is externally visible:
`Fava::diagnostics().subscriptions` reports a wire subscription id that is
indistinguishable from a logical demand id, so an application cannot tell
whether it is looking at one query's subscription or a shared one.

**Proposed falsifier.**

```rust
#[test]
fn grouped_wire_ids_are_disjoint_from_logical_demand_ids() {
    let demand = three_compatible_author_demands();
    let plan = StandardSubscriptionPlanner::default().plan(&relay(), &demand).unwrap();
    let logical: BTreeSet<_> = demand.iter().map(|d| d.subscription_id.clone()).collect();
    assert!(plan.attribution.keys().all(|w| !logical.contains(w)),
        "wire id reuses a logical demand id: {:?}", plan.attribution.keys());
}
```

**Confidence.** confirmed.

---

### grouping-unprovable-through-observe — critical — behavioral proof

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1051` —
RELAY-003 acceptance: "**300 compatible tag-value queries may share one wire
request** while each logical query retains exact matching and evidence."
`docs/spec/ARCHITECTURE.md:3321-3331` — Falsifier J requires running "the same
routed logical demand" through no-grouping, standard grouping, and an
alternative, and observing identical results/evidence/EOSE
attribution/cancellation/shortfalls.
`FAVA_TDD_BDD_TESTING_GUIDE.md:281` — "Use differential tests: the grouped wire
plan and an ungrouped reference plan must produce identical logical query
results and evidence."
`FAVA_TDD_BDD_TESTING_GUIDE.md:342` — "Diagnostics report what Fava believes.
They do not prove their own claims."

**Implementation.** The contract does not admit both implementations through
the public path, and grouping's behaviour is **not** provable through
`Fava::observe`:

1. `StandardSubscriptionPlanner` is never passed to `Fava::builder()` in any
   test under `crates/fava/tests/` — every one of the five wiring sites uses
   `fava_subscriptions_no_grouping::planner()`
   (`crates/fava/tests/explicit_live.rs:195,245,390`,
   `crates/fava/tests/multi_relay.rs:385`,
   `crates/fava/tests/automatic_routes.rs:256`).
2. The single `plan()` call site is fed one demand
   (`crates/fava/src/relay.rs:181`), so standard and no-grouping are
   *observationally identical by construction* — grouping code is dead through
   the facade.
3. The RELAY-003 acceptance evidence, `apps/canary/src/grouping.rs`,
   **reimplements the entire relay path outside Fava**:
   - `apps/canary/src/grouping.rs:248` — `planner.plan(&key, demand)` called
     directly with a 300-element slice the facade could never build;
   - `:249-252` — `WebSocketTransport::default().open_session(key)` opened
     directly;
   - `:253-258` — frames encoded and sent by the canary;
   - `:291-300` — `admit_subscription_event(cache, .., &id, &id, filter, ..)`
     called by the canary, again collapsing wire and logical id;
   - `:261-264` — CLOSE and session close performed by the canary;
   - `:348-353` — a **second, separate** `Fava` is then built with **no
     transport and no subscription planner**, over the pre-filled
     `MemoryEventCache`, and the 300 queries observed there are
     `.cache_only()` (`:175`) — i.e. they contribute no relay demand at all.

   So the only evidence for RELAY-003 exercises `Fava::observe` purely as a
   cache reader. Not one byte of the grouped wire path crosses the public
   surface. This is precisely the failure mode the brief names: evidence
   written to match the implementation instead of the authority.
4. `apps/canary/src/grouping.rs:474-481` bakes the facade's private
   one-filter-per-REQ assumption into the evidence itself: "planner REQ must
   contain one subscription id and one filter".
5. No differential test exists: `grep -rln no_grouping crates` returns only the
   no-grouping crate's own test plus the three `fava` test files, none of which
   compares planners.

**Observable distinction.** Swap `StandardSubscriptionPlanner` for
`fava_subscriptions_no_grouping::planner()` in any real application assembly:
the observable wire shape, results, and evidence are byte-identical, because
the facade never gives grouping anything to group. Falsifier J's premise —
"wire shapes may differ" — is false today.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn three_hundred_tag_queries_share_one_req_through_observe() {
    let script = scripted_relay();
    let fava = fava_with(StandardSubscriptionPlanner::default(), script.transport());
    let mut obs = Vec::new();
    for i in 0..300 { obs.push(fava.observe(live_tag_query(i)).await.unwrap()); }
    assert_eq!(script.frames_matching("REQ").len(), 1, "grouping never reached the wire");
    for (i, o) in obs.iter().enumerate() { assert_eq!(o.current().events.len(), 1, "query {i}"); }
}
```

**Confidence.** confirmed.

---

### diagnostics-snapshot-shape — critical — ownership

**Authority.** `docs/spec/ARCHITECTURE.md:2307` — "expose bounded, current,
**typed** facts from Fava owners". `:2326-2332`:

```rust
pub struct DiagnosticsSnapshot {
    pub relays: Vec<RelayDiagnostic>,
    pub queries: Vec<QueryDiagnostic>,
    pub writes: Vec<WriteDiagnostic>,
    pub providers: Vec<ProviderDiagnostic>,
    pub limits: Vec<LimitDiagnostic>,
}
```

`:2311-2321` — inputs include "open observation and route ownership; relay-
session state and reason; source shortfalls; router unresolved needs;
subscription-plan limits; stalled write reasons; signer and auth availability;
cache and write-store failures; bounded counts and high-water facts."
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1389-1398` — OPS-001.

**Implementation.** `crates/fava-diagnostics/src/lib.rs:16-40` — eleven flat
`Vec`s of anonymous tuples of primitives
(`crates/fava-diagnostics/src/lib.rs:10-13`):

```rust
type SessionFact      = (RelaySessionKey, u64);
type SubscriptionFact = (RelaySessionKey, u64, SubscriptionId);
type MessageFact      = (RelaySessionKey, u64, SubscriptionId, String);
type FailureFact      = (RelaySessionKey, u64, String);
```

None of the five specified categories exists; no named diagnostic type exists.

**Crisis-report claims, checked one by one:**

| Claim | Verdict | Evidence |
|---|---|---|
| Cannot express **open observation identity** | **Confirmed** | No observation id appears anywhere in `crates/fava-diagnostics/src/lib.rs`; `fava-observe` has no observation identity to publish and never calls `Diagnostics` |
| Cannot express **observation-to-route binding** | **Confirmed** | `routes: Vec<(u64, Vec<RelaySessionKey>)>` (`:23`) is keyed by an anonymous route revision with no query or observation identity; with two live automatic queries the revisions interleave indistinguishably |
| Cannot express **logical demand** | **Confirmed** | No filter, no query, and no logical demand id is retained. `subscriptions: Vec<(RelaySessionKey, u64, SubscriptionId)>` (`:29`) carries the wire id only — the plan's `attribution` filters and `demand` map are never passed in (`crates/fava/src/relay.rs:208-210`) |
| Cannot express **desired plan** | **Confirmed** | Only accepted-after-handoff wire ids are recorded (`crates/fava/src/relay.rs:208`, after the send loop). There is no desired-vs-installed distinction, no plan revision, and no record of a plan that was computed but not installed |
| Cannot express **shared-work refcount** | **Confirmed** | No count field of any kind besides `coalesced_query_updates: u64` (`:19`) |
| Cannot express **source shortfall** | **Confirmed with nuance** | `route_shortfalls: Vec<(u64, String)>` (`:25`) exists but is *route-revision* scoped, untyped, and is the dumping ground for subscription-plan refusals too (`crates/fava/src/routes.rs:162`). There is no per-relay-session, per-query, source-scoped shortfall, and no `limits` category |
| Cannot express **current provider operation** | **Confirmed** | No `providers` category. `router_sessions: Vec<String>` (`:21`) records router *names* at open (`crates/fava/src/routes.rs:27`) and nothing else. No signer, publisher, cache, write-store, or transport operation is ever reported |
| **Retention is bounded** | **Confirmed for count, refuted for size** — see `diagnostics-unbounded-fact-payloads` | `push_bounded` (`:215-223`) caps each `VecDeque` at `capacity` (default 256, `:65`) |

Additionally missing against the spec inputs: stalled write reasons (OPS-003
has no data path — `DiagnosticsSnapshot` has no `writes` field and
`fava-publication` never calls `Diagnostics`), signer/auth availability, and
cache/write-store failures.

**Observable distinction.** An application with two concurrent live queries
over the same three relays cannot determine, from
`Fava::diagnostics()`, which query is responsible for which relay session,
which subscription belongs to which query, or which query is short of a
source. Every fact is either global or relay-keyed.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn diagnostics_attribute_each_relay_session_to_its_observation() {
    let fava = fava_with(planner(), two_relay_scripted());
    let a = fava.observe(live_query_a()).await.unwrap();
    let _b = fava.observe(live_query_b()).await.unwrap();
    let snap = fava.diagnostics();
    let for_a: Vec<_> = snap.queries.iter().filter(|q| q.observation == a.id()).collect();
    assert_eq!(for_a.len(), 1);
    assert_eq!(for_a[0].relays.len(), 2, "cannot bind observation to routes: {snap:?}");
}
```

**Confidence.** confirmed.

---

### diagnostics-facade-sole-producer — critical — ownership

**Authority.** `docs/spec/ARCHITECTURE.md:2311` — "**Each owner** publishes
structured diagnostic facts", followed by facts belonging to observe,
routing, subscriptions, publication, session/signer, auth, event cache, and
write store. `docs/spec/ARCHITECTURE.md:2989` — "Current diagnostic snapshot |
`fava-diagnostics` | facade/SDK observers" (the facade is a *consumer*).

**Implementation.** `grep -rn "fava_diagnostics|Diagnostics" crates` shows the
only crate that depends on `fava-diagnostics` at all is `fava`
(`crates/fava/Cargo.toml:8`), and every mutating call originates in
`crates/fava/src/relay.rs` and `crates/fava/src/routes.rs`. Neither
`fava-observe`, `fava-publication`, `fava-ingest`, `fava-routing`,
`fava-transport`, `fava-signer`, nor any store crate publishes a fact.
`Diagnostics` is constructed at `crates/fava/src/lib.rs:402` and read at
`crates/fava/src/lib.rs:226-228`.

This is the same deviation as the known-good baseline, one layer down: because
the facade privately owns relay establishment, it is also the only thing with
facts to report, so diagnostics reports *facade-private* state rather than
owner state. It also means the diagnostics contract cannot be satisfied
without first moving ownership — the two findings are coupled.

**Observable distinction.** No publication, signing, routing-internal, cache,
or write-store fact is ever visible through `Fava::diagnostics()`, so OPS-003
(inspect every currently stuck write under one classification) has no
implementation path at all.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn a_stalled_write_appears_in_diagnostics_without_a_receipt_stream() {
    let fava = fava_with_unresolvable_router_and_no_signer();
    let _w = fava.publish(note("hello")).unwrap();
    let stuck = fava.diagnostics().writes;
    assert_eq!(stuck.len(), 1);
    assert!(matches!(stuck[0].classification, WriteStall::Unsignable));
}
```

**Confidence.** confirmed.

---

### diagnostics-unbounded-fact-payloads — major — boundedness

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1420-1437` —
OPS-004 requires explicit bounds for "frame and message sizes" and
"**diagnostics**"; "Exceeding a bound MUST produce refusal, backpressure, or
exact shortfall." `docs/spec/ARCHITECTURE.md:2307` — "**bounded**, current,
typed facts".

**Implementation.** The count bound is real
(`crates/fava-diagnostics/src/lib.rs:215-223`, default 256 per category at
`:65`), but every string payload is unbounded and externally supplied:

- `MessageFact = (.., String)` (`:12`) stores the relay's verbatim `CLOSED`
  message (`crates/fava/src/relay.rs:295`);
- `FailureFact = (.., String)` (`:13`) stores `format!("relay NOTICE:
  {message}")` (`crates/fava/src/relay.rs:302`), `format!("invalid relay
  message: {error}")` (`:112`), and decoded-frame error text;
- `route_shortfalls: VecDeque<(u64, String)>` (`:53`) stores
  `format!("{relay:?}: {error}")` (`crates/fava/src/routes.rs:162`);
- `router_sessions: VecDeque<String>` (`:52`) stores application-supplied
  router names (`crates/fava/src/routes.rs:27`).

`crates/fava-transport-websocket/src/lib.rs:110` enforces `max_frame_bytes`
only on **outbound** sends; the inbound path
(`crates/fava-transport-websocket/src/lib.rs:146`,
`Some(Ok(Message::Text(text))) => return Ok(text.to_string())`) applies no
Fava-declared bound, so a hostile relay's NOTICE flows straight into retained
diagnostics. Retention is therefore `256 x (unbounded)` per category, which is
not a bound. `Diagnostics::bounded` is also not reachable from
`Fava::builder()` — `crates/fava/src/lib.rs:402` hardcodes
`Diagnostics::default()` and no builder method exists.

**Observable distinction.** A relay sending 256 NOTICEs of 8 MiB each causes
Fava to retain ~2 GiB of relay-controlled text with no refusal, shortfall, or
truncation marker, and the application cannot configure the bound.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn hostile_relay_text_is_bounded_in_retained_diagnostics() {
    let fava = fava_with(planner(), scripted_relay_sending_notice(8 * 1024 * 1024));
    let _o = fava.observe(live_query()).await.unwrap();
    let snap = fava.diagnostics();
    assert!(snap.failures.iter().all(|f| f.2.len() <= 4096),
        "unbounded relay text retained: {} bytes", snap.failures[0].2.len());
}
```

**Confidence.** confirmed.

---

### diagnostics-untyped-failure-bucket — major — failure isolation

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1389` — OPS-001
requires "bounded, **queryable** facts" in distinct named categories.
`:1142` — RELAY-012: hostile relay behaviours "MUST remain scoped to the exact
relay/session/request". `:1107` — RELAY-008: relay text must remain verbatim
evidence, i.e. distinguishable from Fava's own explanations.

**Implementation.** `FailureFact = (RelaySessionKey, u64, String)`
(`crates/fava-diagnostics/src/lib.rs:13`) is the single bucket for at least
eight semantically different events, all funnelled through
`Diagnostics::failed`:

| Event | Site |
|---|---|
| transport read error | `crates/fava/src/relay.rs:89-93` |
| undecodable frame | `crates/fava/src/relay.rs:109-114` |
| unattributed EVENT / EOSE / CLOSED | `crates/fava/src/relay.rs:266, 286, 297` |
| invalid signature / off-filter event (ingest refusal) | `crates/fava/src/relay.rs:278` |
| relay `NOTICE` — an informational relay message, not a failure | `crates/fava/src/relay.rs:302` |
| reconnect refusal | `crates/fava/src/relay.rs:161-165` |
| CLOSE encode failure and CLOSE handoff failure during withdrawal | `crates/fava/src/relay.rs:320-326, 333` |
| session close error | `crates/fava/src/relay.rs:338-342` |

An application cannot distinguish "the relay said something" from "the relay
sent me a forged event" from "my socket died" from "my own CLOSE failed to
hand off" except by substring-matching Fava-authored prose. RELAY-008's
verbatim relay text is concatenated with Fava's invented prefix (`"relay
NOTICE: "`, `"invalid relay message: "`), so the two are no longer separable.

**Observable distinction.** Under RELAY-012's hostile-relay scenarios, the
application sees one undifferentiated `failures` list. `apps/canary/src/hostile.rs:77`
already demonstrates the consequence: the hostile-relay canary asserts only
`!diagnostics.failures.is_empty()`, because there is nothing more specific to
assert.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn a_notice_is_not_reported_as_a_session_failure() {
    let fava = fava_with(planner(), scripted_relay_sending_notice("rate limited"));
    let _o = fava.observe(live_query()).await.unwrap();
    let snap = fava.diagnostics();
    assert!(snap.failures.is_empty(), "NOTICE was filed as a failure");
    assert_eq!(snap.notices.first().map(|n| n.text.as_str()), Some("rate limited"));
}
```

**Confidence.** confirmed.

---

### diagnostics-not-a-current-state-stream — major — behavioral proof

**Authority.** `docs/spec/ARCHITECTURE.md:2335` — "The output is a bounded
**latest-state stream**."
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1402-1406` — OPS-002:
"Diagnostics current-state **observations** MAY coalesce bursts into one exact
latest snapshot. **Opening diagnostics mid-burst returns current truth rather
than replaying stale intermediates.** With no diagnostics observer, Fava
SHOULD avoid constructing expensive presentation snapshots."

**Implementation.** The entire public surface is
`crates/fava/src/lib.rs:226-228`:

```rust
pub fn diagnostics(&self) -> DiagnosticsSnapshot { self.diagnostics.snapshot() }
```

There is no observer, subscription, or change stream — an application must
poll. Worse, the retention model is the inverse of the specified one: `push_bounded`
(`crates/fava-diagnostics/src/lib.rs:215-223`) keeps a **replay log** of the
last 256 *historical* facts per category (every EOSE, every CLOSED, every
session open), which is exactly the "stale intermediates" OPS-002 says an
opening observer must not receive. And because every fact is recorded
unconditionally at the call site, there is no observer-presence check: Fava
constructs and retains diagnostics even when no application ever calls
`diagnostics()`.

**Observable distinction.** An application cannot react to a diagnostic change
without a polling loop, and when it does poll it receives a historical log
rather than current state — e.g. `sessions` contains relay sessions that have
already been closed and withdrawn, with no way to tell which are still open.

**Proposed falsifier.**

```rust
#[tokio::test]
async fn diagnostics_report_current_sessions_not_a_history_log() {
    let fava = fava_with(planner(), scripted());
    let obs = fava.observe(live_query()).await.unwrap();
    drop(obs);                                   // observation closes, session withdraws
    settle().await;
    assert!(fava.diagnostics().relays.is_empty(),
        "closed session still reported as current: {:?}", fava.diagnostics().relays);
}
```

**Confidence.** confirmed.

---

### no-grouping-planner-is-nameless — minor — vocabulary

**Authority.** `docs/spec/ARCHITECTURE.md:57` —
`.subscription_planner(NoGroupingPlanner::new())`; `:38` —
`.subscription_planner(StandardSubscriptionPlanner::new())`.
`docs/spec/ARCHITECTURE.md:3153` — "a no-grouping subscription planner" is a
required early external-provider example.
`AGENTS.md` vocabulary policy: a public provider implementation is a
vocabulary noun; `tools/check_vocabulary.py` only scans
`pub struct|enum|trait|type` and is blind to this case.

**Implementation.** `crates/fava-subscriptions-no-grouping/src/lib.rs:11` —
`struct OnePerDemand;` is private; `:54` —
`pub const fn planner() -> impl SubscriptionPlanner`. The crate exports **no
nominal type at all**, so the specified `NoGroupingPlanner` does not exist and
an application cannot name the planner in a type position, store it in a
struct field, or write a `where` bound over it — unlike the standard planner.
`docs/internals/vocabulary.toml:676-680` lists
`fava_subscriptions_standard::StandardSubscriptionPlanner` but **zero**
symbols from `fava-subscriptions-no-grouping`, even though the crate is listed
at `:684`. Also note `StandardSubscriptionPlanner::new()` (spec) is
`::default()` / `::bounded()` in the implementation.

**Observable distinction.** Code written against the ARCHITECTURE.md builder
example does not compile. An application that wants to hold
`Vec<Box<dyn SubscriptionPlanner>>` alongside a named no-grouping planner
cannot express the type.

**Proposed falsifier.**

```rust
#[test]
fn the_no_grouping_planner_has_a_public_name() {
    let planner: fava_subscriptions_no_grouping::NoGroupingPlanner =
        fava_subscriptions_no_grouping::NoGroupingPlanner::new();
    assert!(planner.plan(&relay(), &[one_demand()]).is_ok());
}
```

**Confidence.** confirmed.

---

### standard-planner-group-homonym — minor — vocabulary

**Authority.** `AGENTS.md` vocabulary policy: "a synonym, wrapper, alternate
representation, or adjective-qualified variant of an existing noun" is a
vocabulary change; `tools/check_vocabulary.py` is blind to private types.
`docs/internals/vocabulary.toml:191-215` defines `Group` as an approved noun
owned by `fava-simple-groups`: "A relay-based Nostr group; Fava's Group value
presents one opaque group id over an application-selected non-empty host-relay
set."

**Implementation.** `crates/fava-subscriptions-standard/src/lib.rs:101-105`:

```rust
struct Group {
    wire_id: SubscriptionId,
    filter: Filter,
    logical: Vec<SubscriptionId>,
}
```

A private homonym of an approved cross-crate noun, meaning "a set of merged
filters" — an unrelated concept in the same workspace. Private, so the
vocabulary gate does not see it.

**Observable distinction.** None externally; this is a cohesion/vocabulary
finding only, reported because the brief asks for private nouns the gate is
blind to. `FilterGroup` or `MergedSubscription` would carry no collision.

**Proposed falsifier.** Extend `tools/check_vocabulary.py` to scan
`^\s*(pub\(crate\)|pub\(super\))?\s*(struct|enum|trait|type)` and fail on any
name equal to an approved term owned by a different crate; that check fails on
`crates/fava-subscriptions-standard/src/lib.rs:101` today.

**Confidence.** confirmed.

---

## Conforming (verified, not merely unexamined)

These were checked against the authority and found correct. Absence claims come
from searches recorded in **Scope checked**.

- **Contract/implementation dependency direction** (`ARCHITECTURE.md:3066`).
  `crates/fava-subscriptions/Cargo.toml` depends on `fava-query`, `fava-state`,
  `fava-wire`, `nostr`, `thiserror` — and on **neither** standard planner.
  Both planner crates depend on the contract crate, never the reverse. No
  cycle.
- **Relay-access isolation in grouping** (RELAY-003, "relay access").
  `RelaySessionKey` carries `RelayAccess`, and `plan()` is scoped to one key
  (`crates/fava-subscriptions-standard/src/lib.rs:40-44`), so demand under
  different access can never be merged. Structurally correct.
- **Time-window and limit merge refusal** (RELAY-003, "incompatible time
  windows, relay-side limits").
  `merge_candidate` (`crates/fava-subscriptions-standard/src/lib.rs:123`)
  refuses any pair where either side carries an explicit `limit`, and both
  `merge_author_axis` (`:135`) and `merge_tag_axis` (`:152`) require the entire
  remaining `Filter` — including `since`/`until` — to be equal. Verified
  against `crates/fava-subscriptions-standard/tests/grouping.rs:118-181`, which
  covers limit, author+tag multi-axis, two tag axes, opposite-case tag keys,
  present-empty axis, and unequal `search`. (The *relay default* limit remains
  unhandled — reported above as `no-nip11-invented-planner-limits`.)
- **Tag-key case sensitivity.** `#e` and `#E` are never merged
  (`merge_tag_axis` keys on `SingleLetterTag`, which is case-sensitive);
  covered by `grouping.rs:148-152` and by
  `crates/fava-subscriptions/tests/tag_values.rs:17-26`.
- **Present-empty axis semantics.** A present empty tag set survives to the
  wire as `{"#p":[]}` (`crates/fava-subscriptions/tests/tag_values.rs:29-36`)
  and blocks merging (`grouping.rs:153-157`), matching
  `crates/fava-query/src/selection.rs:18` ("A present empty set matches
  nothing").
- **Canonical `demand_for_query` encoding.** Repeated and reordered kinds and
  tag values encode identically (`kinds.rs:8-17`, `tag_values.rs:39-54`),
  supporting QUERY-002 stable identity.
- **`demand_for_query` completeness against today's `Query`.** I checked
  `crates/fava-query/src/selection.rs:9-20`: the only selection axes are
  `ids`, `authors`, `kinds`, `tag_values`, plus `result_limit`. All five are
  mapped at `crates/fava-subscriptions/src/lib.rs:100-115`. Nothing is dropped.
  `Query` has no `since`/`until` yet (OPEN-001 is still open), so QUERY-016 is
  not yet violable here.
- **Duplicate-demand refusal.** Both planners refuse duplicate
  `subscription_id` (`fava-subscriptions-standard/src/lib.rs:107-117`;
  `fava-subscriptions-no-grouping/src/lib.rs:26-33`), satisfying RELAY-004's
  "MUST NOT ... collide identifiers" *within one plan*. (Across grouped plans
  it is violated — see `grouping-collides-wire-and-logical-identity`.)
- **Frame-size refusal is exact.** `FrameTooLarge { bytes, maximum }` reports
  the actual encoded length, not an estimate
  (`fava-subscriptions-standard/src/lib.rs:79-87`), verified byte-exactly by
  `grouping.rs:226-238`. The value is typed and precise at the contract; only
  its transport to the application is lossy.
- **Planner purity.** Neither planner opens a socket, spawns a task, holds a
  lock, or mutates observation state — both are pure functions of
  `(relay, demand)`. `ARCHITECTURE.md:1520` is satisfied *by the planner
  crates*. The violation of that line lives entirely in the consumer
  (`crates/fava/src/relay.rs`), which is the known-good baseline.
- **Diagnostics per-category count bound.** `push_bounded`
  (`crates/fava-diagnostics/src/lib.rs:215-223`) is correct: it dedupes an
  equal existing entry, pops the front at capacity, and pushes at the back.
  Default 256 per category (`:65`). Count is genuinely bounded; payload size is
  not.
- **Diagnostics lock hygiene.** `Diagnostics::lock`
  (`crates/fava-diagnostics/src/lib.rs:208-212`) recovers from poisoning via
  `PoisonError::into_inner`, so a panic inside one recorder cannot wedge
  unrelated diagnostic recording (gate 4).
- **`coalesced_query_updates` overflow.** `saturating_add`
  (`crates/fava-diagnostics/src/lib.rs:101`) — no panic path.
- **Vocabulary registration of the approved nouns.** `RelayDemand`,
  `SubscriptionPlan`, `SubscriptionPlanError`, `SubscriptionPlanner`,
  `StandardSubscriptionPlanner`, `Diagnostics`, and `DiagnosticsSnapshot` are
  all present in `docs/internals/vocabulary.toml:676-680, 864-865` under owners
  matching the architecture.
- **No private lifecycle owner in the four scoped crates.** I enumerated every
  `struct`/`enum` in scope: `RelayDemand`, `SubscriptionPlan`,
  `SubscriptionPlanError`, `StandardSubscriptionPlanner`, `Group` (private),
  `OnePerDemand` (private), `Diagnostics`, `DiagnosticsSnapshot`, `State`
  (private field-bag of `Diagnostics`). Only `Diagnostics` owns a lifecycle,
  and it is an approved noun. `Group` and `OnePerDemand` are reported above as
  vocabulary-only findings, not lifecycle owners. The unapproved private
  lifecycle owner in this call graph is `fava::OpenedRelay`, which is already
  the known-good baseline.

---

## Open questions

1. **Where does `demand_for_query` belong?** It lives in the contract crate
   (`crates/fava-subscriptions/src/lib.rs:99-116`) and forces
   `fava-subscriptions` to depend on `fava-query`, but the ownership ledger
   assigns "Query demand for one relay" to `fava-observe`
   (`ARCHITECTURE.md:2978`). I could not construct an observable distinction,
   so I did not raise it as a finding — but once `RelayDemand` gains
   `owner: ObservationId` it can only be built by `fava-observe`, and the
   function must move. Flagging for the `fava-observe` auditor.
2. **First-fit grouping is input-order dependent.**
   `crates/fava-subscriptions-standard/src/lib.rs:50-66` merges greedily into
   the first compatible group. Two permutations of the same demand set can
   yield different wire shapes. Each shape is individually equivalent, so this
   is not a RELAY-003 violation, but it may matter for `retain`-stability once
   plan diffing exists (`planner-contract-shape`): a replan that reorders
   demand would churn subscriptions. Needs a stability rule in the contract.
3. **Does `Diagnostics` survive `Fava` close and destructive reset?**
   `Fava` has no `close()` in its public surface (`crates/fava/src/lib.rs`
   public fn list), so I could not settle whether EVENT-012's destructive reset
   must clear retained diagnostic facts. Belongs to the facade/lifecycle
   auditor.
4. **Which crate should own `RelayReadConstraints`?** RELAY-004 requires
   NIP-11-derived, per-relay, three-valued (known / unknown / malformed)
   limits, and `ARCHITECTURE.md:1816` says "limitation values consumed by
   subscription and publication planning". No NIP-11 service crate exists, so
   the constraints type has no producer today. Sequencing question for the
   plan owner.
