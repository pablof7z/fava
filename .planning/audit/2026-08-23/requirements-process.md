# Requirements and planning-process audit

**Area slug:** `requirements-process`
**Date:** 2026-08-23
**Mode:** read-only. The only file written is this report.

---

## Scope checked

**Authorities read in full**

- `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` (1711 lines, all 131 numbered requirements extracted by ID)
- `docs/spec/ARCHITECTURE.md` — Part IX ownership ledger (2961–2995), ordering-owner rules (2997–3037), one-owner-per-responsibility (66–79), crate families (270–283), Part XII falsifiers A–P (3131–3444), crate inventory (3567–3663)
- `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` — M0–M11 goals, required behavior, canary scenarios, **exit gates**, falsifiers; §7.5 documentation gates
- `AGENTS.md`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` (headings + evidence rules)

**Planning artifacts read in full**

- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/PROJECT.md`, `.planning/STATE.md`, `.planning/HANDOFF.json`
- `.planning/codebase/{ARCHITECTURE,CONCERNS,STRUCTURE,STACK,CONVENTIONS,TESTING,INTEGRATIONS}.md`
- `.planning/research/{ARCHITECTURE,SUMMARY,PITFALLS}.md`
- All 9 `*-VERIFICATION.md`; `07.1.1-VALIDATION.md`, `07.1.1-COVERAGE.md`, `06.1-VALIDATION.md`; sampled `*-SUMMARY.md` (36 total); `.planning/debug/observe-ownership-collapse.md`
- `.planning/todos/pending/*`

**Repository facts established by command, not assertion**

- `git log --diff-filter=A -- .planning/REQUIREMENTS.md` and per-milestone commit timestamps
- `git log -- docs/issues/000{1,4,5,6,7,8}-*.md` (evidence-vs-implementation commit identity)
- `ls apps/canary/runs/`, `cat .gitignore` (existence of cited external evidence)
- `ls crates/` (existence of ledger-named owners)
- Spot-read `crates/fava/src/lib.rs:108-114`, `crates/fava/src/live.rs:20-60`, `crates/fava/tests/observation_bounds.rs:1-47`, `crates/fava-routing/src/chain.rs:445-462`, `crates/fava-transport-websocket/{src/lib.rs,tests/conformance.rs}`, M1 crate `Cargo.toml` dependency sets

Findings that are purely document-vs-document are marked as such; the brief's `implementation` field then carries the exact `.planning/…:LINE` that stands in for the code, and I add a code line wherever one exists.

---

## Executive verdict

The requirements corpus is not a specification of the product. It is a **description of the code that already existed when it was written.**

`.planning/REQUIREMENTS.md` was first committed at `277d839` on **2026-08-21 07:44:48 +0300**. M6 was completed at `309e421` on **2026-08-21 04:03:09 +0300**. All 66 of the LOCAL/READ/ROUTE/WRITE requirements that M1–M6 are graded against were authored **3 hours and 41 minutes after the last of those milestones shipped.** They were born checked.

That single fact explains every downstream failure in this area:

- A requirement written from working code cannot fail. LOCAL-08/READ-02 is not an unlucky split; it is what reverse-engineering a spec from an implementation *produces*.
- 113 of the 131 authoritative spec requirement IDs are referenced **nowhere** in `.planning/`. There is no traceability edge from `QUERY-004` to `LOCAL-08`, so nobody could see the conjunct fall out.
- The codebase map commit `b184aae` (08:44:48) recorded 8 known bugs and 5 High-priority test-coverage gaps in M1/M3/M5 behavior. The reconciliation commit `da8db46` (08:45:08) — **20 seconds later** — declared "No M1 gaps remain", "No M3 gaps remain", "No M5 gaps remain".
- Every external-process artifact cited by the M1–M6 and 06.1 and 07.1 verdicts is absent from disk and git-ignored. Those verdicts are today unreconstructable.

**Milestone verdicts that must be revoked: M1, M2, M3, M5, M6, and Phase 07.1.1.** M4 and Phase 7 are downgradeable rather than revocable. Detail in §5 and §7.

---

## Findings

### req-authored-after-implementation — critical — behavioral proof

**Authority.** `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` and `.planning/REQUIREMENTS.md:181` Definition of Done item 2, which the project adopted as binding: *"The smallest executable proof failed before implementation for the intended reason."* `AGENTS.md` gate 6: *"public promises have falsifiable evidence at the owning component, through the real public path."*

**Implementation.** Repository chronology, verified by command:

| Artifact | Commit | Timestamp (+0300) |
|---|---|---|
| M1 complete | `6be0fa5` | 2026-08-21 01:08:52 |
| M2 complete | `7fac920` | 2026-08-21 01:37:08 |
| M3 complete | `1f2c0ed` | 2026-08-21 01:57:30 |
| M4 complete | `9860711` | 2026-08-21 02:29:56 |
| M5 complete | `7e5820f` | 2026-08-21 03:21:43 |
| M6 complete | `309e421` | 2026-08-21 04:03:09 |
| **`.planning/REQUIREMENTS.md` created** | **`277d839`** | **2026-08-21 07:44:48** |
| `.planning/ROADMAP.md` created | `38e3270` | 2026-08-21 07:54:40 |
| Codebase map refreshed | `b184aae` | 2026-08-21 08:44:48 |
| Phase 1–6 VERIFICATION backfilled | `da8db46` | 2026-08-21 08:45:08 |

`.planning/REQUIREMENTS.md:15` states the intent plainly: *"M1-M6 are also complete; their 66 requirements are checked below from the focused milestone records, implementation commits, current validation, and retroactive phase verification reports."* The requirements were **derived from** the implementation commits they are used to grade.

**Observable distinction.** An application relying on `QUERY-004` (no relay wait) gets a hang, because the requirement that would have caught it (`LOCAL-08`) was written to describe `Observer::open` — the local path — rather than `Fava::observe`. `crates/fava/src/live.rs:31` constructs the local `Observation`, then lines 32–53 serially `await OpenedRelay::open(...)` for every relay before returning it. No requirement in `.planning` forbids that, because the requirement author had that code in front of them.

**Proposed falsifier.** Process gate, not a Rust test: add a CI check `tools/check_requirement_provenance.py` asserting that for every requirement marked Complete, `git log --diff-filter=A -- .planning/REQUIREMENTS.md` predates the earliest commit cited as its evidence. It fails today for all 66 M1–M6 requirements.

**Confidence.** `confirmed`.

---

### req-no-traceability-to-spec-ids — critical — behavioral proof

**Authority.** `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` defines 131 numbered normative requirements (`GOAL-001`…`PROFILE-008`, plus `OPEN-001..005`). `.planning/PROJECT.md:71` binds the project: *"`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` owns required behavior — implementation and planning must not weaken its distinctions."*

**Implementation.** `.planning/REQUIREMENTS.md` (370 lines) invents a parallel ID space (`LOCAL-*`, `READ-*`, `ROUTE-*`, `WRITE-*`, `CAP-*`, `GROUP-*`, `SESSION-*`, `HARD-*`, `PROF-*`, `SUB-*`, `NATIVE-*`) and contains **zero** references to any spec ID. Sweeping all 131 spec IDs across the whole of `.planning/`:

- **113 appear nowhere at all.**
- 18 appear, and 12 of those only inside phase `RESEARCH.md` prose or the untracked `.planning/debug/` file.
- Only `QUERY-001`, `QUERY-004`, `RELAY-003` appear in more than one planning file.

There is no mapping table anywhere from `QUERY-004` → `LOCAL-08`, from `QUERY-002` → `LOCAL-09`, from `EVENT-006` → `LOCAL-07`, or from any of the other 128.

**Observable distinction.** Without the edge, a reviewer cannot mechanically ask "is every clause of `QUERY-004` covered?" The specific consequence: `QUERY-013` ("relay demand begins at open") and `QUERY-004` ("initial view never waits on a relay") both mapped, informally and in different heads, onto two requirements in two different milestones, and their conjunction was never anyone's responsibility.

**Proposed falsifier.** `tools/check_requirement_traceability.py`: parse `^## [A-Z]+-[0-9]+[A-Z]?` from the goals spec, require each to appear in a `Spec basis` column of `.planning/REQUIREMENTS.md`, fail on any unmapped ID. Fails today with 113 unmapped.

**Confidence.** `confirmed`.

---

### req-conjunction-local08-read02 — critical — ownership

**Authority.**
`GOALS.md:313` — *"The initial query value MUST be produced from the configured local query sources without waiting for any relay response."*
`GOALS.md:325` — *"**Acceptance:** with every relay unreachable, opening a query returns its local view or a local-source error, never hangs waiting for the network."*
`GOALS.md:459` — *"Opening a live-freshness query MUST contribute relay demand immediately."*
`ARCHITECTURE.md:2972` — *"| Open live-query handle | `fava-observe` | facade/SDK handle |"*

**Implementation.**
`.planning/REQUIREMENTS.md:29` — *"**LOCAL-08**: Opening a **local** query is all-or-nothing and returns one complete current snapshot without waiting for relay work."* → mapped to **Phase 1**.
`.planning/REQUIREMENTS.md:38` — *"**READ-02**: Opening a live query starts relay work immediately when live freshness is requested."* → mapped to **Phase 2**.

Two weakenings compound:

1. `QUERY-004` says *"the initial query value"* — of any query. LOCAL-08 says *"a **local** query"*. The qualifier `local` restricts the requirement to exactly the regime where no relay exists to wait for.
2. LOCAL-08 lands in M1, whose own exit gate (`FAVA_REWRITE_IMPLEMENTATION_PLAN.md:349`) is *"No relay, transport, or runtime networking dependency exists in these crates."* I confirmed by reading the eight M1 `Cargo.toml` files that this gate holds. So LOCAL-08 was **assigned to the one milestone that is structurally incapable of falsifying it.**

The conjunction — *starts relay work immediately **while** returning the coherent local observation without waiting* — exists in no requirement, no roadmap success criterion, and no phase verification. Code proof it fails: `crates/fava/src/live.rs:31` opens the local observation, `:32-53` then serially awaits `OpenedRelay::open(...)` per relay and only `:55-59` returns the handle.

**Observable distinction.** With a `Transport` whose `open_session` never resolves, `Fava::observe(live_query)` never returns, while the identical query with `.cache_only()` returns instantly. `QUERY-004` requires both to return.

**Proposed falsifier.**
```rust
#[tokio::test(flavor = "current_thread")]
async fn live_open_returns_local_view_while_relay_establishment_is_pending() {
    let fava = assembly_with_transport(PendingOpenTransport::new()); // open_session never resolves
    seed_cache(&fava, signed_event());
    let obs = timeout(Duration::from_millis(200), fava.observe(Query::events().live().to(["wss://x"])))
        .await.expect("handle must not wait on relay establishment").expect("opens");
    assert_eq!(obs.current().events.len(), 1);              // QUERY-004: local view present
    assert_eq!(PendingOpenTransport::open_attempts(), 1);   // QUERY-013: relay work started anyway
}
```
Fails today (times out at the `timeout`).

**Confidence.** `confirmed`.

---

### req-lost-conjunctions-catalogue — critical — behavioral proof

LOCAL-08/READ-02 is not unique. Every case below is a single spec invariant whose conjuncts were split across requirement IDs or milestones such that no artifact tests the conjunction. All are document-vs-document with exact quotes on both sides.

**(a) QUERY-002 shared work.**
Authority `GOALS.md:296` — *"Equivalent observations MAY share local evaluation, relay connections, and wire subscriptions."* `GOALS.md:298` — *"**Acceptance:** two equivalent handles share work; closing one does not close work still needed by the other."*
Planning `.planning/REQUIREMENTS.md:30` — *"**LOCAL-09**: Equivalent query descriptions, including access context, acquisition scope, and result authority, have stable semantic identity."*
Only the *identity* half survived, and it landed in M1. The *sharing* half and the *close-safety* half vanished from M2 and M3 entirely. `MAY` in the body was read as optional and the normative `Acceptance` line was discarded with it. Consequence: `crates/fava/src/live.rs:33` allocates a fresh `RelaySessionKey` and a fresh `OpenedRelay` per observation, so two equivalent handles open two sessions and send two `REQ`s, and no requirement objects.

**(b) QUERY-003 refusal leaves no relay work.**
Authority `GOALS.md:305` — *"return a typed refusal and leave no ownerless demand, partial dependency, or relay work."* `GOALS.md:309` — *"**Acceptance:** injected failure during open leaves existing queries unchanged and creates no leaked subscription."*
Planning: LOCAL-08 keeps only the phrase *"all-or-nothing"*, again scoped to local queries. `READ-09` covers cancellation and `READ-10` covers close; **neither covers failure during open of the relay path.** The M2/M3 requirement set has no "open failed, nothing leaked" clause. Code: `crates/fava/src/live.rs:47-52` does attempt cleanup, but partial-open leak is a confirmed baseline finding in the brief and no requirement would have caught it.

**(c) QUERY-012 pull semantics — four of eight conjuncts lost.**
Authority `GOALS.md:444-451` enumerates eight invariants. Planning covers three: wake-on-cancel and post-cancel non-delivery (`READ-09`, `:45`), idempotent close (`READ-10`, `:46`), no waiter backlog (`READ-18`, `:54`). **Absent from all 129:** `GOALS.md:445` *"a second concurrent pull is refused without consuming data"*; `:447` *"an update delivered once is never delivered again"*; `:448` *"invalid acknowledge/cancel/close ordering is refused"*; `:451` *"shutdown ends all pending pulls without hanging."* Splitting one requirement across three IDs dropped half of it.

**(d) QUERY-011 memory bound.**
Authority `GOALS.md:436` — *"Observation memory MUST remain bounded even when an application is slow."*
Planning `.planning/REQUIREMENTS.md:31` (`LOCAL-10`) — bounded *delivery*, coalescing, rebasing. `.planning/REQUIREMENTS.md:52` (`READ-16`) — *"an exact **bounded latest result**"*. A bound on the *result value* is not a bound on *observation memory*. The memory conjunct exists only in `READ-20` (`:56`), whose evidence is discussed in `req-read20-unfalsifiable` below and measures no memory at all.

**(e) QUERY-013 anti-duplication.**
Authority `GOALS.md:461` — *"Cache-only queries contribute no relay work. Reiterating an already-open handle does not create another underlying query."*
Planning `READ-02` (`:38`) keeps only the first sentence of `QUERY-013` and drops both clauses of line 461. The second clause is the same shared-work property lost in (a) — so it was dropped **twice, independently**, from the two requirements that could each have carried it.

**(f) WRITE-004 acceptance visibility deadline — a straight weakening.**
Authority `GOALS.md:761` — *"The accepted local materialization MUST be visible through the write-store query source **before `Accepted` is returned**."*
Planning `.planning/REQUIREMENTS.md:77` — *"**WRITE-04**: Matching queries expose the accepted local materialization directly from the write store **before relay acknowledgement**."*
"Before relay acknowledgement" is an arbitrarily later deadline than "before `Accepted` returns" — it permits a window in which the application holds a `Write` but the event is invisible to its own queries. The strict boundary was replaced with a loose one.

**Observable distinction (whole finding).** For each pair, an application can construct the conjunction and observe a violation that no `.planning` requirement forbids: two handles → two sockets (a); failed open → orphan session (b); double `next()` → data consumed (c); slow consumer burst → unbounded growth (d); publish → query gap before ack (f).

**Proposed falsifier.** One test per case; the highest-value one:
```rust
#[tokio::test]
async fn two_equivalent_live_handles_share_one_relay_session_and_one_req() {
    let t = CountingTransport::new();
    let fava = assembly_with_transport(t.clone());
    let q = Query::events().live().to(["wss://relay"]);
    let (a, b) = tokio::join!(fava.observe(q.clone()), fava.observe(q.clone()));
    assert_eq!(t.open_sessions(), 1, "QUERY-002: equivalent observations share relay work");
    assert_eq!(t.reqs_sent(), 1);
    drop(a);
    assert_eq!(t.closes(), 0, "closing one must not close work the other still needs");
}
```
Fails today on the first assertion.

**Confidence.** `confirmed`.

---

### req-coverage-holes — critical — behavioral proof

Spec requirements with **no counterpart of any kind** in the 129. Each was checked by keyword sweep across `REQUIREMENTS.md`, `ROADMAP.md`, and `PROJECT.md` (all three return zero for the discriminating terms shown).

| Spec requirement | Authority line | Discriminating term, hits in `.planning` |
|---|---|---|
| `QUERY-001` query language: reactive current-account input, values projected from another query, union/intersection/difference, independently configured nested queries | `GOALS.md:277-284` | `nested` 0, `derived` 0, `union` 0, `intersection` 0, `difference` 0, `current account` 0, `projected` 0 |
| `QUERY-006` derived-dependency shrink retracts records from the same open query | `GOALS.md:366` | `derived` 0 |
| `QUERY-007` nested queries retain independent routing/access/freshness/cache/evidence authority | `GOALS.md:372-381` | `nested` 0 |
| `QUERY-007A` derived references contribute permitted relay hints | `GOALS.md:387-389` | `hint` present only in `WRITE-14`, write-side |
| `QUERY-008` combined query = one deduplicated view with per-branch evidence; whole-query bound | `GOALS.md:395-399` | `branch` 0 |
| `QUERY-009` MUST NOT expose or imply global synced / complete / percentage / authoritative-empty | `GOALS.md:403-416` | present only as an **Out of Scope table row** (`REQUIREMENTS.md:216`), i.e. demoted from a testable MUST NOT to a non-feature note |
| `QUERY-016` app-authored `since`/`until`/limit are never widened by cache coverage | `GOALS.md:493-497` | `since` 0, `watermark` 0 |
| `EVENT-014` one admitted event is one atomic observable mutation; fault-inject each boundary | `GOALS.md:685-691` | `atomic` 0 |
| `RELAY-001` Fava contacts only justified relays; bystander relays receive no connection attempt | `GOALS.md:1033-1035` | `justified` 0, `bystander` 0 |
| `WRITE-009` sign without publishing (no intent, receipt, route, delivery) | `GOALS.md:825` | `without publishing` 0 |
| `WRITE-024` page through active/retained writes without loading all history; inspect by event id | `GOALS.md:974-979` | `page` 0, `paging` 0 |
| `WRITE-027` settled empty routing yields a typed no-destination outcome naming the reasons | `GOALS.md:999` | `no-destination` 0 |
| `WRITE-030` already-expired events refused before custody | `GOALS.md:1023` | `expired` 0 |
| `ID-002`…`ID-005`, `ID-007`, `ID-008` current-account reactivity, refusal before acceptance, raw-vs-bech32 identity shape, all-or-nothing session restore, NIP-44/NIP-04 separation, secret material never entering generic state | `GOALS.md:1189-1231` | `bech32` 0, `encrypt` 0, `NIP-44` 0, `secret` 0; `restore` matches only `READ-13` reconnect |
| `OPS-003` stalled writes visible under one classification | `GOALS.md:1410-1416` | `stalled` 0, `stuck` 0 |
| `OPS-005` shipped application-facing test infrastructure as product | `GOALS.md:1441-1455` | not represented; `SUB-02` covers conformance kits only |
| `PROTO-005`, `PROTO-007`, `PROTO-008`, `PROTO-009`, `PROTO-010` | `GOALS.md:1293,1328,1334,1340,1346` | `content pars` 0 |

`.planning/REQUIREMENTS.md:355-359` nonetheless asserts *"v1 requirements: 129 total / Mapped to phases: 129 ✓ / Unmapped: 0 / Duplicate mappings: 0"*. That coverage block measures the corpus against itself. It is a tautology, not a coverage check.

**Two additional defects in that same block, both falsifiable:**

- **Phantom requirement IDs.** `.planning/ROADMAP.md:243` gives Phase 07.1 `**Requirements**: R1, R2, R3, R4, R5, R6, R7, R8, R9`, and `07.1-VERIFICATION.md:11` records `requirements: [R1 … R9]` and grades all nine `VERIFIED`. `R1`…`R9` appear **nowhere** in `REQUIREMENTS.md`. Nine requirements were delivered, verified, and closed entirely outside the requirement registry.
- **"Duplicate mappings: 0" is false.** `REQUIREMENTS.md:280` maps `LOCAL-09 | Phase 1` and `:322` maps `ROUTE-10 | Phase 4`, while `ROADMAP.md:214` assigns `LOCAL-09, ROUTE-10` to Phase 06.1 and `06.1-VERIFICATION.md:151-154` grades both there. Two requirements have two owning phases and two independent `SATISFIED` verdicts.

**Observable distinction.** An application that composes a nested query, a derived pubkey set, or a combined branch query is exercising behavior the project has never required, never planned, and never tested — and `QUERY-001` makes that behavior mandatory for v1.

**Proposed falsifier.** The traceability gate from `req-no-traceability-to-spec-ids` covers this; additionally a red test proving the gap is real:
```rust
#[tokio::test]
async fn derived_dependency_shrink_retracts_only_its_records() {
    // QUERY-006: unfollowing one author removes only that author's records.
    // Fails to compile today: Query has no derived-value / projected-from-query axis at all.
}
```

**Confidence.** `confirmed`.

---

### req-ownership-ledger-unrepresented — critical — ownership

**Authority.**
`ARCHITECTURE.md:2993` — *"The ledger should remain a maintained architecture artifact. Adding mutable state requires naming its owner and consumers."*
`GOALS.md:186` — *"**Acceptance:** an ownership ledger can name exactly one owner for every stateful concept. Any duplicate owner is treated as an architecture defect."*
`AGENTS.md` gate 1 — *"Ownership — one authority for every mutable fact and lifecycle."*

**Implementation.** The ledger's 27 rows were checked one by one against all 129 planning requirements. **No requirement anywhere in `.planning` names the ownership ledger, and no requirement asserts the owner for the six rows that the confirmed deviation violates:**

| Ledger row | Authority | Owning requirement in `.planning` |
|---|---|---|
| Open live-query handle → `fava-observe` | `ARCHITECTURE.md:2972` | **none** |
| Current merged query snapshot → `fava-observe` | `:2973` | **none** |
| Reactive dependency node → `fava-observe` | `:2974` | **none** (nested/derived queries absent entirely) |
| Query demand for one relay → `fava-observe` | `:2978` | **none** (`ROUTE-09` describes the planner's *input*, never the demand's owner) |
| Wire subscription plan → `fava-observe` owns desired plan | `:2979` | **none** |
| Relay connection generation → selected `Transport` | `:2980` | **none** (`READ-13` states the behavior, names no owner) |
| Execution resources and joins → `fava-runtime` | `:2990` | **none** — and `ls crates/` confirms `fava-runtime` does not exist |
| NIP-42 challenge lifecycle → `fava-auth` | `:2982` | **none** — `fava-auth` does not exist; `HARD-01` names no owner |
| Public engine lifecycle → `fava` | `:2991` | **none** |

Approximately 11 of 27 rows have partial owner-bearing coverage (`LOCAL-02`, `LOCAL-03`, `WRITE-03/04/05/07`, `PROF-05/06`, `ROUTE-02`, and — added only on 2026-08-23 — `SESSION-07`). `SESSION-07` (`REQUIREMENTS.md:133`, *"`fava-session` exclusively owns mutable signer attachment…"*) is the **only** requirement in the entire corpus written in ledger form: named owner, exclusive, with a lock/transaction constraint. It is the template the other 26 rows need.

Worse, the architecture's own ownership check is deferred past every completed milestone. `ARCHITECTURE.md:3388` defines **Falsifier N — ownership audit**, and A–P define sixteen architectural falsifiers total. `.planning` collapses all sixteen into a single requirement — `REQUIREMENTS.md:169`, *"**SUB-08**: Every architecture falsifier passes…"* — assigned to **Phase 10**. Grepping the implementation plan, falsifiers A–P are referenced exactly twice (`:1440`, `:1441`, both M10). No milestone gate between M1 and M7 runs an ownership audit. That is precisely why a facade-owned relay lifecycle survived six "passed" verdicts.

**Observable distinction.** An application supplying an alternative `Transport` cannot obtain relay-session ownership, because `crates/fava/src/relay.rs` retains planner, cache, diagnostics, reconnect, and ingest-dispatch state that the ledger assigns elsewhere. No requirement makes that a failure.

**Proposed requirement IDs (the deliverable asked for).** Add an `OWN-*` family, each in `SESSION-07` form, mapped to the milestone that first creates the state — **not** to Phase 10:

| ID | Text | Phase |
|---|---|---|
| `OWN-01` | `fava-observe` exclusively owns observation identity, the open live-query handle, and the current merged snapshot; the facade orders construction and shutdown and retains no observation state. | Phase 1 (extend at Phase 2) |
| `OWN-02` | `fava-observe` exclusively owns retained logical query demand per relay session and the desired wire-subscription plan; the selected planner computes the plan and owns none of it. | Phase 2 |
| `OWN-03` | The selected `Transport` exclusively owns relay connection establishment, session generation, reconnect, backoff, and close; no other crate retains session state or a reconnect loop. | Phase 2 |
| `OWN-04` | Equivalent observations share one relay session, one wire subscription, and one refcounted work item; closing one observation releases only its reference. | Phase 3 |
| `OWN-05` | `fava-runtime` exclusively owns execution resources, task joins, cancellation propagation, and shutdown barriers; provider calls execute through it with operation and generation identity. | Phase 3 (hardened Phase 8) |
| `OWN-06` | `fava-ingest` exclusively owns wire attribution, id/signature verification, and admission ordering; no other crate may commit a cache mutation from relay bytes. | Phase 2 |
| `OWN-07` | `fava-auth` exclusively owns the NIP-42 challenge lifecycle per access context and session generation. | Phase 8 |
| `OWN-08` | Every row of `ARCHITECTURE.md` Part IX names exactly one existing owner, and an executable ownership audit (Falsifier N) runs at every milestone gate, not only at Phase 10. | Every phase |

**Proposed falsifier for `OWN-08`.**
```rust
#[test]
fn ownership_ledger_rows_all_name_an_existing_owner() {
    for row in parse_ledger("docs/spec/ARCHITECTURE.md") {   // Part IX table
        assert!(crate_exists(row.owner), "unowned ledger fact: {}", row.fact);
        assert!(requirement_exists_for(row.fact), "unmapped ledger fact: {}", row.fact);
    }
}
```
Fails today on `fava-runtime`, `fava-auth`, `fava-session`, and on ~16 unmapped facts.

**Confidence.** `confirmed`.

---

### req-verification-evidence-self-authored — critical — behavioral proof

**Authority.** `.planning/REQUIREMENTS.md:186` Definition of Done item 6 (project-binding): *"Independent wire, process, relay, storage, or native evidence exists wherever Fava cannot be its own witness."* `AGENTS.md` gate 6. `FAVA_REWRITE_IMPLEMENTATION_PLAN.md:283-287` (M0 exists precisely to supply independent witnesses).

**Implementation — full inventory of the 9 verification records.**

| Record | Verdict | Evidence cited | Evidence authored by the change it verifies? | Backfilled? | External scenarios rerun? |
|---|---|---|---|---|---|
| `01-VERIFICATION.md` | `passed` 12/12, "No M1 gaps remain" | `docs/issues/0001-local-source-merge.md`; commit `6be0fa5` | **Partially** — issue predates M1 (`74f5f94`) but was rewritten in `6be0fa5` itself | **Yes** — `da8db46`, 4h41m after M1 | No external scenario named |
| `02-VERIFICATION.md` | `passed` 10/10, "No M2 gaps remain" | `docs/issues/0004-explicit-live-query.md`; commit `7fac920` | **Yes** — the issue's only commit **is** `7fac920` | **Yes** | **No** — states verbatim: *"External scenarios were inspected, not rerun, during this reconciliation."* |
| `03-VERIFICATION.md` | `passed` 10/10, "No M3 gaps remain" | `docs/issues/0005-multi-relay-observation.md`; commit `1f2c0ed` | **Yes** — first and only commit is `1f2c0ed` | **Yes** | **No** — *"External scenarios were inspected, not rerun, here."* |
| `04-VERIFICATION.md` | `passed` 11/11, "No M4 gaps remain" | `docs/issues/0006-ordered-automatic-routing.md`; commit `9860711` | **Yes** — first commit is `9860711` | **Yes** | **No** — *"Preserved M4 run bundles report all four real-relay canaries passing."* |
| `05-VERIFICATION.md` | `passed` 11/11, "No M5 gaps remain" | `docs/issues/0007-durable-explicit-publication.md`; commit `7e5820f` | **Yes** — first commit is `7e5820f` | **Yes** | **No** — *"Preserved M5 bundles report…"* |
| `06-VERIFICATION.md` | `passed` 12/12, "No M6 gaps remain" | `docs/issues/0008-automatic-write-routing.md`; commit `309e421` | **Yes** — first commit is `309e421` | **Yes** | **No** — *"Preserved M6 bundles report…"* |
| `06.1-VERIFICATION.md` | `passed` 12/12, "No gaps found" | named tests + `docs/issues/0018` + *"Preserved `nostr-rs-relay 0.8.12` artifact: 300 rows"* | **Partly** — the deliberate-break record is in issue `0018`, authored by the phase | No (contemporaneous) | **Claims yes** — *"full Cargo/Clippy/fmt + canary + Bazel + vocabulary gate at `cb1b698`"*; the preserved 300-row artifact is **absent** |
| `07-VERIFICATION.md` | `passed` 12/12, "No blocking or warning gaps" | named tests; *"verifier reran all four CLIs"*; issue break records | Partly | No | **Yes** — the strongest record in the set |
| `07.1-VERIFICATION.md` | `passed` 9/9 (`R1`–`R9`) | `evidence.croissant_pair: apps/canary/runs/phase-07.1-pair.9EyxBY` | **Yes** — the pair was produced by the phase's own Plan 12 | No | Replayed **by the phase**, not by the verifier; and see below |
| `07.1.1` | **no VERIFICATION.md exists** | `07.1.1-VALIDATION.md` rows marked `passed Plan 12` | **Yes** — `07.1.1-12-SUMMARY.md` lists `07.1.1-VALIDATION.md` under `modified` | n/a | Pair exists on disk but is self-produced and git-ignored |

**Evidence that no longer exists.** `ls apps/canary/runs/` returns only six `phase-07.1.1-pair.*` directories. `apps/canary/runs/` is line 3 of `.gitignore`. Therefore:

- the "Preserved M2/M3/M4/M5/M6 run bundles" cited by five `passed` verdicts are **not in the repository and not on disk**;
- the 06.1 "preserved controlled-relay artifact with 300 rows and zero mismatches", cited as evidence for truths 9 and 10, is **absent**;
- `apps/canary/runs/phase-07.1-pair.9EyxBY`, the sole external witness for Phase 07.1's nine `VERIFIED` verdicts, **does not exist** (`ls` → No such file or directory). `HANDOFF.json:47` says it is *"a separate GitHub handoff release asset"*.

**Verdicts that rest on implementation-authored evidence and must be flagged (all of them, per the brief's instruction):**
`01`, `02`, `03`, `04`, `05`, `06` — fully; `06.1` and `07.1` — for their external-process truths; `07.1.1` — entirely, with no verification record at all. Only `07` survives with a genuine independent rerun claim.

**Observable distinction.** A fresh checkout cannot reproduce any M1–M6 or 06.1 or 07.1 external claim. `.planning/codebase/CONCERNS.md:541-548` records this as a Medium test-coverage gap — *"Risk: A failed or historical live claim cannot be reconstructed independently"* — and it was not treated as verdict-invalidating.

**Proposed falsifier.** Process gate: `tools/check_evidence_reachable.py` — for every `*-VERIFICATION.md`, resolve each cited artifact path and each cited `docs/issues/NNNN` commit; fail if the path is missing, if the path is git-ignored, or if the issue's first commit equals the implementation commit it verifies. Fails today for 8 of 9 records.

**Confidence.** `confirmed`.

---

### req-map-contradicts-verdicts-by-20-seconds — critical — behavioral proof

**Authority.** `.planning/REQUIREMENTS.md:189` Definition of Done item 9: *"The complete scoped validation set passes."* `AGENTS.md` gate 6.

**Implementation.** Two commits, twenty seconds apart, in the same repository, by the same process:

- `b184aae` **2026-08-21 08:44:48** — "docs: refresh codebase map through M6" — writes `.planning/codebase/CONCERNS.md`, which records **eight Known Bugs** and **five High-priority Test Coverage Gaps** in shipped M1/M3/M5 behavior.
- `da8db46` **2026-08-21 08:45:08** — "docs: reconcile planning state through M6" — writes `01`–`06-VERIFICATION.md`, every one `status: passed`, every one ending "No M1/M2/M3/M4/M5/M6 gaps remain."

Direct contradictions, quoted from both sides:

| `CONCERNS.md` (08:44:48) | `*-VERIFICATION.md` (08:45:08) |
|---|---|
| `:112` *"**Authorized deletion does not retract a matching local write** … `StandardQueryEvaluator` still emits the same or another matching event from `WriteStore` because it performs no deletion-tombstone pass across sources."* | `01-VERIFICATION.md` — *"LOCAL-07 ✓ SATISFIED — deletion and expiry revise the same open observation"* |
| `:123` *"**Future expiration does not retract automatically** … Events accepted before their expiration remain in current queries after the timestamp passes."* `:29` *"**Expiry has no lifecycle owner**"* | same row, same verdict |
| `:44` *"**The event-cache mutation contract exposes an admission bypass** … A consuming application or provider can create query-visible signed state with fabricated relay evidence."* | `01-VERIFICATION.md` — *"LOCAL-02 ✓ SATISFIED — verified-only memory cache admission"*; `02-VERIFICATION.md` — *"READ-05 ✓ SATISFIED"* |
| `:174` *"**Configured WebSocket inbound frame bound is not enforced** … `next_message` returns arbitrarily larger text messages."* (confirmed: `crates/fava-transport-websocket/src/lib.rs:110` bounds outbound only) | `02-VERIFICATION.md` — *"READ-03 ✓ SATISFIED — bounded NIP-01 wire and WebSocket transport corpora"* |
| `:183` *"**A slow first relay blocks later known relays** … One slow DNS/TCP/TLS open delays every later relay and can prevent the initial observation handle from returning."* | `03-VERIFICATION.md` — *"No M3 gaps remain"*; `04-VERIFICATION.md` — *"ROUTE-03 ✓ SATISFIED — delayed router cannot block already-known relay work"* |
| `:139` *"**Duplicate local acceptance terminates query evaluation** … an already-open observation then closes without the cause."* | `05-VERIFICATION.md` — *"No M5 gaps remain"* |
| `:497-533` five **High**-priority coverage gaps across M1/M3/M5 | six consecutive "No gaps remain" |

The map's `Missing Critical Features` section (`:446-448`) explicitly frames its scope as *"These are specified M7-M11 scopes, **not defects in the completed M0-M6 slices**"* — so the map author declined to treat M0–M6 defects as milestone-invalidating, and the reconciliation twenty seconds later ratified that.

**Observable distinction.** `CONCERNS.md:183` describes, in the project's own words, the exact `QUERY-004` violation that this audit was convened over. It was written into `.planning` and then overruled by a verdict written twenty seconds later.

**Proposed falsifier.** Process gate: block a `status: passed` verification for phase *N* while `.planning/codebase/CONCERNS.md` contains any Known Bug or High-priority coverage gap whose `Files:` list intersects the crates that phase owns. Fails today for phases 1, 2, 3, 5.

**Confidence.** `confirmed`.

---

### req-read20-unfalsifiable — major — boundedness

**Authority.** `FAVA_REWRITE_IMPLEMENTATION_PLAN.md:478` M3 exit gate — *"At least 1,000 simultaneous idle observations remain bounded under a declared profile."* `GOALS.md:436` — *"Observation memory MUST remain bounded even when an application is slow."*

**Implementation.** `.planning/REQUIREMENTS.md:56` — *"**READ-20**: The **declared standard profile** keeps at least 1,000 simultaneous idle observations within explicit **task, memory, descriptor, and queue** bounds."* Marked `[x]` Complete.

Its sole evidence, `03-VERIFICATION.md` — *"READ-20 ✓ SATISFIED — 1,000 idle observations remain on one current-thread runtime"* — resolves to `crates/fava/tests/observation_bounds.rs:27`, `one_thousand_idle_observations_share_the_current_runtime_thread`. Reading it:

- `:29` `let thread = std::thread::current().id();` and `:34,:46` `assert_eq!(std::thread::current().id(), thread)` — the test measures **thread identity only**. It asserts nothing about memory, descriptors, or queues. Three of the four named bounds are unmeasured.
- `:32` `fava.observe(Query::events().cache_only())` — all 1,000 observations are **cache-only**. Zero relay sessions, zero subscriptions, zero descriptors. The M3 exit gate for the milestone titled *"Multi-Relay Reactivity and Bounded Observation"* is met by a test with no relay in it.
- The requirement's subject — *"The declared standard profile"* — **does not exist**. `ls crates/` confirms `fava-standard` is absent; `docs/internals/vocabulary.toml:270` lists it under `spec_crates` (specified, unimplemented). A requirement whose subject does not exist cannot be satisfied.

This is the same failure shape as LOCAL-08/READ-02: a property was verified in the local-only regime and never reapplied once networking existed.

**Observable distinction.** 1,000 live observations against one relay open, per `crates/fava/src/live.rs:33-53`, 1,000 `RelaySessionKey`s and 1,000 `OpenedRelay` tasks with 1,000 sockets — well past ordinary descriptor limits — and nothing in the corpus forbids it.

**Proposed falsifier.**
```rust
#[tokio::test(flavor = "current_thread")]
async fn one_thousand_idle_live_observations_share_bounded_sessions_and_descriptors() {
    let t = CountingTransport::new();
    let fava = assembly_with_transport(t.clone());
    let q = Query::events().live().to(["wss://relay"]);
    let obs: Vec<_> = futures::future::try_join_all((0..1_000).map(|_| fava.observe(q.clone())))
        .await.expect("1000 live observations open");
    assert_eq!(t.open_sessions(), 1, "declared profile bounds relay sessions");
    assert!(t.open_descriptors() <= DECLARED_DESCRIPTOR_BOUND);
    drop(obs);
}
```
Fails today: 1,000 sessions.

**Confidence.** `confirmed`.

---

### req-codebase-map-two-owners — critical — ownership

**Authority.**
`ARCHITECTURE.md:2972,2978,2979` — `fava-observe` owns the live-query handle, per-relay query demand, and the desired wire subscription plan.
`ARCHITECTURE.md:2980` — the selected `Transport` owns relay connection generation.
`GOALS.md:186` — *"Any duplicate owner is treated as an architecture defect."*

**Implementation — the planning artifacts assert both owners, in three files, and never reconcile them.**

*Owner A — `fava` facade owns relay work:*
- `.planning/codebase/ARCHITECTURE.md:102` — *"| `fava` | Exposes the facade, validates assembly, **owns live-query relay tasks**, and delegates publication lifecycle. |"*
- `.planning/codebase/ARCHITECTURE.md:185-189` — *"**Facade and Relay Coordination Layer:** … Contains: `Fava`, `FavaBuilder`, explicit/automatic query opening, **reconnect loops, route reconciliation**, `QuerySource for Fava`, and publication delegation."*
- `.planning/codebase/ARCHITECTURE.md:87` — *"| `fava-observe` | **Atomically opens local sources**, reevaluates current state, coalesces bounded snapshots, and owns close. |"* — scoped to local only.

*Owner B — `fava-observe` owns relay work:*
- `.planning/research/ARCHITECTURE.md:81` — *"| `fava-observe` | One observation lifecycle, source coherence, current projection, **route demand**, bounded app delivery, teardown | query sources, routing, planner, transport, facade | **M1 local; M2-M4 relay work** |"*
- `.planning/research/ARCHITECTURE.md:92` — *"| `fava` / `fava-standard` | **Thin public commands** and explicit assembly |"*
- `.planning/research/ARCHITECTURE.md:128` — the required open sequence: *"install observation owner, **then** expose handle/current value"*
- `.planning/research/SUMMARY.md:101` — *"**Observation owner** — coherent opening, merged current view, bounded delivery, **route demand**, cancellation, and teardown."*
- `.planning/codebase/STRUCTURE.md:27` — *"`fava-observe/` # **Query observation owner**"*

`.planning/research/*` was committed on 2026-08-20, **before M2**, and states the authoritative model correctly. `.planning/codebase/*` was refreshed on 2026-08-21 after M6 and **normalized the deviation into a named architectural layer** — "Facade and Relay Coordination Layer" — with a stated Purpose, Location, Contains, Depends on, and Used by. It did not flag it. `.planning/codebase/CONCERNS.md` contains **no ownership section at all**: grepping `owner` there returns only deletion-owner, expiry-owner, and module-size entries. The one entry that touches the deviation (`:183`, slow-first-relay) classifies it as a performance/fragility bug, never as an ownership violation.

Both documents remain in `.planning/` today, unreconciled, with no ADR, no decision entry in `PROJECT.md`'s Key Decisions table, and no concern raised.

**Observable distinction.** A contributor reading `.planning/codebase/` builds relay features into `crates/fava/`; a contributor reading `.planning/research/` builds them into `crates/fava-observe/`. The repository shows the first happened at M2 and was then documented as the architecture.

**Proposed falsifier.**
```rust
#[test]
fn facade_retains_no_relay_session_or_subscription_state() {
    let src = include_str!("../src/lib.rs");
    for forbidden in ["subscription_planner", "transport", "next_subscription", "OpenedRelay"] {
        assert!(!src.contains(forbidden), "fava facade must not own {forbidden}");
    }
}
```
Fails today: all four are `Fava` fields / facade types.

**Confidence.** `confirmed`.

---

### req-milestone-exit-gates — critical — behavioral proof

Every documented exit gate from `FAVA_REWRITE_IMPLEMENTATION_PLAN.md` for M1–M6, with a verdict I can defend from evidence available today.

**M1 (plan `:347-352`)** — verdict **met, but insufficient**

| Gate | Met? | Evidence |
|---|---|---|
| No relay/transport/runtime networking dependency in these crates | **Yes** | Read all 8 `Cargo.toml`s: only `fava-state/query/write/routing`, `nostr`, `serde`, `thiserror`, `tokio` |
| Same semantic corpus runs against memory cache and memory write store | **Yes** | `crates/fava-query-standard/tests/source_merge.rs`, `crates/fava/tests/local_source_merge.rs` |
| Canary uses only the public facade for local queries and writes | **Yes** | `apps/canary/src/local.rs` |
| Cache/write-store source data inspectable through public event records without exposing storage internals | **Yes** | public `EventRecord` evidence paths |

The gates hold. The problem is that they are *all* local, and LOCAL-08 was parked behind them. **M1's verdict is revocable not on its gates but on LOCAL-07**, which `CONCERNS.md:112,123` proves fails for cross-source deletion and for due-time expiry.

**M2 (plan `:412-417`)** — verdict **not met**

| Gate | Met? | Evidence |
|---|---|---|
| One real relay path works without any automatic router | **Yes** | `ROUTE-06`, explicit path |
| Fava public diagnostics and the independent proxy agree on relay/session/subscription identity | **Unverifiable today** | the M2 run bundles asserting this are absent from disk and git-ignored |
| No Fava internal types appear in the canary | Probably yes | not re-checked in depth |
| Transport conformance kit includes handoff success, refusal, disconnect, and close | **Partially** | `crates/fava-transport-websocket/tests/conformance.rs` has `complete_text_frame_handoff_succeeds:26`, `remote_disconnect_is_reported_exactly:66`, `close_is_idempotent_and_refuses_later_handoff:83`. **No standalone refusal case**, and `CONCERNS.md:174` proves the inbound frame bound the kit is supposed to cover is unenforced |

Additionally, the plan's own M2 slice list (`:373`) names *"relay-facing portion of `fava-observe`"*. `crates/fava-observe/src/lib.rs` contains none of it. The slice was never built; the gate set never asked.

**M3 (plan `:475-479`)** — verdict **not met**

| Gate | Met? | Evidence |
|---|---|---|
| Observation resource usage independent of one-thread-per-query design | **Yes, trivially** | `observation_bounds.rs:27`, cache-only |
| At least 1,000 simultaneous idle observations remain bounded under a declared profile | **No** | see `req-read20-unfalsifiable`: cache-only, thread-only assertion, and **no declared profile exists** — `fava-standard` is absent from `crates/` |
| Multi-relay scenario passes against two independent relay implementations by M8 | Deferred to M8, correctly | `03-VERIFICATION.md` says so |

**M4 (plan `:547-552`)** — verdict **met on its own terms**

| Gate | Met? | Evidence |
|---|---|---|
| `fava-routing` contains no NIP-65/hint/app-relay/fallback meaning | **Yes** | `crates/fava-routing/src/chain.rs:445-462` is an executable negative test. (Narrow: it scans `lib.rs` and `Cargo.toml` only, not `chain.rs`.) `Cargo.toml` deps confirm no router implementation edge |
| Each higher-level routing policy is a separate crate | **Yes** | four `crates/fava-router-*` |
| A router outside the workspace by M10 | Deferred, correct | |
| Planner substitution does not require router or observation changes | **Cannot be true as built** | the desired plan is computed inside `crates/fava/src/relay.rs`, which the ledger (`ARCHITECTURE.md:2979`) assigns to `fava-observe`; substituting a planner therefore touches the facade |

M4's verdict is **downgradeable, not revocable** — three of four gates genuinely hold.

**M5 (plan `:630-635`)** — verdict **not met**

| Gate | Met? | Evidence |
|---|---|---|
| Standard write-store profile has process-kill tests at **every** commit/effect boundary | **No** | `crates/fava-write-store-redb/tests/process_kill.rs` exists, but `CONCERNS.md:151` — *"**Redb terminal eviction can diverge memory from durable state** … it exists before restart and disappears after reopen"* — and `:519` lists "Redb retention eviction … restart parity after eviction" as an untested **High**-priority gap. A boundary with a known divergence bug is a boundary without a kill test |
| `fava-publication` owns the write lifecycle but not router/signer/publisher/transport/delivery-policy state | Plausible | not deeply re-checked in this area |
| Canary sees no internal attempt/lane types | Probably yes | |
| Memory write store not presented as the standard durable profile | **Yes** | |

Independently, `WRITE-04` is a weakened restatement of `GOALS.md:761` (see `req-lost-conjunctions-catalogue` (f)), and `CONCERNS.md:139` proves duplicate local acceptance closes an open observation — an M5 acceptance path defect.

**M6 (plan `:704-708`)** — verdict **not met**

| Gate | Met? | Evidence |
|---|---|---|
| No central routing crate names or depends on router implementations | **Yes** | as M4 |
| Router contribution count and route fan-out are bounded with exact shortfall | **Yes** | `chain.rs:430,436` — typed refusals at 256 destinations / 32 routers, with exact numbers in the message |
| Async recipient scenario passes through real relay processes and **independent wire transcripts** | **Unverifiable today** | `06-VERIFICATION.md` cites "Preserved M6 bundles"; `ls apps/canary/runs/` shows they do not exist |

**Summary of exit-gate verdicts:** M1 **revoke** (LOCAL-07 falsified by the project's own map); M2 **revoke** (two gates unmet/unverifiable, and its named `fava-observe` slice was never built); M3 **revoke** (the 1,000-observation gate is met by a cache-only thread-identity assertion against a profile that does not exist); M4 **downgrade**; M5 **revoke** (kill-test gate unmet, with a known durable-divergence bug); M6 **revoke** (independent wire transcript gate unverifiable).

**Observable distinction / falsifier.** Per row above.

**Confidence.** `confirmed` for M1, M3, M5, M6; `confirmed` for M2 modulo the "no internal types" gate which I did not re-check exhaustively.

---

### req-0711-complete-without-verification — critical — behavioral proof

**Authority.** `.planning/ROADMAP.md:16` — *"A phase is not complete until every mapped requirement, every complete exit gate … and every item in `.planning/REQUIREMENTS.md`'s Definition of Done passes."* Definition of Done item 3: *"The owning component proof passes through public contracts."*

**Implementation.** Phase 07.1.1 owns `GROUP-01` … `GROUP-12`, all marked `[x]` Complete (`REQUIREMENTS.md:110-121`, `:341-352`) and `Complete | 2026-08-22` in `ROADMAP.md:348`. Yet:

1. **No `07.1.1-VERIFICATION.md` exists.** `ls .planning/phases/07.1.1-*/` returns 12 PLANs, 12 SUMMARYs, CONTEXT, PATTERNS, RESEARCH, REVIEW, three REVIEW-FIX files, VALIDATION, COVERAGE, PAIR-ROOT, deferred-items — and no verification record. Every other completed GSD phase (06.1, 07, 07.1) has one.
2. **The only requirement-level verdict is self-reported.** `07.1.1-VALIDATION.md` marks all 12 GROUP rows `passed Plan 12`; `07.1.1-12-SUMMARY.md` lists `07.1.1-VALIDATION.md` in its `modified` key-files. The executing plan wrote its own pass marks.
3. **`COVERAGE.md` is one sentence** — *"No external API integration: Phase 07.1.1 implements a pure Nostr protocol capability…"* — for a phase whose `GROUP-12` requires *"a controlled two-relay public canary."*
4. **84 commits landed after the completion mark.** `788a0e9` (2026-08-22 16:51:18) set `Complete` in `ROADMAP.md`. Between then and now: 84 commits, including substantive behavioral fixes to the very requirements declared complete — `d11a928 fix(07.1.1): reject caller-owned h axes` (GROUP-07), `626763f fix(07.1.1): verify signed group payloads` (GROUP-08), `b71987c fix(07.1.1): WR-01 refuse duplicate saved hosts` (GROUP-10), `3660f8e fix(07.1.1): CR-02 cover all event identities` (GROUP-04), plus ~20 `fix(0711): CR-0N …` evidence-integrity commits. No re-verification followed.
5. **`HANDOFF.json` still describes the phase as unfinished.** `HANDOFF.json:9` `"status": "paused"`; `:15-18` lists tasks 2, 3, 4 (*"Create VALIDATION.md and checked executable plans for GROUP-01 through GROUP-12"*, *"Execute, review, verify, and complete Phase 07.1.1"*) as `not_started`. `PROJECT.md:26` likewise still lists `fava-simple-groups` as an unchecked Active requirement.

**Observable distinction.** Twelve requirements are marked Complete in the project's requirement registry on the strength of a self-marked validation table, contradicted by the project's own handoff record and by 84 subsequent fix commits.

**Proposed falsifier.** Process gate: refuse to write `Complete` into `ROADMAP.md`/`REQUIREMENTS.md` for a phase lacking `<phase>-VERIFICATION.md` with `status: passed`, and invalidate the verdict when any commit touching that phase's owned crates lands after the verification timestamp. Fails today for 07.1.1 (no record) and for 07.1 (84 later commits, several `fix(0711): …`).

**Confidence.** `confirmed`.

---

### req-state-artifacts-inconsistent — minor — cohesion

**Authority.** `.planning/ROADMAP.md:16` binding completion contract.

**Implementation.** Mutually inconsistent status across the four top-level planning records:

- `STATE.md:12-14` — `completed_phases: 9`, `total_phases: 15`, `percent: 60`; `:31` — *"Progress: 71%"*; `:33` — *"Phase progress is 10/14."* Three different denominators and two different percentages in one file.
- `STATE.md:203` — *"Phase 06.1 awaits final goal verification; implementation and code review are complete"* — while `06.1-VERIFICATION.md` has existed since 2026-08-21 with `status: passed`, and `ROADMAP.md:344` marks it Complete.
- `ROADMAP.md:337-352` progress table **omits Phase 07.2 entirely**, though the phase exists, has a SPEC, a CONTEXT, and a plan.
- `PROJECT.md:26` still lists `fava-simple-groups` as an unchecked Active requirement; `PROJECT.md:60` — *"Phase 07.1 is active"* — is two phases stale. `PROJECT.md`'s Key Decisions table has no entry for the runtime-signer-lifecycle architecture amendment (`b67d54e`).
- `HANDOFF.json` is 1.5 days and one whole phase stale (see previous finding).

**Observable distinction.** No single artifact can be trusted to answer "what is done"; a reader must cross-reference five and pick.

**Proposed falsifier.** `tools/check_planning_consistency.py`: assert `STATE.progress.completed_phases` equals the count of `[x]` phases in `ROADMAP.md`, that every ROADMAP phase has a progress-table row, that `HANDOFF.json.phase` equals `STATE.current_phase`, and that `PROJECT.md` Active items have no corresponding `Complete` ROADMAP row.

**Confidence.** `confirmed`.

---

## Conforming (verified, not merely unexamined)

Each of these was actually checked by reading the file or running the command named.

- **M1 dependency isolation.** I read all eight M1 crate `Cargo.toml` files. None depends on `fava-transport`, `fava-wire`, `fava-subscriptions`, or any networking crate. The M1 exit gate at `FAVA_REWRITE_IMPLEMENTATION_PLAN.md:349` genuinely holds.
- **Routing-core policy isolation (M4/M6 gate 1).** `crates/fava-routing/Cargo.toml` depends only on `fava-query`, `fava-state`, `fava-write`, `thiserror`, `tokio` — no router implementation. `crates/fava-routing/src/chain.rs:445-462` is a real executable negative test over eight forbidden tokens. Conforming; my only note is that it scans `lib.rs` and `Cargo.toml`, not `chain.rs`.
- **Route fan-out bounds (M6 gate 2).** `chain.rs:428-442` produces typed refusals with exact numbers (`"route destinations exceed bound: 257 > 256"`, `"configured routers exceed bound: 33 > 32"`). `ROUTE-11` and `WRITE-23` are honestly evidenced.
- **`SESSION-01`…`SESSION-07` are correctly formed.** These seven, added 2026-08-23, are the only requirements in the corpus written against a named architectural owner with lifecycle and lock constraints (`REQUIREMENTS.md:133`). They also correctly close the previously unmapped `GOALS.md:815` (`WRITE-008`, missing-signer parking). They are the template the rest of the corpus needs, and they were authored *before* their implementation — the only family in the file for which that is true.
- **`.planning/research/` is tracked, not scratch.** I checked `git ls-files .planning/research/` on the suspicion that the `.gitignore:17` entry made the correct owner model untracked. It does not — all five files are tracked. The correct model was durably committed on 2026-08-20 and simply not enforced.
- **`.planning/codebase/CONCERNS.md` is substantively honest.** It is the highest-integrity artifact in `.planning/`. It records eight real bugs with triggers, files, and workarounds, and five High-priority coverage gaps. Its failure is scope (`:446-448` explicitly declines to treat M0–M6 defects as milestone-relevant) and the fact that it was overruled twenty seconds later — not accuracy.
- **`07-VERIFICATION.md` is the strongest verification record.** It distinguishes `implementation_head` (`f97ecd8`) from `verified_head` (`1dd7e5e`), states that the verifier *reran* all four CLI canaries rather than citing preserved bundles, and explicitly resolves a PLAN-vs-authority conflict in favour of the authority (*"Older PLAN wording … was superseded by the current GOALS/ARCHITECTURE contract; verification follows the authority order in `AGENTS.md`"*). Its evidence is contemporaneous and partly independent.
- **`06.1-VERIFICATION.md` is methodologically strong where its artifacts survive.** Twelve observable truths, a test-quality audit with skipped/circular counts, a post-review fix table, and an explicit **Disconfirmation Pass** (*"The 52-key identity test alone would not prove matching. That potential misleading pass is closed by…"*). This is the only record in the corpus that argues against itself. Only its external 300-row artifact is missing.
- **`docs/issues/0001` predates M1.** Unlike issues 0004–0008, `0001-local-source-merge.md` was first committed at `74f5f94` (2026-08-20), before the M1 implementation. Its later rewrite in `6be0fa5` weakens but does not fully void its independence, which is why I graded M1's evidence "Partially" rather than "Yes".
- **The five explicitly deferred product decisions are handled correctly.** `REQUIREMENTS.md:200-207`, `ROADMAP.md:18`, and `STATE.md` consistently keep `OPEN-001`…`OPEN-005` unpromised and name their owning phases. `06-VERIFICATION.md` correctly refuses to convert them into gaps. This is the process working as designed.
- **`.planning/debug/observe-ownership-collapse.md` independently reached the same M2-origin conclusion** by a different route (git ancestry) and is labelled `[REPOSITORY-PROVED]` throughout. I re-derived its two load-bearing chronology claims from `git log` rather than accepting them. Both hold.

---

## Open questions

1. **Does a `docs/issues/000N` record predating its implementation commit exist for any of M2–M6?** I checked the first commit of each and found identity with the implementation commit in all five cases. If a pre-implementation red record exists elsewhere (a branch, a stash, a deleted path), M2–M6 evidence independence is partially recoverable. I found none.
2. **Where is `apps/canary/runs/phase-07.1-pair.9EyxBY`?** `HANDOFF.json:47` calls it *"a separate GitHub handoff release asset."* The repo has `origin https://github.com/pablof7z/fava.git`. If that asset is retrievable and its manifest hashes match `07.1-VERIFICATION.md`'s recorded `croissant_executable_sha256`, Phase 07.1's external claim is recoverable; otherwise nine `VERIFIED` verdicts have no surviving witness.
3. **Were the M2–M6 run bundles ever committed and later removed, or never committed?** `.gitignore:3` has excluded `apps/canary/runs/` for the whole history I sampled, which suggests never. If so, the M0 principle at `FAVA_REWRITE_IMPLEMENTATION_PLAN.md:283` — that evidence be *reconstructable* — was structurally unmet from M2 onward, and `.planning/codebase/CONCERNS.md:541` recorded it as Medium rather than blocking.
4. **Should `LOCAL-12` exist as a requirement at all?** *"The same semantic corpus passes through memory event-cache and write-store providers … without relay, transport, or runtime networking dependencies"* is verbatim an M1 exit gate promoted into the behavioral requirement set. Requirements that restate build constraints inflate the coverage count without adding falsifiable behavior. Same question for the `Coverage: 129/129 ✓` block, which measures the corpus against itself.
5. **Who owns re-verifying the 66 M1–M6 requirements after they are rewritten against spec IDs?** Rewriting them without rerunning owner-level and public negative proofs would repeat the exact failure this audit documents — a corpus updated to match code rather than code proved against a corpus.
6. **Does `.planning/codebase/` need to be regenerated or retired?** Its "Facade and Relay Coordination Layer" is an accurate description of the code and a false description of the architecture. A codebase map that normalizes deviations is worse than no map; it needs an explicit "deviates from `ARCHITECTURE.md:NNNN`" column.
