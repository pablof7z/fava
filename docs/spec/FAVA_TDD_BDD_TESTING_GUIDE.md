# Fava Testing Strategy: TDD, BDD, and Evidence

**Status:** proposed testing authority for the Fava rewrite
**Behavioral authority:** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`
**Architectural authority:** `ARCHITECTURE.md`
**Delivery plan:** `FAVA_REWRITE_IMPLEMENTATION_PLAN.md`

## 1. Purpose

This document defines how Fava behavior is specified, implemented, and proved.

Fava uses both TDD and BDD, but they serve different purposes:

- **BDD preserves durable product meaning.** It records the app-visible distinctions that must survive rewrites.
- **TDD drives implementation.** It starts with a failing executable proof at the smallest owner responsible for the behavior, then implements the smallest change that makes it pass.

A feature file is not automatically an executable test. A test is not automatically a product specification. The canary is not the first or only proof. Each artifact has one job.

## 2. One home for each kind of truth

| Concern | Authoritative home |
|---|---|
| What Fava must do | `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` |
| App-readable examples and contrasts | behavior feature files |
| Why responsibilities and crates are split | `ARCHITECTURE.md` |
| What work remains | GitHub issues |
| Proof that behavior works | executable tests at the owning layer |
| Proof that an ordinary app can use it | Rust canary and platform capstones |

Do not copy the same rule into several places. Link instead.

## 3. The required development loop

Every behavior change follows this sequence.

### 3.1 Name the behavior and its owner

Before editing implementation, identify:

1. the requirement or behavior being changed;
2. the app-visible distinction, if any;
3. the single component that owns the decision or lifecycle; and
4. the smallest executable witness that could prove it.

If ownership cannot be named, the design is not ready to implement.

### 3.2 Update behavior text first when meaning changes

When the observable contract changes, correct the owning feature and `Rule` before implementation.

The scenario must show:

- the context that matters;
- the action or event;
- the observable result;
- the nearby situation that must produce a different result; and
- the tempting wrong interpretation being excluded.

Correct wrong text in place. Do not retain the superseded claim in an appendix, duplicate feature, or compatibility scenario.

### 3.3 Write the smallest failing executable proof

Write the test at the smallest stable owner that can prove the behavior.

Run it before implementation and confirm:

- it fails;
- it fails for the intended reason;
- the setup reached the relevant precondition; and
- it would not pass because the fixture inserted the expected answer.

A test that merely fails to compile because everything is absent is useful only until the first real behavior boundary can be exercised. Replace scaffolding failures with causal behavioral failures as soon as the contract exists.

### 3.4 Make it pass with the smallest complete change

Implement only the responsibility needed to satisfy the behavior. Do not add speculative provider hooks, state variants, recovery paths, or configuration merely because they might be useful later.

Run the focused test continuously. Run the changed crate's tests before widening scope.

### 3.5 Refactor while green

Once behavior passes:

- remove duplicated decisions;
- make the single owner obvious;
- narrow interfaces;
- move tests to the stable owner when the first location was provisional; and
- preserve the exact observable result.

Refactoring is not complete while tests depend on private representation that the refactor is supposed to free.

### 3.6 Break the mechanism deliberately

Every newly claimed invariant needs a mechanism-disable check.

Examples:

- accept before write-store commit;
- block publication until all routers settle;
- omit a router's later contribution;
- admit an invalid signature;
- apply a stale signer completion;
- silently drop subscription-planner overflow;
- map ambiguous handoff to ordinary failure;
- merge NIP-05 and NIP-11 freshness rules;
- remove one SDK operation.

The linked evidence must fail for the claimed reason. A scenario that remains green after its protection is removed does not prove the invariant.

The mutation may be a local patch, a test-only seam owned by the responsible crate, or a proxy/lab mutation. It must not become an application-facing production knob.

### 3.7 Add the public capstone only when it proves something additional

Add or enable a public-API/system/canary scenario when the behavior crosses boundaries or makes a promise to ordinary applications.

The capstone proves that the pieces compose. It does not replace owner, property, model, crash, or protocol tests.

## 4. BDD: what belongs in behavior features

BDD is for durable app-visible contracts and multi-step lifecycle guarantees.

Good subjects include:

- query continuity and retraction;
- routing that makes partial progress while knowledge is unresolved;
- explicit routing bypassing routers;
- accepted writes surviving restart;
- per-relay truth rather than global completeness;
- cancellation races visible to callers;
- source/access/account isolation;
- ambiguous delivery outcomes;
- protocol-crate behavior that an app calls directly; and
- guarantees that something must never happen.

Do not use feature files for:

- local parser or codec matrices;
- private enum transitions;
- table layouts;
- exhaustive input combinations;
- every property of an index;
- benchmarks;
- crate-by-crate implementation plans; or
- test inventory.

If the implementation can change completely without an application noticing, prove it with an owner, property, model, differential, or headless test instead.

## 5. Behavior-file shape

Organize features by user behavior, never by crate.

Use stable behavior IDs and explicit status. Avoid ambiguous `@wip` or untagged prose that looks built when it is not.

Recommended header:

```gherkin
# id: ROUTING-ASYNC-001
# requirement: ROUTER-004
# status: built
# evidence:
#   - crates/fava-routing/tests/partial_progress.rs
#   - crates/fava/tests/automatic_route_expansion.rs
# canary: async-route-expansion

Feature: Automatic routing can make progress before all knowledge settles

  Rule: Known destinations are useful immediately

    Scenario: Two known recipients deliver while a third is still resolving
      Given Alice publishes one event tagging Bob, Carol, and Dave
      And Bob and Carol already have known inbox relays
      And Dave's relay discovery is still unresolved
      When Fava accepts the write
      Then delivery begins to Bob and Carol's relays immediately
      And the same receipt remains open for Dave
      When Dave's relay becomes known
      Then that relay is added to the same receipt
```

Allowed statuses:

- **specified** — normative behavior exists but executable proof is not complete; link the owning issue;
- **built** — named executable evidence exists and has been run;
- **known-violation** — current behavior contradicts the specification; link the owning issue.

A runner is optional. The feature remains the readable behavior memory; executable evidence may live in owner tests, model tests, public API tests, the deterministic relay lab, or the canary.

## 6. Scenario-writing rules

Each scenario should contain one product promise.

Use application and Nostr terms:

- event;
- relay;
- query;
- receipt;
- signer;
- author;
- route;
- source evidence;
- EOSE;
- cancellation.

Do not name private reducers, tables, channels, helper methods, crate-internal IDs, or database rows.

### 6.1 Show a contrast

The strongest scenarios distinguish two nearby cases that a broken implementation might collapse:

- unknown route versus settled no route;
- relay selected versus relay actually contacted;
- event returned versus EOSE received;
- pre-handoff cancellation versus post-handoff uncertainty;
- app relay always included versus fallback used only when coverage is insufficient;
- cached event versus unpublished local materialization;
- current request completion versus stale previous-generation completion.

### 6.2 Set up causes, not conclusions

Seed the protocol fact where discovery begins. Do not insert the route that routing is supposed to discover.

Create a write through the public acceptance operation. Do not hand-write write-store state.

Have a relay send an event and EOSE. Do not set internal coverage flags.

Ask: **is this fixture input a cause, or is it the result under proof?**

### 6.3 Keep examples minimal

Use the smallest values that expose the distinction. Exhaustive combinations belong in property/model tests.

## 7. Test layers and placement

Put proof in the smallest stable component responsible for it.

| Layer | Best for |
|---|---|
| Pure/unit/table | one value, parser, codec, deterministic transition |
| Property/model/differential | algebra, many operation orders, invariants across broad input space |
| Provider conformance | public provider contract, lifecycle, cancellation, advertised guarantees |
| Owner integration | one owner with its real collaborators and persistence boundary |
| Headless cross-owner | ordering, generations, committed-fact propagation, retries, cancellation, deadlines |
| Scripted relay | malformed frames, exact wire races, stale responses, auth timing, limits |
| Public Rust API | complete deterministic query/write promise through the supported facade |
| Real local relay lab | interoperability, real sockets/processes, relay persistence, NIP-42/NIP-11 behavior |
| Rust canary | ordinary app usability, complete flows, restart/crash/resource behavior |
| Native capstone | SDK cancellation, process lifecycle, packaging, parity |
| Public-relay mode | reconnaissance only; never the sole correctness oracle |

Do not duplicate the same path at every layer. Two tests are justified only when each proves something different.

## 8. Rewrite-specific placement guidance

### `fava-state`

Use property/model tests for deduplication, replacement, deletion, expiration, provenance merging, and order-independence where required.

### `fava-query`

Use algebraic and differential tests for query identity, union/intersection/difference, source merging, retraction, ordering, and bounded latest-state delivery.

### `fava-event-cache`

Use conformance tests for the baseline cache contract. Each implementation adds tests for the guarantees it advertises: volatility, persistence, provenance retention, eviction, expiry recovery, or coverage.

### `fava-write-store`

Use crash/reopen and state-machine tests for acceptance, receipt identity, current materialization, cancellation, rematerialization, lanes, outcomes, and bounded terminal retention.

### `fava-routing`

Use deterministic delayed routers to prove immediate partial results, later replacement snapshots, retractions, ordered downstream fallback reaction, deduplication, and explicit-route bypass.

### Router implementation crates

Use the shared routing conformance kit plus algorithm-specific examples. The primitive routing crate must not contain tests that encode NIP-65, hint, app-relay, or fallback policy.

### `fava-subscriptions`

Use differential tests: the grouped wire plan and an ungrouped reference plan must produce identical logical query results and evidence. Test limits and explicit shortfalls separately.

### `fava-transport` / `fava-wire` / `fava-ingest`

Use byte-exact scripted relay tests for framing, generation identity, malformed input, off-filter events, signature rejection, AUTH, CLOSED, EOSE, reconnect, and stale frames.

### `fava-publication`, `fava-publisher`, and `fava-delivery`

Use headless schedules and model tests for acceptance-before-effect, signer/route independence, ambiguous handoff, retries, give-up, per-relay outcomes, supersession, and stale generation rejection.

### Protocol crates

Use pure tests for typed decoding, event construction, and replaceable-event edit application. Add one public publication/query example only when it proves that a protocol crate reaches the ordinary Fava primitives correctly.

### SDKs

Use one shared behavior corpus. Platform-specific tests prove only native cancellation, lifecycle, packaging, and representation differences.

## 9. Distributed-systems rules

First identify the kind of promise:

| Promise | Required proof style |
|---|---|
| Safety | property/model test over operation orders |
| Liveness | controlled clock/deadline plus recovery |
| Durability | process stop/kill, reopen, reconstruct, continue |
| Isolation | compare accounts, sessions, requests, or relays for leakage |
| Truthfulness | compare public result with an independent witness |
| Boundedness | exceed the limit and observe explicit shortfall/backpressure |
| Idempotence | replay and duplicate delivery |

### 9.1 Control schedules

Use controlled clocks, barriers, channels, proxy gates, and witness signals. A longer sleep is not proof.

Exercise relevant orders:

- duplicate or reorder frames;
- reconnect during active work;
- return an old completion after a generation changed;
- switch identity or source;
- allow only some routers or relays to resolve;
- cancel before and after a handoff boundary;
- crash between durable facts and external effects.

### 9.2 Durability

A durability proof must:

1. create the fact through a supported operation;
2. stop at the claimed boundary;
3. destroy process/runtime state;
4. reopen through the supported construction path;
5. observe the public result; and
6. continue the operation to check identity and duplication.

Opening a second engine in the same process is not a process-restart proof.

### 9.3 Independent witnesses

Diagnostics report what Fava believes. They do not prove their own claims.

Use an independent witness for external effects:

- transparent wire proxy;
- relay process log;
- signer log;
- filesystem/process/port state;
- platform instrumentation.

## 10. Deterministic lab versus real relays

Use three environments deliberately:

1. **Deterministic scripted relay:** authoritative for exact protocol behavior and races.
2. **Third-party relay process in a local isolated lab:** authoritative for interoperability, real sockets/processes, and relay persistence.
3. **Public-relay mode:** reconnaissance for ecosystem behavior and long-lived observations; never deterministic pass/fail evidence.

BDD scenarios must not rely on uncontrolled public data.

## 11. Provider-contract TDD

A replaceable boundary is provisional until a meaningfully different implementation challenges it.

For each provider contract:

1. write contract examples before stabilizing the trait;
2. implement the first real provider through those examples;
3. write the conformance kit from behavior the first provider actually needs;
4. implement a second materially different provider, preferably outside the owning crate/workspace boundary;
5. remove methods that exist only because the first implementation leaked its shape; and
6. run architecture falsifiers and public composition tests.

A trait with one implementation is not evidence of substitutability.

## 12. Workflow by change type

### 12.1 Bug fix

1. Correct behavior text if the documented meaning was wrong.
2. Reproduce the defect at the narrowest stable owner.
3. Run and record the red failure.
4. Fix the owner, not a downstream symptom.
5. Run focused and affected integration tests.
6. Disable the fix and confirm the reproducer fails again.
7. Add a public capstone only if the bug crossed the public boundary in a way owner tests cannot prove.

### 12.2 New behavior

1. Add the behavior example and requirement link.
2. Add the smallest failing owner/property/model test.
3. Implement the vertical slice.
4. Add provider conformance where a contract is introduced.
5. Add or enable the milestone canary scenario.
6. Perform the named mutation.

### 12.3 Refactor with no intended behavior change

1. Identify the behavior that must remain unchanged.
2. Add characterization only where current proof is insufficient.
3. Do not add new BDD behavior text.
4. Refactor behind existing contracts.
5. Run exact owner, integration, public, and mutation checks affected by the change.

### 12.4 Performance change

1. Preserve or create the behavior oracle first.
2. Record the representative workload and baseline.
3. Attribute the cost before changing architecture.
4. Optimize one owner.
5. Run behavior, mutation, resource, and physical measurements together.
6. Reject an optimization that changes evidence, lifecycle, bounds, or failure truth.

## 13. Milestone discipline

Before implementation begins, every milestone slice must name:

- requirement/behavior IDs;
- owner;
- first failing proof;
- cross-owner/public capstone, when needed;
- mechanism-disable mutation;
- independent witness, when external effects are claimed; and
- advertised provider/profile guarantees involved.

A milestone exits only when:

- behavior text is accurate;
- focused evidence passes;
- the named mutation fails the evidence;
- the canary scenario passes where one is required;
- unimplemented specified behavior remains explicitly marked and issue-owned; and
- no unexecuted scenario appears built.

## 14. Review checklist

### Meaning

- [ ] Product meaning is in the specification/owning feature, not only in test names or chat.
- [ ] Each behavior scenario states one promise in app/Nostr terms.
- [ ] Contrasting cases are explicit.

### Red/green honesty

- [ ] The focused test was observed failing before the implementation.
- [ ] It failed for the intended reason after reaching its precondition.
- [ ] The smallest responsible owner was fixed.
- [ ] The focused test and affected tests were observed passing afterward.

### Mutation

- [ ] Removing/bypassing the protection makes the linked evidence fail.
- [ ] The failure is causal, not an unrelated panic or setup error.

### Fixtures

- [ ] Setup provides causes, not expected conclusions.
- [ ] Production constructors and operations create state.
- [ ] Public claims use public output or independent witnesses.
- [ ] Clocks, ports, identities, stores, processes, and teardown are isolated.

### Distributed behavior

- [ ] Relevant duplicate, reordering, reconnect, stale-completion, partial-result, cancellation, and crash orders are covered.
- [ ] Counts are not used where exact relationships matter.
- [ ] Ambiguous outcomes remain ambiguous.
- [ ] Limits produce explicit shortfall or refusal.

### Completion

- [ ] No duplicate harness/test proves the same path without additional value.
- [ ] Unrun live/platform checks are stated.
- [ ] Formatting, focused crate tests, public capstones, and workspace tests pass according to the current merge policy.

## 15. Anti-patterns

Reject these:

- implementation first, test afterward;
- feature files used as backlog or exhaustive test inventory;
- untagged/unmarked scenarios that are not executable but look built;
- broad end-to-end tests as the only proof of a local invariant;
- hand-written internal maps or tables in fixtures;
- a mock that implements the behavior under test;
- live public relays as the sole oracle;
- sleeps used to prove ordering or liveness;
- private-state assertions used to prove a public promise;
- several copies of the same test at different layers;
- a line/count/grep gate presented as behavioral proof;
- a conformance trait stabilized before a second implementation challenges it;
- a canary helper that conceals an awkward or missing public Fava API; and
- a green test that was never shown to fail when its protection was removed.

## 16. Minimal evidence record for a PR

No large process template is required. The PR or commit description should state:

```text
Behavior: <requirement / feature rule>
Owner: <crate / lifecycle owner>
Red: <command and intended failure>
Green: <command and result>
Mutation: <what was disabled and which evidence failed>
Capstone: <scenario, if applicable>
Unrun: <live/platform checks not executed>
```

The commands and artifacts matter; ceremony does not.
