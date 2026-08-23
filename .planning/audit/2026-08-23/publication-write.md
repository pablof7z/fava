# Publication / write path audit

**Date:** 2026-08-23 · **Area slug:** `publication-write` · **Mode:** read-only

## Scope checked

Authority read in full:

- `docs/spec/ARCHITECTURE.md` §`fava-publication` (2114–2196): owned state, acceptance
  paths, materialization changes, independent signing/routing, routing behavior,
  delivery behavior, cancellation, recovery, suggested modules.
- `docs/spec/ARCHITECTURE.md` Part IX ownership ledger (2955–3010) and "Ordering
  belongs to the lifecycle owner" (3012+).
- `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` WRITE-001..WRITE-030
  (697–1029), CANCEL-001 refs, RELAY-001 (1031), PROFILE-006/007/008 (1582–1615).
- `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` §3.1, §3.3, §11.
- `docs/internals/vocabulary.toml` (full term index; entries `Write`, `WriteStore`,
  `PublishAs`, `PublishTo`, `Publisher`, `DeliveryPolicy`, `Signer`, `Session`).

Implementation read in full:

- `crates/fava-publication/src/{lib.rs,run.rs,delivery.rs,materialization.rs}`
- `crates/fava-write/src/{lib.rs,receipt.rs,routing.rs}` (bounds + WRITE-030 path)
- `crates/fava-write-store/src/{lib.rs,receipt.rs}`
- `crates/fava-write-store-memory/src/{lib.rs,state.rs,semantic.rs,lifecycle.rs}`
- `crates/fava-write-store-redb/src/{lib.rs,ops.rs,lifecycle.rs,semantic.rs,validation.rs}` (targeted)
- `crates/fava-publisher/src/lib.rs`, `crates/fava-publisher-nip01/src/lib.rs`
- `crates/fava-delivery/src/lib.rs`, `crates/fava-delivery-standard/src/lib.rs`
- `crates/fava/src/publication.rs`, `crates/fava/src/lib.rs` (facade edge + builder)
- Supporting: `crates/fava-routing/src/{lib.rs,chain.rs}`, `crates/fava-signer/src/lib.rs`,
  `crates/fava-transport/src/lib.rs`
- Test-corpus census: `crates/*/tests` + `mod tests` across all nine scoped crates.

---

## Findings

### router-open-failure-abandons-write — critical — failure isolation (+ ownership)

**authority**
`docs/spec/ARCHITECTURE.md:2160` — "Once an unsigned event is committed, signer
acquisition and route acquisition begin independently. Routing uses the unsigned
event's pubkey, tags, references, and route-relevant protocol facts; it does not
wait for a signature."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1007` (WRITE-028) —
"Automatic routing remains a live strategy for the write while destination work is
unresolved or new route knowledge can create required lanes."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1003` (WRITE-027) — "If the
route remains unresolved, it stays open rather than becoming no-destination merely
because time passed."

**implementation**
`crates/fava-publication/src/run.rs:180-186`:

```rust
let (routes, _) = self.open_routes(&receipt);
if matches!(receipt.routing, WriteRouting::Automatic)
    && routes.is_none()
    && semantic.is_none()
{
    self.finished(receipt_id);
    return None;          // run() returns before line 57
}
```

`crates/fava-publication/src/run.rs:50-57` — `start_signing` is reached only *after*
`initialize()` returns `Some`. `crates/fava-publication/src/run.rs:312-332`
(`open_routes`) returns `(None, …)` whenever `fava_routing::open` errors (duplicate/
empty router name, a router's `open()` refusal, or a bounded-output violation), and
commits `RoutePlan::shortfall`, which sets `settled: false`
(`crates/fava-routing/src/lib.rs:228`) so `settle_route` leaves
`ReceiptOutcome::Open` (`crates/fava-write-store/src/receipt.rs:169-179`).

So a single router refusal on an automatic non-edit write leaves a durably accepted
receipt that is permanently `Open`, is never signed, has no router session, has no
delivery lane, and has no live owner task at all (`finished()` already dropped its
cancellation entry). Routing failure silently gates signing — the exact opposite of
"begin independently".

**observable distinction**
`fava.publish(unsigned_event)` succeeds; `write.settled(fava::all())` and
`Publication::wait_terminal` never return; `write.receipt()?.current.publication.signature`
stays `SignatureState::Unsigned` forever even though an `Available` signer for that
pubkey is registered. Fixing a later router does not revive it.

**proposed falsifier**
```rust
#[tokio::test]
async fn signing_proceeds_when_the_router_chain_refuses_to_open() {
    let fava = assembly_with(RefusingRouter, available_signer_for(alice));
    let write = fava.publish(unsigned_note_by(alice)).unwrap();
    let signed = timeout(Duration::from_secs(1), poll_until(|| {
        matches!(write.receipt().unwrap().current.event, EventValue::Signed(_))
    })).await;
    assert!(signed.is_ok(), "route refusal must not gate signer acquisition");
}
```

**confidence** confirmed

---

### frozen-signer-registry-never-wakes-parked-write — critical — ownership (+ WRITE-008)

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:815-821` (WRITE-008) — "If an
accepted unsigned event has no available signer for its pubkey, it remains awaiting
that signer without elapsed-time abandonment. … Restart causes a fresh signer request
when the correct provider becomes available."
`docs/spec/ARCHITECTURE.md:2982` (ownership ledger) — "Signer registration and
availability | `fava-session` plus signer provider | publication/auth owners".

**implementation**
`crates/fava-publication/src/lib.rs:32` — `signers: Arc<BTreeMap<PublicKey, Arc<dyn Signer>>>`,
built once in `Publication::new` (`lib.rs:59-66`) from `FavaBuilder::signers`
(`crates/fava/src/lib.rs:335-347`). There is no attach/detach path anywhere; the
publication owner privately owns the signer registry that the ledger assigns to
`fava-session`.
`crates/fava-publication/src/run.rs:422-431`:

```rust
let Some(signer) = self.signers.get(&unsigned.pubkey).cloned() else { return; };
if !matches!(signer.availability(), SignerAvailability::Available) { return; }
```

`start_signing` is invoked from exactly two sites — `run.rs:57` (once per run task)
and `run.rs:354` inside `reopen_materialization`. Nothing observes signer availability
transitions, so an `Unavailable → Available` transition (or a signer that would be
registered later) never re-drives the parked write.

**observable distinction**
Register a signer for Alice whose `availability()` returns `Unavailable` for the first
N calls and then `Available`. Publish an unsigned event by Alice. The receipt stays
`Unsigned`/`Open` forever; `settled(all())` never returns. WRITE-008 requires the write
to be parked *and resumable*, not abandoned.

**proposed falsifier**
```rust
#[tokio::test]
async fn parked_write_signs_when_its_signer_becomes_available() {
    let signer = TogglingSigner::unavailable_for(alice);
    let fava = assembly_with_signer(signer.clone());
    let write = fava.publish(unsigned_note_by(alice)).unwrap();
    signer.become_available();                       // no other stimulus
    let receipt = timeout(Duration::from_secs(1), write.settled(fava::all())).await;
    assert!(receipt.is_ok(), "parked write must wake on signer availability");
}
```

**confidence** confirmed (new consequence of the known-baseline "`fava-session` does
not exist", localized in `fava-publication`)

---

### auth-required-bypasses-delivery-policy — critical — replaceability (+ WRITE-018/019)

**authority**
`docs/spec/ARCHITECTURE.md:2168-2176` — "For each due lane: 1. delivery policy returns
a decision; … 3. the selected publisher performs one attempt".
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:904-920` (WRITE-018) — the
receipt "MUST preserve exact observable outcomes such as … authentication denied; …
given up; …" as *distinct* facts.
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:924-928` (WRITE-019) — "Route
acquisition, signer availability, transport connection, relay authentication, and
durable delivery attempts have separate owners. Time spent offline, awaiting routing,
awaiting signing, or awaiting auth MUST NOT count as a failed delivery attempt."

**implementation**
`crates/fava-publication/src/delivery.rs:198-207`:

```rust
PublishOutcome::AuthenticationRequired => RelayDeliveryOutcome::GivenUp {
    reason: "relay authentication required".to_owned(),
},
```

The publication owner unilaterally converts an auth-required result into a terminal
`GivenUp` before `DeliveryPolicy::decide` ever sees it (the policy is consulted at
`delivery.rs:118` only on the *next* iteration, by which time the lane is terminal),
and the attempt has already been counted durably via `begin_attempt`
(`delivery.rs:161-168`). No `RelayDeliveryOutcome` variant expresses "authentication
denied" (`crates/fava-write/src/receipt.rs:22-55`).

**observable distinction**
An application selecting a custom `DeliveryPolicy` that wants to hold the lane while
NIP-42 auth is arranged cannot: the receipt already reads
`GivenUp { reason: "relay authentication required" }`, indistinguishable from a policy
attempt-ceiling give-up, and `receipt.attempts[session]` has been incremented for a
non-delivery condition. The default publisher therefore has a private bypass around
the replaceable delivery contract.

**proposed falsifier**
```rust
#[tokio::test]
async fn auth_required_is_a_policy_decision_not_an_owner_decision() {
    let policy = RecordingPolicy::default();                  // returns AttemptNow once, then Settled
    let fava = assembly(AuthDemandingPublisher, policy.clone());
    let write = fava.publish(signed_note()).unwrap();
    write.settled(fava::at_least(1).unwrap()).await.ok();
    assert!(policy.saw_authentication_outcome(), "policy must observe the auth fact");
    assert_eq!(write.receipt().unwrap().attempts.values().copied().max(), Some(0));
}
```

**confidence** confirmed

---

### recovery-incomplete-before-admission — major — ownership (+ WRITE-029)

**authority**
`docs/spec/ARCHITECTURE.md:2185` — "At engine start, the owner loads open writes,
reconstructs current edit or event state from durable facts, restores replaceable-event
edits through their selected protocol crates, **reopens route sessions, resumes required
signing, and schedules current lanes. Query and relay execution begin after required
write-store reconciliation is complete.**"
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1013` (WRITE-029) — "MUST
recover its open obligations, receipts, current materializations, routes, and delivery
state before the engine admits new commands that could conflict with them."

**implementation**
`crates/fava-publication/src/lib.rs:135-176` — `recover()` reads `recover_open()` /
`recover_materialized_edits()` and then calls `start(...)`/`start_semantic(...)`, which
`tokio::spawn` a run task (`crates/fava-publication/src/run.rs:26-40`) and return
immediately. Route reopening, signing resumption, and lane scheduling all happen inside
that spawned task, i.e. *after* `FavaBuilder::build()` returns
(`crates/fava/src/lib.rs:429-433`). `build()` is synchronous and hands the caller a
`Fava` that already accepts `observe()` and `publish()`.

**observable distinction**
Immediately after `build()` returns on a restarted persistent store, no router session
exists for any recovered automatic write, no lane has begun, and `open_receipts()`
returns pre-restart `route_revision`/`desired_destinations`. Reconciliation is
concurrent with, not before, query and relay admission.

**proposed falsifier**
```rust
#[tokio::test]
async fn recovery_reopens_route_sessions_before_build_returns() {
    let store = reopen_store_with_one_open_automatic_write();
    let router = CountingRouter::default();
    let fava = Fava::builder().write_store(store).router(router.clone()) /* … */ .build().unwrap();
    assert_eq!(router.open_calls(), 1, "route session must exist when build() returns");
    let _ = fava;
}
```

**confidence** confirmed

---

### facade-cancel-write-bypasses-owner — major — ownership (+ WRITE-023 / RELAY-001)

**authority**
`docs/spec/ARCHITECTURE.md:2126` — owned state of the publication owner includes "exact
cancellation eligibility"; `ARCHITECTURE.md:2181` — "Cancellation is decided from current
materialization, signature, and handoff facts."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:961-964` (WRITE-023) —
"Cancellation MUST: terminate current signer/route/delivery work; …"
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1033` (RELAY-001) — "Fava MUST
open or retain a relay session only while current query, routing acquisition,
authentication, publication … requires it."

**implementation**
`crates/fava/src/lib.rs:121-125`:

```rust
pub fn cancel_write(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError> {
    self.write_store.cancel(receipt_id).map(|receipt| receipt.is_some())
}
```

This is a second public cancellation door that goes straight to the durable provider,
skipping `Publication::cancel` (`crates/fava-publication/src/lib.rs:194-214`) and
therefore never firing the owner's per-receipt cancellation watch
(`cancellations: Arc<Mutex<BTreeMap<ReceiptId, watch::Sender<bool>>>>`, `lib.rs:37`).
`read_receipt` short-circuits on that watch (`run.rs:197-205`); without it, the run task
proceeds through `initialize()` and calls `open_routes()` on the already-cancelled
receipt (`run.rs:180`), opening router sessions for work that no longer exists.

**observable distinction**
On a current-thread runtime, `let w = fava.publish(e)?; fava.cancel_write(w.receipt_id())?;`
before the spawned run task is first polled results in one `Router::open` call (and any
relay work that router starts); the same sequence via `fava.cancel_publication(...)`
results in zero. Two doors, two post-conditions, for one lifecycle.

**proposed falsifier**
```rust
#[tokio::test]
async fn cancellation_never_opens_router_sessions_for_a_cancelled_write() {
    let router = CountingRouter::default();
    let fava = assembly_with_router(router.clone());
    let write = fava.publish(unsigned_note_by(alice)).unwrap();
    fava.cancel_write(write.receipt_id()).unwrap();     // owner-bypassing door
    tokio::task::yield_now().await;
    assert_eq!(router.open_calls(), 0);
}
```

**confidence** confirmed

---

### delivery-lane-can-hot-spin — major — failure isolation / boundedness

**authority**
`AGENTS.md` gate 4 — "blocking, failure, panic, cancellation, and stale completions
remain scoped and attributable"; gate 3 — "a competing implementation can use the public
contract to achieve the same result".
`docs/spec/ARCHITECTURE.md:2168-2176` — the delivery lane is a bounded sequence of
policy decision → authorized attempt → publisher attempt → committed result.

**implementation**
`crates/fava-publication/src/delivery.rs:94-142` — `run_destination` is
`loop { read_receipt; decide; if AttemptNow { self.attempt(...).await } }`, and
`attempt` (`delivery.rs:152-170`) returns without recording anything when
`store.begin_attempt` refuses:

```rust
let Ok(receipt) = self.store.begin_attempt(...) else { return; };
```

`begin_attempt` refuses whenever the destination outcome is not `Pending`/`Retryable`
(`crates/fava-write-store-memory/src/lifecycle.rs:166-173`). A replaceable
`DeliveryPolicy` that returns `AttemptNow` for, say, an `Acknowledged` destination
therefore produces a `loop` whose body reaches **no await point** — `read_receipt`
returns on the first synchronous `Ok`, and `attempt` returns before its first `.await`.
The task never yields, permanently starving a Tokio worker.

**observable distinction**
Selecting a third-party `DeliveryPolicy` (an explicitly replaceable boundary) can wedge
the whole engine: unrelated `Fava::observe` handles and other writes stop making
progress. A provider's bad decision must stay scoped to its own lane and be refused,
not become a runtime-wide liveness failure.

**proposed falsifier**
```rust
#[tokio::test(flavor = "current_thread")]
async fn a_policy_that_always_attempts_cannot_starve_the_runtime() {
    let fava = assembly_with_policy(AlwaysAttemptPolicy);
    let write = fava.publish(signed_note()).unwrap();
    let other = timeout(Duration::from_secs(1), fava.observe(Query::events().cache_only())).await;
    assert!(other.is_ok(), "one lane's policy must not block the runtime");
    let _ = write;
}
```

**confidence** confirmed

---

### memory-write-store-retains-terminal-receipts-without-bound — major — boundedness

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:981` (WRITE-024) — "Completed
receipt retention MUST be bounded under one declared policy. Eviction removes only
evidence exclusively owned by the evicted receipt and never active work."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1607-1613` (PROFILE-008) —
oldest-first bounded terminal-receipt retention.

**implementation**
`crates/fava-write-store-memory/src/state.rs:41-55`:

```rust
pub(super) fn active_count(state: &WriteState) -> usize {
    state.writes.values().filter(|receipt| !receipt.is_terminal()).count()
}
pub(super) fn capacity_reached(state: &WriteState, capacity: usize) -> bool { … }
```

Only *active* writes are bounded. `MemoryWriteStore` exposes a single `capacity`
(`crates/fava-write-store-memory/src/lib.rs:33-58`) and never evicts; terminal receipts
accumulate in `state.writes` until the process dies. Contrast `RedbWriteStore`, which
declares both bounds and evicts oldest-first (`crates/fava-write-store-redb/src/lib.rs:58-60`,
`crates/fava-write-store-redb/src/lifecycle.rs:32,72`).

**observable distinction**
With `MemoryWriteStore::bounded(8)`, drive 10 000 writes to terminality: every
`fava.receipt(first_receipt_id)` still returns `Some`, and process RSS grows linearly
with total historical writes. No declared terminal-retention policy exists to name.

**proposed falsifier**
```rust
#[tokio::test]
async fn memory_write_store_retires_oldest_terminal_receipts() {
    let store = MemoryWriteStore::with_bounds(nz(4), nz(4));  // terminal bound must exist
    let first = accept_and_settle(&store);
    for _ in 0..8 { accept_and_settle(&store); }
    assert!(store.receipt(first).unwrap().is_none(), "oldest terminal receipt must retire");
}
```

**confidence** confirmed

---

### publication-owner-has-no-owner-level-proof — major — behavioral proof

**authority**
`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:42` — "the single component that owns the
decision or lifecycle"; `:63` — "Write the test at the smallest stable owner that can
prove the behavior."; `:371` — "implement a second materially different provider,
preferably outside the owning crate/workspace boundary"; `:375` — "A trait with one
implementation is not evidence of substitutability."

**implementation** (census, ran across all nine scoped crates)

| crate | `tests/` | `mod tests` |
|---|---|---|
| `fava-publication` | none | none |
| `fava-write-store` | none | none |
| `fava-publisher` | none | none |
| `fava-publisher-nip01` | none | none |
| `fava-delivery` | none | none |
| `fava-delivery-standard` | none | `src/lib.rs` (2 policy cases) |
| `fava-write-store-memory` | none | `src/model.rs` only |

The owner of acceptance, materialization, signing, routing, lanes, cancellation and
recovery — `fava-publication` — has **zero** executable evidence at the owning
component; every claim is proved indirectly through `crates/fava/tests/*`. That is
exactly the "evidence written to match the implementation" shape the live-query
finding exposed: the facade tests exercise the assembled happy path and therefore
never reach the router-refusal, availability-transition, or policy-refusal branches
reported above.

Separately, `DeliveryPolicy` has exactly **one** implementation workspace-wide
(`grep -rn "impl DeliveryPolicy for"` → `fava-delivery-standard` only, no test double,
no conformance kit), so its replaceability is unproven by the guide's own §11 rule.
`Publisher` does have test doubles and is better off.

**observable distinction**
Not directly application-observable; it is the gate-6 hole that lets the five findings
above ship green. Reported as major because the public promises of `fava-publication`
carry no falsifiable evidence at their owner.

**proposed falsifier**
```rust
// crates/fava-publication/tests/lifecycle.rs — first owner-level test
#[tokio::test]
async fn owner_signs_independently_of_route_acquisition() {
    let publication = Publication::new(/* refusing router, available signer, … */).unwrap();
    let accepted = publication.accept(WriteIntent::event(unsigned, WriteRouting::Automatic).unwrap()).unwrap();
    let receipt = timeout(Duration::from_secs(1), publication.wait_terminal(accepted.receipt_id)).await;
    assert!(receipt.is_ok());
}
```

**confidence** confirmed

---

### publisher-opens-a-fresh-session-per-attempt — major — boundedness (+ WRITE-019)

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:930` (WRITE-019) — "Several
writes for one relay SHOULD share connection/backoff ownership rather than creating
independent reconnect storms."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1033` (RELAY-001).

**implementation**
`crates/fava-publisher-nip01/src/lib.rs:23` opens a session per attempt
(`transport.open_session(attempt.session.clone()).await`) and unconditionally closes it
on every exit path (`:32,:44,:49,:53,:104`). `crates/fava-transport/src/lib.rs:29`
documents `open_session` as "Open a **fresh** session generation", so there is no
sharing at the contract level either. Lanes are also per-`(write, session)` tasks
(`crates/fava-publication/src/delivery.rs:66`) with no cross-write refcount.

**observable distinction**
Ten writes routed to one relay produce ten independent connect/close cycles against
that relay (observable at a scripted relay as ten socket opens), rather than one shared
session with shared backoff.

**proposed falsifier**
```rust
#[tokio::test]
async fn concurrent_writes_to_one_relay_share_one_session() {
    let relay = ScriptedRelay::acknowledging();
    let fava = assembly_to(relay.url());
    let writes: Vec<_> = (0..10).map(|i| fava.to([relay.url()]).unwrap().publish(note(i)).unwrap()).collect();
    for w in &writes { w.settled(fava::at_least(1).unwrap()).await.unwrap(); }
    assert_eq!(relay.connection_count(), 1);
}
```

**confidence** confirmed (overlaps the known relay-session baseline; the *driver* is in
this area, so reported here)

---

### correction-destinations-dropped-by-next-route-apply — major — WRITE-022 / ownership

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:948-955` (WRITE-022) — "the
successor's destination set MUST include: current automatic or explicit routing; **and
destinations that require correction because they may have received the predecessor.**"
`docs/spec/ARCHITECTURE.md:2154` — "prior delivery facts remain scoped to their exact
predecessor event id."

**implementation**
The write store does the right thing: `install_semantic` unions
`desired_destinations ∪ destinations.keys()` into `correction_destinations`
(`crates/fava-write-store-memory/src/semantic.rs:294-306`; same in
`crates/fava-write-store-redb/src/semantic.rs:250-262`), resetting each to `Pending`.

The publication owner then immediately overwrites that set with a fresh router-only
plan: `crates/fava-publication/src/run.rs:289-304` applies
`route.revision = installed.route_revision + 1`, and `apply_route_to_receipt`
(`crates/fava-write-store/src/receipt.rs:123-140`) *removes* any destination that is
`Pending` and absent from the new plan. Every correction destination is `Pending` by
construction, so a router that has since withdrawn a relay silently erases the
correction obligation the store just created. `reopen_materialization`
(`crates/fava-publication/src/run.rs:345-359`) applies a second router-only plan on
top for the same generation.

**observable distinction**
Accept a follow edit routed automatically; let it be acknowledged at relay A; make the
router withdraw A; ingest a newer contact list so the edit rematerializes. WRITE-022
requires the successor generation to still carry A as a correction destination; today
`receipt.desired_destinations` no longer contains A and A never receives the corrected
event.

**proposed falsifier**
```rust
#[tokio::test]
async fn corrected_generation_keeps_predecessor_destinations_after_router_withdrawal() {
    let (fava, router) = semantic_assembly();
    let write = fava.by(alice).publish(follow(bob)).unwrap();
    write.settled(fava::at_least(1).unwrap()).await.unwrap();     // acknowledged at A
    router.withdraw(relay_a());
    ingest_newer_contact_list(&fava);
    poll_until(|| write.receipt().unwrap().current.publication.materialization_id.as_u64() == 2);
    assert!(write.receipt().unwrap().desires(&session_for(relay_a())));
}
```

**confidence** suspected (needs a scripted run to confirm the ordering window;
both code paths were read and they do contradict)

---

### private-semantic-lifecycle-nouns — minor — vocabulary

**authority**
`AGENTS.md` vocabulary policy (restated in the audit brief): "A new crate, public or
cross-crate nominal type, provider contract, persisted entity, configuration concept,
or **lifecycle owner** is a vocabulary change"; `docs/internals/vocabulary.toml` is the
source of truth and `tools/check_vocabulary.py` only scans `pub struct|enum|trait|type`.

**implementation**
`crates/fava-publication/src/materialization.rs:18` `pub(super) struct OpenedSemanticSources`
owns two live `OpenedQuerySource` subscriptions plus their per-source liveness and an
explicit `close()` lifecycle (`:82-85`). `crates/fava-publication/src/materialization.rs:95`
`pub(super) struct SemanticState` owns the per-write rematerialization lifecycle
(selected source id, source floor, failed source id, the sources handle) with
`accepted`/`recovered`/`close` constructors. Neither appears in
`docs/internals/vocabulary.toml` (checked the full `name =` index; the `Write` term at
:430 lists only `fava_publication::Publication` and `PublicationError`). The checker
cannot see them because they are `pub(super)`.

**observable distinction**
None at the application boundary — this is the known gate hole, reported so the ledger
stays honest about who owns the semantic-source subscription lifecycle.

**proposed falsifier**
```python
# tools/check_vocabulary.py — widen the scan
def test_private_lifecycle_owners_are_declared():
    nouns = scan(r"pub(\(crate\)|\(super\))? (struct|enum|trait) (\w+)")
    assert {"OpenedSemanticSources", "SemanticState"} <= declared_terms()
```

**confidence** confirmed

---

### attempt-timeout-is-owner-private — minor — replaceability

**authority** `docs/spec/ARCHITECTURE.md:2168-2176` (the owner performs policy-decided
attempts; the publisher performs one attempt); `PROFILE-007:1590` — a distribution
"SHOULD name its complete selected profile rather than relying on hidden facade defaults."

**implementation** `crates/fava-publication/src/delivery.rs:14` —
`const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);`, stamped into every
`PublishAttempt` (`delivery.rs:180`). No assembly input, no `DeliveryPolicy` input, no
`Publisher` input can change it.

**observable distinction** An application on a slow relay cannot lengthen the
per-attempt deadline through any public contract; every attempt becomes
`OutcomeUnknown { reason: "publication deadline elapsed after handoff" }` at 5 s.

**proposed falsifier**
```rust
#[test]
fn attempt_deadline_is_a_selected_profile_fact() {
    let policy = StandardDeliveryPolicy::new(nz(1)).with_attempt_timeout(Duration::from_secs(30));
    assert_eq!(policy.attempt_timeout(), Duration::from_secs(30));  // no such surface today
}
```

**confidence** confirmed

---

## Conforming (verified, not merely unexamined)

Each of these was read on both sides and matches the authority:

- **Facade thinness (the live-query failure shape does NOT repeat here).**
  `crates/fava/src/publication.rs` (317 lines) contains only payload→`WriteIntent`
  conversion, the inert `PublishAs`/`PublishTo` scopes, the settlement predicates
  `all()`/`at_least()`, and a `Write` handle that delegates to `Publication`. It owns no
  route session, no lane, no retry loop, no signer selection, no transport handle. The
  one exception is `Fava::cancel_write` (reported above).
- **Acceptance never blocks on a network or signer future (WRITE-004 / WRITE-013).**
  `Publication::accept` (`crates/fava-publication/src/lib.rs:88-131`) is fully
  synchronous: reserve → `prepare_semantic` (local `cache_only()` sources +
  synchronous `Router::preview`) → atomic store commit → `start_semantic` spawn. No
  `.await` exists on the acceptance path, and the store commit precedes the spawn, so
  write-source visibility precedes the returned handle. This is the write analogue of
  the observe hang and it is **absent**.
- **Exact generation identity on every completion (WRITE-007 / ARCH:2150-2156).**
  `install_signed`, `record_signer_refusal`, `apply_route`, `begin_attempt`,
  `record_outcome`, `install_materialization`, and `record_materialization_failure` all
  take `(write_id, receipt_id, materialization_id, event_id)` and are validated by
  `validate_current_materialization` (`crates/fava-write-store/src/receipt.rs:57-72`).
  `install_signed_current` additionally verifies the signature and compares the exact
  unsigned body (`crates/fava-write-store-memory/src/lifecycle.rs:25-48`), and
  `apply_route_to_receipt` enforces strict revision monotonicity
  (`crates/fava-write-store/src/receipt.rs:88-93`). A late signing, route, or delivery
  completion for a superseded generation cannot install stale state.
- **Explicit writes open no router session (ARCH:2166).**
  `open_routes` early-returns `(None, receipt.route_revision)` for
  `WriteRouting::Explicit` (`crates/fava-publication/src/run.rs:313-315`), and
  `apply_route_to_receipt` refuses to mutate an explicit receipt
  (`crates/fava-write-store/src/receipt.rs:83-87`). Acceptance stamps
  `route_settled = true, route_revision = 1` for explicit
  (`crates/fava-write-store-memory/src/lib.rs:136-137`).
- **Per-write delivery fan-out is bounded.** `apply_route_to_receipt` refuses plans over
  `destination_evidence_capacity()` = 256 (`crates/fava-write-store/src/receipt.rs:94-100`),
  shortfalls are bounded and text-validated (`:110-119`), retired materializations are
  bounded (`crates/fava-write-store-memory/src/semantic.rs:281-287`), explicit relay sets
  are bounded at 256 (`crates/fava-write/src/routing.rs:8`), materializers at 64
  (`crates/fava-publication/src/materialization.rs:15`), and active writes are bounded by
  the store's `active_capacity`. Lane tasks are keyed by session in `active` and
  de-duplicated (`crates/fava-publication/src/delivery.rs:48-52`), so lanes cannot grow
  without bound *within* a write.
- **Attempt counts are durably authorized before any transport effect (WRITE-019).**
  `begin_attempt` commits `Attempting` and the incremented count before
  `publisher.publish` is called (`crates/fava-publication/src/delivery.rs:161-186`); the
  standard policy stops at a finite `NonZeroU32` ceiling
  (`crates/fava-delivery-standard/src/lib.rs:31-46`); `OutcomeUnknown` is terminal for
  the standard policy (WRITE-020).
- **WRITE-022 correction destinations are computed correctly at the store layer** (both
  memory and redb) — the defect reported above is in the owner's subsequent
  route application, not the store.
- **WRITE-030 (already-expired refusal) is enforced pre-custody** in
  `WriteIntent::event`/`presigned` (`crates/fava-write/src/lib.rs:93,117`).
- **Receipt-change delivery reports lag explicitly rather than losing state.**
  `Publication::wait_until` re-reads durable state on `RecvError::Lagged`
  (`crates/fava-publication/src/lib.rs:266-272`), as does the run loop
  (`crates/fava-publication/src/run.rs:120`).
- **No unapproved *public* vocabulary noun in the nine crates.** Every `pub struct|enum|trait`
  in scope maps to a `vocabulary.toml` term (`Write`, `WriteStore`, `PublishAs`,
  `PublishTo`, `WriteIntent`, `ReplaceableEventEdit`, `ReplaceableEventMaterializer`,
  `MaterializationId`, `UnsignedEvent`, `EventBuilder`, `Publisher`, `DeliveryPolicy`).
  The two private lifecycle nouns are reported above.

## Open questions

1. **WRITE-009 (sign without publishing) has no implementation anywhere.**
   `grep -rn "sign_event" crates/` shows the contract is only ever driven from
   `crates/fava-publication/src/run.rs:438`; there is no facade or owner door that signs
   an `UnsignedEvent` without creating a write intent, receipt, route session, or
   delivery. I could not determine from `FAVA_REWRITE_IMPLEMENTATION_PLAN.md` whether
   this is scheduled for a later slice, so I did not file it as a deviation. If it is in
   a delivered slice, it is a critical gap.
2. **WRITE-018 does not have distinct facts for "awaiting route", "awaiting signer",
   "queued", or "backing off".** `RelayDeliveryOutcome` collapses all pre-attempt states
   into `Pending` and `SignatureState` has no "awaiting signer" variant. The requirement
   says "such as", so this may be intentional; it does mean an application cannot
   distinguish a write parked on a missing signer from one queued behind routing —
   which is precisely the state the `frozen-signer-registry` finding makes permanent.
3. **Double route application per materialization.** `rematerialize`
   (`run.rs:289-304`) and the subsequent `reopen_materialization` (`run.rs:345-359`)
   both apply a router-derived plan for the same generation, bumping `route_revision`
   twice. I could not construct an application-visible harm beyond the
   `correction-destinations` finding, so it is folded into that entry rather than filed
   separately.
4. `Publication::recover()` requires a live Tokio runtime and is called from the
   synchronous `FavaBuilder::build()`, so any assembly with publication providers
   cannot be built outside a runtime. Whether that is intended is a product decision,
   not obviously a spec deviation.
