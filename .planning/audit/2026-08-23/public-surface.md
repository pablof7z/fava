# Public surface audit

Area slug: `public-surface`.
Scope: complete public API of every crate in `crates/`, plus `apps/**`, `README.md`,
and `docs/` other than `docs/spec/`.

Findings that other area reports in this directory already filed under their own id
are **not** re-filed here. Where my area produces additional evidence for an existing
finding, it appears under "Confirmations of findings already filed elsewhere" with the
existing id, not as a new finding. New findings below have ids that do not appear in
any other report in `.planning/audit/2026-08-23/`.

## Scope checked

Specs / authority read:

- `docs/spec/partial-spec-api-semantics.md` (all 628 lines)
- `docs/spec/ARCHITECTURE.md` lines 190-250, 500-680, 1025-1045, 2367-2470, 2900-3010, 3540-3686
- `AGENTS.md` (all 75 lines)
- `docs/internals/vocabulary.toml` (registry structure + query/observe/planner/write-store sections)
- `tools/check_vocabulary.py` (lines 1-30, 200-285) and its live output

Code read in full or in the relevant part:

- `crates/fava/src/lib.rs`, `publication.rs`, `live.rs`, `routes.rs`, `query_source.rs`,
  `relay.rs` (lines 1-215)
- `crates/fava-observe/src/lib.rs` (lines 1-280)
- `crates/fava-query/src/lib.rs`, `selection.rs`
- `crates/fava-event-cache/src/lib.rs` (complete)
- `crates/fava-write-store/src/lib.rs` (trait body, lines 1-350)
- `crates/fava-publication/src/lib.rs` (public signatures), `run.rs`/`delivery.rs` spawn sites
- `crates/fava-routing/src/lib.rs`, `fava-subscriptions/src/lib.rs`, `fava-transport/src/lib.rs`,
  `fava-signer/src/lib.rs`, `fava-publisher/src/lib.rs`, `fava-delivery/src/lib.rs`,
  `fava-diagnostics/src/lib.rs`, `fava-state/src/lib.rs`, `fava-write/src/*` (public declarations)
- All 37 `crates/*/Cargo.toml` + `apps/canary/Cargo.toml` + workspace `Cargo.toml`
- `apps/canary/src/**` (scenario entry points, `grouping.rs`, `local.rs`, `live.rs`,
  `multi.rs`, `routing.rs`, `publication.rs`, `semantic_write_support.rs`,
  `automatic_publication.rs`), `apps/canary/README.md`, `apps/canary/scenarios.json`
- `README.md`, `crates/fava-simple-groups/README.md`, `crates/fava-nip02/README.md`

Searches that actually ran (results recorded under "Conforming"):

- `grep -rn 'downcast\|dyn Any\|any::type_name' crates` -> 0 hits
- `grep -rn '#[cfg(feature' crates apps` -> 0 hits
- `grep -rn '\[features\]' crates/*/Cargo.toml apps/*/Cargo.toml` -> 0 hits
- full `pub (struct|enum|trait|type|fn|const|use)` inventory across all 37 crates
- `find crates apps -name '*.rs' | xargs wc -l | awk '$1>500'`
- `python3 tools/check_vocabulary.py`

---

## Deliverable 1 — Surface inventory (summary; full item list captured during the audit)

| Crate | pub items | Verdict |
|---|---|---|
| `fava` | 53 | specified shape (ARCHITECTURE:2390-2419) but **incomplete**: no `fetch_cache`, `services`, session/account ops, sign-without-publish, NIP-42 attachment, or `close`/`reset`. See `facade-lacks-nameable-assembly-vocabulary`, and existing `no-facade-close-or-command-admission`. Leaks a Tokio primitive (`receipt_changes`) and a private trait bound (`publish<P: PublishPayload>`). |
| `fava-query` | 54 | specified (ARCHITECTURE:558-700). `Query`/`EventRecord`/`QuerySnapshot`/`QueryEvidence` match the spec shapes. `Selection` union/intersection/difference and `ValueSet<T>` are absent — registered as `spec_symbols` in `vocabulary.toml:290`, i.e. knowingly not-yet-delivered, not drift. `SourceKind`/`SourceStatus` are **additions** to the specified `SourceSnapshot` (ARCHITECTURE:622-625 has neither field); registered vocabulary but see `source-role-impersonation`. |
| `fava-write` | 67 | specified (ARCHITECTURE:495-556). Conforming. |
| `fava-observe` | 11 | `Observer`/`Observation`/`ObserveError`/`ObservationClosed` registered. Two unspecified-and-leaking items: `attach_cancellation(watch::Sender<bool>)` and `ObserveError::Relay(String)`. |
| `fava-write-store` | 9 | contract; 10 of 21 trait methods carry default bodies — see `write-store-contract-half-optional`. |
| `fava-event-cache` | 2 | contract; `events()` is an unbounded required read (existing `event-cache-contract-forces-full-materialization`). |
| `fava-simple-groups` | 108 | matches partial-spec section 10 item-for-item (`Group::on`, `events`, `records`, `GroupSnapshot::at`/`metadata_differ`, `SimpleGroups::saved_*`/`groups_where_*`/`groups_saved_by`, `prepare`, `project`, materializers). All nominal types registered. Conforming. |
| `fava-nip02` | 26 | matches partial-spec section 10 (`contact_list`, `followers_of`, `follows_of`, `ContactList`, `ContactListRowEvidence`). Conforming. |
| `fava-nip65`, `fava-bookmarks` | 9 / 5 | pure protocol values + edits, no observation lifecycle. Conforming with partial-spec rule 5. |
| `fava-routing` | 23 | specified. Conforming for surface shape. |
| `fava-subscriptions` (+ `-standard`, `-no-grouping`) | 7 / 2 / 1 | specified. `-no-grouping` is registered vocabulary (`vocabulary.toml:684`) though absent from the ARCHITECTURE crate table (3640-3653). |
| `fava-transport` (+ `-websocket`, `-testkit`) | 4 / 2 / 4 | specified. |
| `fava-signer`, `-local`; `fava-publisher`, `-nip01`; `fava-delivery`, `-standard` | 3/2, 3/1, 3/2 | specified. Conforming. |
| `fava-state` | 27 | specified. `RelayEvidence` uses accessors instead of the illustrative `pub seen_on`; partial-spec section 5 says the shape may grow, so not drift. |
| `fava-diagnostics` | 15 | 1 read method + 12 public write methods with no producer authority. See existing `diagnostics-facade-sole-producer`. |
| `fava-wire` | 3 | specified. |
| `fava-router-*` (4 + testkit) | 3-4 each | specified. |
| `fava-event-cache-memory`, `fava-write-store-memory`, `-redb`, `fava-query-standard` | 1-3 each | implementations expose only a constructor plus the trait impl. Conforming. |

Crates named in ARCHITECTURE:3612-3665 that do not exist: `fava-runtime`, `fava-session`,
`fava-standard`, `fava-auth`, `fava-event-cache-redb`, `fava-fetch-cache*`,
`fava-signer-nip46`, `fava-nip05*`, `fava-nip11*`, `fava-content`, and every `*-testkit`
except router/transport. Already filed as `runtime-crate-absent`, `session-owner-misplaced`,
`nip42-auth-has-no-owner`, `no-router-conformance-testkit`, `no-signer-conformance-kit`.

## Deliverable 2 — Contract/impl split and dependency graph

Every one of the 37 `crates/*/Cargo.toml` files plus `apps/canary/Cargo.toml` was read.

**Result for the specific check the brief asked for: clean.** No universal owner
(`fava-ingest`, `fava-observe`, `fava-publication`, `fava-diagnostics`) and no
`fava` facade has a **regular** `[dependencies]` edge to any `-standard`, `-memory`,
`-redb`, `-websocket`, `-local`, or `-no-grouping` crate.

- `fava` deps: `fava-diagnostics`, `-event-cache`, `-ingest`, `-observe`, `-publication`,
  `-publisher`, `-query`, `-routing`, `-signer`, `-state`, `-subscriptions`, `-transport`,
  `-wire`, `-write`, `-write-store`, `-delivery`. Contracts and universal owners only.
- `fava-ingest` -> `fava-event-cache`, `fava-state` (contract + domain).
- `fava-observe` -> `fava-query` only.
- `fava-publication` -> `fava-delivery`, `-publisher`, `-query`, `-routing`, `-signer`,
  `-state`, `-transport`, `-write`, `-write-store`. All contracts.
- `fava-diagnostics` -> `fava-state`.
- Every provider crate depends on its contract crate, never the reverse. No contract crate
  depends on any implementation crate.

Direction inversions and cycles found:

- **No runtime cycles.** Two `[dev-dependencies]` cycles exist and are Cargo-legal:
  `fava` <-> `fava-write-store-redb` and `fava` <-> `fava-simple-groups`. Both are
  test-only; `crates/fava-simple-groups/src/**` contains no `fava::` reference (checked),
  the facade dep is used only by `crates/fava-simple-groups/tests/`.
- `fava -> fava-wire` (`crates/fava/Cargo.toml:20`) and `fava -> fava-ingest`
  (`crates/fava/Cargo.toml:10`) are the Cargo-level fingerprint of the facade owning
  wire grammar and admission. Already filed as `facade-owns-ingest-pipeline`; the manifest
  edge is added evidence there, not a new finding.
- `fava-write-store -> fava-routing` and `fava-event-cache -> fava-query`: contract-on-contract,
  consistent with `domain values -> neutral contracts -> providers`.

## Deliverable 3 — Private bypass hunt

`grep -rn 'downcast\|dyn Any\|any::type_name' crates --include='*.rs'` returns **zero
hits**. The only `std::any` use in the repository is `apps/canary/src/croissant_simple_groups.rs:406`,
recovering a panic payload from `JoinError` — not a Fava bypass.

`grep -rn '#[cfg(feature'` across `crates` and `apps` returns **zero hits**, and no
`Cargo.toml` in the workspace or in `apps/canary` declares a `[features]` table.
There are no feature-gated code paths.

No owner special-cases a concrete provider type by name. The one live instance of
type-identity bypass is by *role value* rather than Rust type — filed below as
`source-role-impersonation`.

## Findings

### `source-role-impersonation` — major — replaceability

**authority** `docs/spec/ARCHITECTURE.md:622`
```
pub struct SourceSnapshot {
    pub revision: SourceRevision,
    pub events: Vec<SourceEvent>,
}
```
and `docs/spec/ARCHITECTURE.md:198` "A live query's local result is the deterministic
merge of several query sources, **primarily**: ...".

**implementation** `crates/fava-query/src/lib.rs:233` adds a closed
`pub enum SourceKind { EventCache, WriteStore }` and `crates/fava-query/src/lib.rs:257`
stamps it onto every `SourceSnapshot`. `crates/fava-observe/src/lib.rs:239-246`
routes each source change by that value:
```rust
fn replace_source(sources: &mut [SourceSnapshot], changed: SourceSnapshot) {
    if let Some(source) = sources.iter_mut().find(|source| source.kind == changed.kind) {
```
and `crates/fava-observe/src/lib.rs:147` branches on `if role == SourceKind::EventCache`.
`crates/fava/src/query_source.rs:22` and `:88` make the impersonation live:
`impl QuerySource for Fava` returns `SourceSnapshot::empty(SourceKind::EventCache)` and
`SourceSnapshot { kind: SourceKind::EventCache, ... }` while packing
`SourceEvent::Local(..)` write-store contributions into it (`query_source.rs:95-98`).

**observable distinction** A `QuerySnapshot`'s own evidence lies. Assemble Fava A whose
only content is one accepted, unpublished local write; register `Arc<FavaA>` as the
event-cache source of Fava B; `fava_b.observe(q).current().evidence.sources` reports
`SourceEvidence { kind: SourceKind::EventCache, .. }` for a record whose
`publication` is `Some(..)` and whose `relay_evidence` is empty. Separately, a
third-party `QuerySource` returning any snapshot whose `kind` does not match the slot it
was installed in is **silently discarded** by `replace_source` — no error, no diagnostic,
the observation just stops updating for that source.

**proposed falsifier**
```rust
#[tokio::test]
async fn source_evidence_names_the_provider_that_actually_contributed() {
    let fava = assemble_with_write_only_local_event().await;
    let snap = fava.observe(Query::events().cache_only()).await.unwrap().current();
    assert!(snap.events[0].publication.is_some());
    let ev = snap.evidence.sources.iter().find(|s| s.kind == SourceKind::WriteStore);
    assert!(ev.is_some(), "write-store contribution must not be attributed to the event cache");
}
```

**confidence** confirmed

---

### `observe-relay-variant-unproducible-by-its-owner` — major — ownership

**authority** `docs/spec/ARCHITECTURE.md:2985` "| Wire subscription plan | `fava-observe`
owns desired plan; planner computes it | transport executes it |" and
`docs/spec/ARCHITECTURE.md:2372` the facade "owns no event-kind dispatch, routing policy,
query evaluation, retry algorithm, socket state, or storage schema."

**implementation** `crates/fava-observe/src/lib.rs:270-272` declares
```rust
    /// Relay work could not establish one exact live query.
    #[error("relay query refused: {0}")]
    Relay(String),
```
Every producer of that variant is outside the crate that declares it —
`crates/fava/src/lib.rs:240`, `:244`; `crates/fava/src/routes.rs:17`, `:22`, `:25`, `:30`;
`crates/fava/src/live.rs:23`, `:28`, `:51`. `fava-observe` does not depend on
`fava-transport`, `fava-subscriptions`, or `fava-routing` at all
(`crates/fava-observe/Cargo.toml`), so it can never construct it.

**observable distinction** Every relay-side refusal reaches the application as one
untyped `String`. `TransportError`, `SubscriptionPlanError`, and `RouterError` are all
flattened by `.map_err(|error| ObserveError::Relay(error.to_string()))`. An application
cannot programmatically distinguish "no transport was assembled" from "the relay refused
the subscription" from "the planner exceeded its bound" — all three are
`ObserveError::Relay(_)` with a hand-written sentence inside.

**proposed falsifier**
```rust
#[tokio::test]
async fn planner_capacity_refusal_is_typed_at_the_observation_owner() {
    let fava = assemble_with_planner_that_refuses_for_capacity().await;
    let err = fava.observe(Query::events()).await.unwrap_err();
    assert!(matches!(err, ObserveError::Subscription(SubscriptionPlanError::TooManySubscriptions { .. })));
}
```

**confidence** confirmed

---

### `runtime-primitives-in-the-public-surface` — major — replaceability

**authority** `docs/spec/partial-spec-api-semantics.md:330` "Fava SHOULD expose these
semantics rather than exposing a concrete runtime primitive such as Tokio's
`watch::Receiver`, even if a similar mechanism is used internally."
`docs/spec/ARCHITECTURE.md:2381` lists the facade's public surface; no runtime channel
appears in it.

**implementation** Two public items hand a Tokio type to callers:

- `crates/fava/src/lib.rs:184`
  `pub fn receipt_changes(&self) -> broadcast::Receiver<(ReceiptId, Option<Receipt>)>`
  — this is on the **application-facing facade**, and its own doc comment
  (`crates/fava/src/lib.rs:180-182`) makes `tokio::sync::broadcast::error::RecvError::Lagged`
  part of Fava's promised loss signal.
- `crates/fava-observe/src/lib.rs:210`
  `pub fn attach_cancellation(&mut self, cancel: watch::Sender<bool>)` — not in the
  specified `impl Observation` (`docs/spec/partial-spec-api-semantics.md:307-315`, which
  lists exactly `current`, `changed`, `close`). It exists solely so the facade can graft
  its privately owned relay tasks onto the owner's handle
  (`crates/fava/src/live.rs:58`, `crates/fava/src/routes.rs:52`).

**observable distinction** An application consuming receipts must link the exact
`tokio =1.53.1` pinned in `Cargo.toml` and must match on a Tokio error enum; it cannot be
written against Fava's vocabulary alone. And any crate can widen an `Observation`'s
cancellation set without the observation owner knowing what it just took responsibility
for — `additional_cancel` (`crates/fava-observe/src/lib.rs:84`) is a `Vec` with no bound,
no identity, and no way to detach.

**proposed falsifier**
```rust
// compile-only evidence at the facade, no tokio import in the test file
#[test]
fn receipt_changes_is_expressible_without_naming_tokio() {
    let _: fn(&fava::Fava) -> fava::ReceiptChanges = fava::Fava::receipt_changes;
}
```

**confidence** confirmed

---

### `write-store-contract-half-optional` — major — replaceability

**authority** `AGENTS.md:66` "Make invalid use unrepresentable or refuse it before opening
work." `AGENTS.md:68` "No hidden runtime feature flags or silent compatibility behavior."
`AGENTS.md:4` "Do not copy outside implementation code or add compatibility paths."
`docs/spec/ARCHITECTURE.md:864` the write store must "commit and recover accepted local
publication obligations and expose their current event materializations as a query source."

**implementation** `crates/fava-write-store/src/lib.rs:33-345`: 10 of the trait's 21
methods carry default bodies that stand in for an unimplemented provider.
`crates/fava-write-store/src/lib.rs:36-38`:
```rust
    /// Providers that do not yet support semantic custody report zero.
    fn active_capacity(&self) -> usize { 0 }
```
`crates/fava-write-store/src/lib.rs:50-53` and `:61-65` default `reserve_active` /
`release_active` to `Err(WriteStoreError::Refused("write store does not support active
reservations"))`. `crates/fava-publication/src/lib.rs:92` calls `reserve_active()`
unconditionally on the edit path.

**observable distinction** A third-party `WriteStore` that implements only the required
methods compiles, and `Fava::builder().write_store(store)...build()` returns `Ok`. Every
`fava.by(author).publish(edit)` then fails at runtime with
`PublishError::Publication(PublicationError::…Refused("write store does not support
active reservations"))` — the same `WriteStoreError::Refused(String)` shape used for a
transient capacity refusal, so the application cannot tell "this provider is structurally
incapable" from "retry later". The specified behavior is refusal *before* opening work,
i.e. a `BuildError`.

**proposed falsifier**
```rust
#[test]
fn assembly_refuses_a_write_store_that_cannot_carry_semantic_custody() {
    let err = Fava::builder()
        .event_cache(cache()).query_evaluator(eval())
        .write_store(Arc::new(MinimalWriteStore::default()))
        .signer(signer()).publisher(pub_()).delivery_policy(pol()).transport(tx())
        .build().unwrap_err();
    assert!(matches!(err, BuildError::WriteStoreLacksSemanticCustody));
}
```

**confidence** confirmed

---

### `live-assembly-accepted-then-refused-at-observe` — major — boundedness

**authority** `AGENTS.md:66` "Make invalid use unrepresentable or refuse it before opening
work." `crates/fava/src/lib.rs:251` documents the builder as "Static assembly builder.
No provider is silently selected." `docs/spec/ARCHITECTURE.md:2369` the facade owns
"builder, lifecycle, and ordering".

**implementation** `crates/fava/src/lib.rs:396-450`: `build()` never checks
`subscription_planner` or `transport`, and `BuildError`
(`crates/fava/src/lib.rs:453-475`) has no variant for either. The default query freshness
is `Freshness::Live` (`crates/fava-query/src/lib.rs:76-77`, `Query::default` at
`crates/fava-query/src/lib.rs:113`), so *every* ordinary query needs both. The refusal
lands at `crates/fava/src/routes.rs:17`/`:22` and `crates/fava/src/live.rs:23`/`:28`.

Related, in the same function: `crates/fava/src/lib.rs:377-380` infers whether publication
exists at all from whether any of four unrelated providers happened to be selected
(`publication_selected = publisher.is_some() || delivery.is_some() || !signers.is_empty()
|| !materializers.is_empty()`). Selecting none of them yields `publication: None`, and
every publication call then returns `PublicationError::NotConfigured` at runtime rather
than at assembly. That is an implicit runtime mode derived from provider presence.

**observable distinction**
`Fava::builder().event_cache(c).write_store(w).query_evaluator(e).build()` returns `Ok`,
and then `fava.observe(Query::events()).await` — the most ordinary call in the API —
returns `Err(ObserveError::Relay("live queries require a transport"))`. Nothing in the
type system or in `BuildError` warned the application.

**proposed falsifier**
```rust
#[test]
fn assembly_without_transport_cannot_claim_a_live_capable_engine() {
    let err = Fava::builder().event_cache(cache()).write_store(store())
        .query_evaluator(eval()).build().unwrap_err();
    assert_eq!(err, BuildError::MissingTransport);
}
```

**confidence** confirmed

---

### `canary-declares-itself-external-then-is-not` — major — behavioral proof

**authority** `apps/canary/README.md:3-5` "An ordinary downstream Rust application and
independent evidence lab. **It must not depend on Fava internal crates** or use Fava
diagnostics as the sole witness for external effects."
`AGENTS.md:38` "Build vertical slices through the public `fava` API".

**implementation** `apps/canary/Cargo.toml` declares direct path dependencies on
`fava-query:17`, `fava-routing:24`, `fava-state:25`, `fava-subscriptions:31`,
`fava-transport:32`, `fava-ingest:34`, `fava-publisher:36`, `fava-wire:37`,
`fava-write-store:38`. `apps/canary/src/grouping.rs` then uses them to *be* the client
engine rather than to inspect the relay side: `:10` imports
`fava_ingest::admit_subscription_event`; `:73` builds 300 `RelayDemand`s with
`demand_for_query`; `:248` calls `planner.plan(&key, demand)` directly; `:250` opens
`WebSocketTransport::default().open_session(key)` directly; `:254`/`:261` encode the
client's own `REQ`/`CLOSE` frames with `encode_client`; `:281` drives the read loop with
`decode_relay(&session.next_message().await?)`; `:291` performs admission itself with
`admit_subscription_event(cache, session.key(), &id, &id, filter, event, Timestamp::now())`.
This duplicates `crates/fava/src/relay.rs:170-211` and `:269`.

**observable distinction** The claim the canary is asserted to prove is not reachable
through the surface it claims to exercise. `apps/canary/scenarios.json:88-91` marks
`subscription-grouping-equivalence` `"status": "enabled"` for `"D-12 300 literal
tag-value queries prove 1-versus-300 exact result and relay-evidence equivalence"`, but
no query in that scenario is opened live: the 300 corpus queries are built `.cache_only()`
at `apps/canary/src/grouping.rs:173-175` and read back at `:356` from the
`MemoryEventCache` the canary filled itself at `:291`. Worse, the retained evidence is
not derived from the comparison at all — `apps/canary/src/grouping.rs:592-593` and
`:631-632` write `"result_equivalence": true` and
`"relay_source_evidence_equivalence": true` as **hard-coded literals** into
`manifest.json`, alongside `"grouped_reqs": 1` and `"case_isolation": true`, also
literals. The comparison at `:382-415` can only early-return an error, so those fields can
never be `false`, yet they read in the artifact as engine-produced facts.

**proposed falsifier**
```rust
#[test]
fn canary_does_not_link_engine_internal_crates() {
    let manifest = std::fs::read_to_string("apps/canary/Cargo.toml").unwrap();
    for internal in ["fava-ingest", "fava-wire", "fava-subscriptions", "fava-transport"] {
        assert!(!manifest.contains(internal), "canary must reach Fava only through `fava`: {internal}");
    }
}
```

**confidence** confirmed

---

### `canary-m7-scenarios-run-with-no-transport` — major — behavioral proof

**authority** `apps/canary/README.md:66-67` "The four M7 semantic-write canaries are
deterministic, memory-backed **public Fava executions**."
`docs/spec/ARCHITECTURE.md:3669` "every claimed boundary is validated by an external
implementation and an adversarial falsifier."

**implementation** `apps/canary/src/semantic_write_support.rs:61-75` defines
`struct NoopTransport` whose `open_session` always returns `ConnectionRefused`; it is
installed into the assembly at `apps/canary/src/semantic_write_support.rs:180`.
`:43-59` substitutes a canary `impl Publisher for RecordingPublisher` returning a canned
`PublishOutcome::Acknowledged`, installed at `:182`. Readback closes the loop inside the
canary: `apps/canary/src/semantic_writes.rs:411-416` admits the event into the cache
itself and `:449` reads it back with a `cache_only` query.
`apps/canary/src/semantic_write_store.rs:45`/`:51` wrap the write store in a canary
`impl QuerySource`/`impl WriteStore`.

**observable distinction** Nothing in the M7 evidence exercises real delivery, routing,
transport handoff, or ingest, yet the artifact and README present the runs as public Fava
executions. The write/materialization half is genuine; the "public execution" claim
covers a path in which three of the engine's providers have been replaced by the witness.

**proposed falsifier**
```rust
#[test]
fn m7_manifest_declares_every_substituted_provider() {
    let m: Value = serde_json::from_str(&read("runs/.../manifest.json")).unwrap();
    assert_eq!(m["providers"]["transport"], "NoopTransport(canary)");
    assert_eq!(m["providers"]["publisher"], "RecordingPublisher(canary)");
}
```

**confidence** confirmed

---

### `preview-parity-compares-two-router-lists` — minor — behavioral proof

**authority** `apps/canary/scenarios.json:132` "M6 side-effect-free preview and initial
publication route parity". `docs/spec/ARCHITECTURE.md:2380` names "route preview" as a
facade public-surface item.

**implementation** `apps/canary/src/automatic_publication.rs:249` clones the router list
into `preview_routers`; `:262`/`:342-344` compute the preview by calling
`fava_routing::preview(routers, &RouteRequest::Write(event))` on that canary-held clone;
the publish at `:267` runs through a separately constructed `Fava`. The engine's own
preview door for writes is deliberately closed (`crates/fava/src/lib.rs:67-73`
`compile_fail` doctest for `preview_write_routes`), so the direct call is sanctioned —
but the asserted parity is between two independently constructed router lists, not
between the engine's preview and the engine's publish.

**observable distinction** A facade whose preview and publish disagreed would still pass
this scenario, because the previewed routers are never the ones the engine used.

**proposed falsifier**
```rust
#[tokio::test]
async fn engine_preview_matches_engine_publish_route() {
    let fava = assemble_with_routers().await;
    let previewed = fava.preview_routes(&query).unwrap().destinations.keys().cloned().collect::<BTreeSet<_>>();
    let w = fava.publish(payload).unwrap();
    assert_eq!(previewed, w.receipt().unwrap().desired_destinations);
}
```

**confidence** confirmed

---

### `publish-payload-bound-is-unnameable` — minor — replaceability

**authority** `docs/spec/ARCHITECTURE.md:2377-2379` names the facade's publication door as
"`publish(payload)` for unsigned events, replaceable-event edits, or pre-signed events".
`docs/spec/partial-spec-api-semantics.md` rule 5: "Protocol helpers lower to core
expressions."

**implementation** `crates/fava/src/publication.rs:232`
`pub(crate) trait PublishPayload` is private, while `crates/fava/src/lib.rs:131-141` and
`crates/fava/src/publication.rs:164-171` are public generic functions bounded by it,
compiled only via `#[allow(private_bounds)]`.

**observable distinction** `Fava::publish` has a public signature whose bound cannot be
named, imported, or documented from outside the `fava` crate. rustdoc renders a bound the
reader cannot follow; an application cannot write a generic wrapper
`fn send<P: ???>(f: &Fava, p: P)`, and cannot discover the accepted payload set from the
type system. The behavior (three payload kinds) is correct; the surface is not expressible.

**proposed falsifier**
```rust
#[test]
fn the_publication_door_bound_is_public_vocabulary() {
    fn wrapper<P: fava::PublishPayload>(f: &fava::Fava, p: P) -> Result<fava::Write, fava::PublishError> {
        f.publish(p)
    }
    let _ = wrapper::<fava::UnsignedEvent>;
}
```

**confidence** confirmed

---

### `facade-lacks-nameable-assembly-vocabulary` — minor — replaceability

**authority** `docs/spec/ARCHITECTURE.md:3630` "| `fava` | Thin public facade **and
assembly builder**. |"; `:2420` "`fava` depends on contracts and universal owners.";
`:2374` the facade owns "handles to the selected owners and providers"; `:3669`
"Applications select providers at build/construction time".

**implementation** `crates/fava/src/lib.rs:14-39` re-exports values and errors but not a
single provider contract. The builder methods at `crates/fava/src/lib.rs:268`, `:278`,
`:288`, `:298`, `:308`, `:318`, `:335`, `:372`, `:382` are bounded by `EventCache`,
`WriteStore`, `QueryEvaluator`, `SubscriptionPlanner`, `Transport`, `Router`, `Signer`,
`Publisher`, and `DeliveryPolicy` — none of which `fava` re-exports.
(`ReplaceableEventMaterializer` is the sole exception, re-exported at
`crates/fava/src/lib.rs:31-37`.)

**observable distinction** An application crate depending only on `fava` cannot write
`fn build(t: Arc<dyn Transport>) -> Fava`, cannot store `Arc<dyn Router>` in its own
config struct, and cannot use `FavaBuilder::routers(impl IntoIterator<Item = Arc<dyn Router>>)`
at all, because it cannot name `Router`. The measured consequence is
`apps/canary/Cargo.toml`, which adds eight contract-crate dependencies purely to assemble
an engine.

**proposed falsifier**
```rust
// in a test crate whose only fava dependency is `fava`
#[test]
fn every_builder_contract_is_nameable_from_the_facade() {
    fn _f(_: std::sync::Arc<dyn fava::Transport>, _: std::sync::Arc<dyn fava::Router>,
          _: std::sync::Arc<dyn fava::EventCache>, _: std::sync::Arc<dyn fava::WriteStore>) {}
}
```

**confidence** confirmed

---

### `oversize-code-files-without-a-stated-cohesion-reason` — minor — convention

**authority** `AGENTS.md:60-62` "Code files have a 500-line soft limit and an 800-line
hard limit. The limits apply only to code, not documentation or other artifacts.
**Crossing 500 lines requires a concrete cohesion reason**; no code file may cross 800 lines."

**implementation** `find crates apps -name '*.rs' | xargs wc -l | awk '$1>500'` returns
exactly twelve files. Six state a cohesion reason in their module header
(`crates/fava/tests/simple_groups.rs:3-4`, `apps/canary/src/croissant.rs:3-4`,
`apps/canary/src/croissant_nip02.rs:3-5`,
`apps/canary/src/croissant_simple_groups_evidence.rs:3-4`,
`apps/canary/src/croissant_simple_groups_tests.rs:3-4`,
`crates/fava/tests/semantic_write_store.rs:3-5`). Six do not:

| File | Lines |
|---|---|
| `crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs` | 666 |
| `apps/canary/src/grouping.rs` | 647 |
| `crates/fava-simple-groups/src/tests/records.rs` | 634 |
| `crates/fava-query-standard/tests/source_merge.rs` | 568 |
| `crates/fava-write-store-memory/src/semantic.rs` | 535 |
| `apps/canary/src/lib.rs` | 505 |

**observable distinction** Not application-observable; this is the convention gate, which
`AGENTS.md` makes a per-change requirement. Recorded as minor accordingly. No file crosses
800, so the hard limit is clean.

**proposed falsifier**
```rust
#[test]
fn oversize_code_files_state_their_cohesion_reason() {
    for (path, lines) in rust_files_over(500) {
        let head = read_head(&path, 8);
        assert!(head.contains("500-line soft limit") || head.contains("Cohesion:"),
                "{path} is {lines} lines with no stated cohesion reason");
    }
}
```

**confidence** confirmed

---

### `slow-consumer-scenario-is-mislabelled` — minor — behavioral proof

**authority** `apps/canary/scenarios.json:64-67` files `slow-consumer-latest-state` under
`"milestone": "M3"` with `"M3 bounded coalesced latest-state observation"`.
`docs/spec/partial-spec-api-semantics.md:326` "The public contract is latest state, not a
required queue of every intermediate mutation. A slow application MAY skip intermediate
states, but it MUST eventually receive an exact current state reflecting all accepted
changes relevant to the query."

**implementation** `apps/canary/src/local.rs:46-87`, reached from
`apps/canary/src/main.rs:170`, starts no relay, opens `Query::events().cache_only()`
(`apps/canary/src/local.rs:48`), and drives state with `writes.accept_materialized`
(`:67`). No relay arrival, no live coalescing, no bounded delivery under relay pressure.

**observable distinction** The coalescing promise is about a slow consumer against a fast
*live* source; the evidence only exercises a slow consumer against synchronous local
inserts. An engine that coalesced correctly locally and dropped live revisions would pass.

**proposed falsifier**
```rust
#[tokio::test]
async fn slow_consumer_receives_exact_current_state_under_live_relay_pressure() {
    let mut obs = fava.observe(Query::events()).await.unwrap(); // Live
    inject_n_relay_events(&relay, 5_000).await;
    sleep_without_reading(&mut obs).await;
    assert_eq!(obs.changed().await.unwrap().events.len(), 5_000);
}
```

**confidence** confirmed

---

### `local-issue-records-assert-gates-that-are-red` — minor — behavioral proof

**authority** `AGENTS.md:24-26` "Brevity never overrides rigor. Preserve actionable
distinctions, measured results, uncertainty, and verified evidence; **never claim absence
without a search that returned empty**." `AGENTS.md:57` "Run
`python3 tools/check_vocabulary.py` and its unit tests for every architectural or
public-API change."

**implementation** Eight documents in `docs/` record the vocabulary gate as passing:
`docs/issues/0018-literal-tag-value-filters.md:108` ("`python3 tools/check_vocabulary.py`
— exit 0"), and the same claim at `docs/issues/0019-simple-groups.md:99`,
`0015:74`, `0014:89`, `0013:58`, `0011:71`, `0003:49`, `0001:40`;
`docs/internals/README.md:11-16` presents it as the runnable check. Running it at HEAD
exits 1. One of its four diagnostics (`fava-canary`) originates from **tracked** files —
`.planning/phases/07.1.1-.../07.1.1-REVIEW.md` and
`.planning/phases/07.2-.../07.2-02-PLAN.md` — so it fails on a clean checkout, not only
in this working tree.

Two further numeric claims in the same corpus are simply wrong:
`docs/issues/0003-protocol-first-vocabulary.md:51` "all code files are at or below 500
lines" (twelve exceed it, up to 786), and
`docs/issues/0018-literal-tag-value-filters.md:97` "`apps/canary/src/grouping.rs` is 540
lines" (it is 647 — and 647 is above the soft limit with no cohesion header in the file,
see `oversize-code-files-without-a-stated-cohesion-reason`).
`docs/issues/0017-routers-required-at-assembly.md:67` states "Three assemblies select
publication with zero routers"; at least fourteen do
(`crates/fava/tests/{explicit_publication,publication_door,publication_scopes,write_settlement}.rs`,
`crates/fava/tests/support/{semantic_write,semantic_write_capability_lifecycle,semantic_write_capability_protocol}.rs`,
`crates/fava/tests/semantic_write_failures/{source_isolation,support,transient_reads}.rs`,
`crates/fava/tests/semantic_write_publication/{author,winner_order}.rs`,
`apps/canary/src/{publication,publication_child,semantic_write_support,croissant_nip02,croissant_simple_groups_flow}.rs`).
`docs/issues/0006-ordered-automatic-routing.md:46-47` still says the grouping canary
"proves one grouped REQ and **three** no-grouping REQs"; the count is 300
(`apps/canary/src/grouping.rs:32`).

**observable distinction** Not application-observable. It is the audit-integrity failure
the brief names: recorded evidence asserts a gate is green while the gate is red, so a
future change that trips the same gate will look like a regression introduced by that
change rather than a standing failure.

**proposed falsifier**
```rust
#[test]
fn the_vocabulary_gate_is_actually_green() {
    let out = std::process::Command::new("python3").arg("tools/check_vocabulary.py").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}
```

**confidence** confirmed

---

## Deliverable 6 — Docs drift (`README.md`, `docs/` excluding `docs/spec/`, crate READMEs)

**Root `README.md` is accurate.** Every clause of `README.md:5-14` was checked against
code: merged local sources, verified live relay events, exact provenance, fresh reconnect
request identity, ordered automatic router chain, durable acceptance of unsigned and
verified signed events, explicit-or-chain write routing, immediate delivery to known
relays while discovery is unresolved, exact per-relay receipts, pre-handoff cancellation,
and resume after process death. No drift found. Note that `README.md` makes **no** claim
about sharing relay work between equivalent observations, about cancellation of a
partially opened observation, or about subscription grouping — the three places the code
is weakest. It is accurate by omission, not by coverage.

`crates/fava-nip02/README.md`, `crates/fava-simple-groups/README.md`, and
`docs/internals/README.md`'s scan-scope description are accurate; every symbol, bound
(4,096 group rows at `crates/fava-simple-groups/src/bounds.rs:3`), and cited evidence file
exists.

Drift found, in `docs/issues/`:

| Doc claim | Code | App-observable difference |
|---|---|---|
| `docs/issues/0005-multi-relay-observation.md:17-18` "One explicit relay set opens one independently cancellable relay task **per exact `RelaySessionKey`**" | `crates/fava/src/live.rs:31-54` opens unconditionally per `observe` call; neither `Fava` (`crates/fava/src/lib.rs:83-93`) nor `Observer` (`crates/fava-observe/src/lib.rs:14-19`) holds a registry | Two identical `fava.observe(q).await` calls open two WebSocket sessions and two REQs to the same relay. The scope is per-observation, not per-`RelaySessionKey`. |
| `docs/issues/0006-ordered-automatic-routing.md:9-10` "withdraws a relay **only when the current merged plan no longer selects it**" | `crates/fava/src/routes.rs:93-98` and `:102-108` `break` on any router-session error or plan-bound violation, falling into `:130-133` which cancels **every** entry in `active` | One oversized or invalid router contribution silently CLOSEs all live subscriptions for that observation, while the `Observation` handle stays open and `changed()` never errors. Already filed as `chain-collapse-tears-down-all-relay-demand`; this is the doc side of it. |
| `docs/issues/0018-literal-tag-value-filters.md:52-53`, `:94` "300 compatible tag-value logical queries that **share one wire request**" | `crates/fava/src/relay.rs:180` always passes one demand, and `crates/fava/src/relay.rs:241` **actively rejects** any REQ where `filters.len() != 1` as "subscription planner attribution does not match its REQ" | An application selecting `StandardSubscriptionPlanner` and opening 300 compatible observations gets 300 REQs. A conforming planner that *did* group would be refused by the facade. |
| `docs/issues/0016-runtime-handle-at-assembly.md:14-23` quotes a five-line `Publication::accept` at `crates/fava-publication/src/lib.rs:62-73` | `accept` is now `crates/fava-publication/src/lib.rs:87-131` with a full `WritePayload::Edit` branch; `recover` moved `:81` -> `:138`; the facade's `recover()` call moved `:384` -> `crates/fava/src/lib.rs:430` | Record-only. The substantive claim (the `Handle::try_current()` guards at `crates/fava-publication/src/lib.rs:88`, `:139`, `crates/fava/src/query_source.rs:15`) is still exactly true. |
| `docs/issues/0017-routers-required-at-assembly.md:14-22` quotes `publication_selected` without the `|| !self.materializers.is_empty()` term | `crates/fava/src/lib.rs:377-380` includes it | Acting on 0017 as written would miss the materializer-only assemblies the proposed refusal would newly break. |

## Confirmations of findings already filed elsewhere (not new)

These are re-derived from the public-surface angle and add evidence; the existing id owns
the finding.

- `no-facade-close-or-command-admission` / `runtime-no-shutdown-join` /
  `runtime-detached-tasks` — **public-surface evidence**: `docs/spec/ARCHITECTURE.md:2401`
  lists "deterministic close and destructive reset" as a facade public-surface item and
  `:2991` assigns "Public engine lifecycle" to `fava`. `crates/fava/src/lib.rs` contains no
  `close`, `shutdown`, `reset`, or `impl Drop for Fava`, while the workspace has ten
  detached `tokio::spawn` sites with zero join handles:
  `crates/fava-observe/src/lib.rs:113`, `crates/fava-publication/src/delivery.rs:67`,
  `crates/fava-publication/src/run.rs:40` and `:437`,
  `crates/fava-routing/src/chain.rs:86` and `:95`, `crates/fava/src/query_source.rs:25`,
  `crates/fava/src/routes.rs:53` and `:158`, `crates/fava/src/live.rs:59`.
- `single-demand-per-relay-defeats-the-planner-contract` / `singleton-demand-per-plan` /
  `grouping-unprovable-through-observe` — **added evidence**: `crates/fava/src/relay.rs:180`
  is the sole call site, and it is unconditional:
  `planner.plan(session_key, &[demand_for_query(subscription, query)])`. The public path
  can therefore never hand the planner more than one demand, which is why the only evidence
  for the grouping promise had to be written outside it (see
  `canary-declares-itself-external-then-is-not` above). **New evidence**: the facade does
  not merely fail to group, it actively refuses grouping —
  `crates/fava/src/relay.rs:241` rejects any planner REQ where `filters.len() != 1` with
  "subscription planner attribution does not match its REQ". A conforming
  `StandardSubscriptionPlanner` that grouped two compatible demands would be refused by
  `fava`. This also extends `validate-plan-private-conformance`.
- `unbounded-reconnect-storm` — **added evidence**: the reconnect backoff is a hardcoded
  `50ms` literal at `crates/fava/src/relay.rs:135` inside the facade, with no bound on
  attempts and no provider or builder input that can change it. It is not reachable from
  any public contract.
- `facade-owns-ingest-pipeline` — **added evidence at the manifest level**:
  `crates/fava/Cargo.toml:10` (`fava-ingest`) and `:20` (`fava-wire`). The single-owner map
  lists wire-grammar consumers as "transport, publisher, ingest, test tools"
  (`docs/spec/ARCHITECTURE.md:2965`) and ingest consumers as "observations, cache,
  publication reconciliation" (`:2966`); the facade appears in neither list.
- `event-cache-contract-forces-full-materialization` — **added evidence**: the unbounded
  read is `crates/fava-event-cache/src/lib.rs:65`
  (`fn events(&self) -> Result<Vec<CachedEvent>, EventCacheError>`), and the default
  `admit` (`:23`) and `expire` (`:38`) call it once per operation, against a contract that
  `docs/spec/ARCHITECTURE.md:207` says may be "persistent, or backed by a remote service".
- `post-open-evaluation-failure-is-silent` — **added evidence**:
  `crates/fava-observe/src/lib.rs:156` (`let Ok(mut snapshot) = evaluator.evaluate(..) else { break; }`)
  and `:159` (revision overflow `break`) both collapse into the same `ObservationClosed`
  the application gets from a deliberate `close()`; `QueryEvaluationError::Refused(String)`
  is dropped on the floor.
- `vocab-reexports-invisible` — **two concrete symbols**: `fava::SingleLetterTag`
  (re-exported through `crates/fava/src/lib.rs:21-23` from
  `crates/fava-query/src/selection.rs:3 pub use nostr::filter::SingleLetterTag`) and
  `fava_wire::SubscriptionId` (`crates/fava-wire/src/lib.rs:3`) have **zero** entries in
  `docs/internals/vocabulary.toml`, while their siblings `ClientMessage`, `RelayMessage`,
  `Filter`, and `EventValue` are registered. `tools/check_vocabulary.py:14-17` matches only
  `pub struct|enum|trait|type`, so `pub use` is structurally invisible — the omission is a
  gate hole, not a policy decision.
- `vocab-planning-md-is-authority` — **currently red**: `python3 tools/check_vocabulary.py`
  exits non-zero today with `undocumented specified architectural crate: fava-canary`.
  The string `fava-canary` appears nowhere under `docs/`; it is pulled in from
  `.planning/**/*.md` by `tools/check_vocabulary.py:214-215`. `AGENTS.md:57` requires
  running this checker for every architectural or public-API change, so the gate is
  failing on a phantom crate name and cannot flag a real one without being ignored.

## Conforming (verified, not merely unexamined)

- **No downcasting anywhere in `crates/`.** `grep -rn 'downcast\|dyn Any\|any::type_name'`
  over all of `crates` returns zero matches. The single repository hit is
  `apps/canary/src/croissant_simple_groups.rs:406-410`, recovering a panic payload.
- **No feature-gated behavior anywhere.** `grep -rn '#[cfg(feature'` over `crates` and
  `apps` returns zero matches, and no `Cargo.toml` in the workspace or in `apps/canary`
  declares a `[features]` table. `AGENTS.md:68` is satisfied on the feature-flag half.
- **No universal owner or facade depends on an implementation crate.** Every one of the 37
  crate manifests plus `apps/canary/Cargo.toml` was read; `fava`, `fava-ingest`,
  `fava-observe`, `fava-publication`, and `fava-diagnostics` have zero regular
  `[dependencies]` edges to any `-standard`, `-memory`, `-redb`, `-websocket`, `-local`, or
  `-no-grouping` crate.
- **No contract crate depends on any implementation crate,** and no runtime dependency
  cycle exists. The only cycles are two Cargo-legal dev-dependency loops
  (`fava` <-> `fava-write-store-redb`, `fava` <-> `fava-simple-groups`); `grep -rn 'fava::'
  crates/fava-simple-groups/src` returns nothing, confirming the protocol crate's *source*
  is facade-free.
- **Every provider contract has a separate implementation crate** (event-cache/memory,
  write-store/memory+redb, query/query-standard, subscriptions/standard+no-grouping,
  transport/websocket, publisher/nip01, delivery/standard, signer/local, routing/four
  routers). No implementation lives inside its contract crate.
- **No code file exceeds the 800-line hard limit.** Largest is
  `crates/fava/tests/simple_groups.rs` at 786.
- **Every public nominal type in `crates/` is registered vocabulary.**
  `tools/check_vocabulary.py` cross-checks `pub struct|enum|trait|type` in both directions
  and reports no undocumented and no stale nominal symbol; the only diagnostic is the
  `fava-canary` phantom above. `fava-simple-groups` (108 public items) and `fava-nip02`
  (26) are fully registered.
- **`fava-simple-groups` and `fava-nip02` match `partial-spec-api-semantics.md` section 10
  item-for-item.** Both return `Query` and pure projections; neither returns a
  protocol-specific observation type; neither depends on the other
  (`crates/fava-nip02/Cargo.toml`, `crates/fava-simple-groups/Cargo.toml`); group
  publication goes through `fava.to(group.hosts()).publish(payload)` rather than a private
  receipt lifecycle. `Group::events` lowers to `from_relays`, `Group::records` to
  `only_from_relays`, exactly as specified.
- **`Query` is valid by construction.** All fields are private
  (`crates/fava-query/src/lib.rs:91-104`); `limit(0)` and empty explicit relay sets are
  refused at `:166` and `:229-236`. `from_relays` vs `only_from_relays` produce distinct
  `ResultAuthority` values (`:132-153`), so source policy is part of query identity as
  `partial-spec-api-semantics.md` rule 9 requires.
- **`EventRecord`, `QuerySnapshot`, `QueryEvidence` match their specified shapes**
  (`crates/fava-query/src/lib.rs:339-417` vs `partial-spec-api-semantics.md:249-262` and
  `:329-347`). `Row` appears nowhere in the public surface (rule 8 satisfied).
- **Canary scenarios that do exercise the real public live path**, verified by reading
  each: `run_live_scenario` (`apps/canary/src/live.rs:118`), `run_m3_live_scenario`
  (`multi.rs:262`), `run_routing_scenario` (`routing.rs:239`), `run_publication_scenario`
  (`publication.rs:390`), `run_crash_child` (`publication_child.rs:41`),
  `run_croissant_nip02_scenario` (`croissant_nip02.rs:314`),
  `run_croissant_simple_groups_scenario` (`croissant_simple_groups_flow.rs:383`). These
  open `fava.observe(..)` at default `Freshness::Live` over a real `WebSocketTransport`
  against a real third-party relay, and publish through `fava.to(..).publish(..)`.
  `run_real_relay_smoke` and `run_public_recon` make no Fava claim and are honestly
  labelled as lab/reconnaissance.

## Open questions

1. `SourceKind` is approved vocabulary (`docs/internals/vocabulary.toml:300`) but does not
   appear in the specified `SourceSnapshot` (`docs/spec/ARCHITECTURE.md:622`). Is the role
   tag intended to be part of the contract at all, or is it an implementation convenience
   that the vocabulary gate blessed after the fact? The answer decides whether
   `source-role-impersonation` is fixed by removing the field or by making the source set
   an ordered registry with provider-supplied identity.
2. `docs/spec/ARCHITECTURE.md:2391` lists `fetch_cache(...)` and `services(...)` builder
   methods and `:2398-2400` lists session/account operations, sign-without-publish, and
   NIP-42 attachment as facade public surface. These are absent, but so are the crates that
   would own them. I treated this as sequencing (`FAVA_REWRITE_IMPLEMENTATION_PLAN.md`
   owns it) rather than drift. If the orchestrator disagrees, each is a separate missing
   public-surface item.
3. `crates/fava-subscriptions-no-grouping` is registered vocabulary
   (`docs/internals/vocabulary.toml:684`, `:690`) but absent from the ARCHITECTURE crate
   inventory table (`:3640-3653`). Should the inventory table be the crate authority, or is
   `vocabulary.toml` sufficient? Filed as a question rather than a finding because the two
   authorities do not actually contradict — the table is described as "concise".
4. `apps/canary` is a separate Cargo workspace, so `tools/check_vocabulary.py` never scans
   its public symbols. Intentional (it is a downstream application) or a gate gap?
