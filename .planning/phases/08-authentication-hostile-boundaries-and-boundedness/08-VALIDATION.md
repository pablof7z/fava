---
phase: 08-authentication-hostile-boundaries-and-boundedness
status: pending
nyquist_compliant: false
wave_0_complete: false
updated: 2026-08-22
---

# Phase 08 Validation Strategy

## Contract

Plans 02-04 alone adopt the current dirty/untracked WIP, then extract the three known soft-limit crossings. Plans 07-08 author approved Runtime/provider/FavaBuilder/standard source without owner migration. Plan 09 solely owns every Runtime/standard/consumer Cargo, Bazel, MODULE, and Cargo.lock mutation; it also creates honest compile-only smoke sources for every absent predeclared target before compiling the entire pre-migration graph. Plans 10-13 replace/extend those scaffolds with exact behavioral RED before each migration. Plan 14 audits explicit migrations and supplies the exhaustive line checker. Plans 15-17 prove external effects; Plan 18 alone seals manifests and the committed clean tree.

## Waves and Dependencies

| Wave | Plans | Ownership |
|---|---|---|
| 0 | 01 | blocking Go 1.25/Khatru prerequisite |
| 1 | 02 | delivery WIP adoption/public closure and fava-write extraction |
| 2 | 03,04 | hostile WIP vs store WIP/extractions; disjoint |
| 3 | 05 | OPS-004 ledger after final hostile frame/message bound |
| 4 | 06,07 | non-runtime bounds vs approved Runtime/provider source |
| 5 | 08 | FavaBuilder Runtime selection first; standard_builder source second |
| 6 | 09 | all Runtime/standard/consumer metadata, one lock, pre-migration compile |
| 7 | 10,11 | publication vs auth/routing/NIP-11 compiled RED migrations; disjoint |
| 8 | 12,13 | observe/facade vs publisher/delivery/signer compiled RED migrations; disjoint |
| 9 | 14 | repository ownership audit, final ledger, line-gate checker |
| 10 | 15 | hostile real-process/socket canary |
| 11 | 16 | persistent auth/account/NIP-11 canaries |
| 12 | 17 | delivery/provider process canaries and public CLI behavior |
| 13 | 18 | exact manifests and final committed-tree gate |

Same-wave file ownership is disjoint. Plan 03 Task 1 is the explicit six-file atomic WIP adoption/extraction exception. Plan 09 exceeds 15 files only because the locked centralized graph decision requires one graph/lock/scaffold owner; its eight tasks each own at most five files. Plans 17 and 18 own 11 and 9 files respectively; every task is at most five files.

## Exact Task Map

| Tasks | Evidence |
|---|---|
| 08-01-01..02 | exact Go 1.25, module verify/test/build, real readiness/NIP-11/reap probe |
| 08-02-01..03 | five dirty delivery adoptions, public identity/budget break, fava-write private extraction below 500 |
| 08-03-01..03 | sole relay/hostile adoption and extraction below 500, hostile matrix, admission break |
| 08-04-01..03 | sole Memory/Redb adoptions, parity/reopen break, Memory semantic extraction below 500 |
| 08-05-01..03 | complete OPS-004 ledger, diagnostics/evidence-envelope break, approved numeric bounds |
| 08-06-01..03 | non-runtime resource bounds and exact breaks |
| 08-07-01..03 | blocking vocabulary decision, Runtime contract and first Tokio provider source |
| 08-08-01..02 | FavaBuilder Runtime surface before standard_builder source; no owner migration |
| 08-09-01..08 | exact graph RED, all Cargo/Bazel/MODULE edges, one lock, nine honest smoke sources, pre-migration compile/breaks |
| 08-10-01..03 | materializer and publication-run compiled RED/GREEN/breaks plus five-mode matrix |
| 08-11-01..03 | auth, routing, confirmed NIP-11 timeout compiled RED/GREEN/breaks |
| 08-12-01..03 | confirmed query_source spawn, adopted relay timeout, route fan-out compiled RED/GREEN/breaks |
| 08-13-01..03 | publisher future and delivery/signer compiled RED/GREEN/breaks plus five-mode matrix |
| 08-14-01..03 | explicit-migration audit, final ledger, exhaustive modified-code line checker |
| 08-15-01..03 | hostile real-process/socket corpus and break |
| 08-16-01..03 | persistent auth, account isolation, live Khatru NIP-11/no-wire breaks |
| 08-17-01..03 | attempt/ambiguity restart, provider process isolation/break, three-ID public CLI behavior |
| 08-18-01..03 | seven-ID break, 24-break/76-threat manifests, exhaustive line/validation/final clean seal |

Total: 18 plans and 57 exact tasks. HARD-01..10 all occur in plan frontmatter and final evidence.

## Pre-Migration Runtime Build Graph Gate

Plan 09 exclusively owns root `Cargo.toml`, `Cargo.lock`, `MODULE.bazel`, runtime/provider/standard manifests and BUILD targets, and every Cargo/BUILD edge for `fava`, publication, auth, routing, observe, nip11-http, and publisher-nip01. Its graph RED captures status/output and passes only on the exact missing-root-edge assertion without import/syntax/harness error. Tasks 5-6 create all nine source-bearing `wave0_compile_smoke` harnesses, forbid future behavioral names/assertions/ignored tests, and change no metadata. Only then does Task 7 add final provider edges, run the first graph-checker GREEN/source-existence gate, and regenerate the single `Cargo.lock`. Task 8 compiles the complete still-unmigrated graph and executes/restores graph/interface breaks. Plans 10-13 must replace/extend their exact scaffold before behavioral RED; Plans 10-18 edit no Runtime migration metadata or lock.

### Pre-Migration Target Source Inventory

| Target source required by Plan 09 `:all` | Exists by | Pre-migration contract |
|---|---|---|
| `crates/fava-runtime/tests/contract.rs` | Plan 07 | real Runtime contract test |
| `crates/fava-runtime-tokio/tests/conformance.rs` | Plan 07 | real first-provider conformance |
| `crates/fava-standard/tests/assembly.rs` | Plan 08 | real facade/standard assembly test |
| `crates/fava-auth/tests/authentication.rs` | committed input | existing owner behavior |
| `crates/fava/tests/provider_failure_isolation.rs` | 08-09-05 | honest private smoke; replaced/extended 08-10 |
| `crates/fava/tests/runtime_auth_routing.rs` | 08-09-05 | honest private smoke; replaced/extended 08-11 |
| `crates/fava/tests/runtime_sessions.rs` | 08-09-05 | honest private smoke; replaced/extended 08-12 |
| `crates/fava/tests/runtime_delivery.rs` | 08-09-05 | honest private smoke; replaced/extended 08-13 |
| `crates/fava-publication/tests/provider_runtime.rs` | 08-09-05 | honest private smoke; replaced/extended 08-10 |
| `crates/fava-routing/tests/failure_isolation.rs` | 08-09-06 | honest private smoke; replaced/extended 08-11 |
| `crates/fava-observe/tests/runtime_ownership.rs` | 08-09-06 | honest private smoke; replaced/extended 08-12 |
| `crates/fava-nip11-http/tests/runtime_ownership.rs` | 08-09-06 | honest private smoke; replaced/extended 08-11 |
| `crates/fava-publisher-nip01/tests/runtime_ownership.rs` | 08-09-06 | honest private smoke; replaced/extended 08-13 |

Task 8's explicit `test -s` loop enumerates this same set, and `test_runtime_build_graph.py` independently rejects an absent literal BUILD source before Cargo/Bazel execution.

## Compiled RED Before Every Owner Migration

| Owner/resource | Exact pre-migration test | Required RED assertion | Migration plan |
|---|---|---|---|
| publication materializer/panic isolation | `runtime_materializer_scopes_panics` | `materializer work bypassed Runtime` | 08-10-01 |
| publication run task/timer/cancel/join/deadline | `runtime_publication_run_uses_runtime` | `publication run task bypassed Runtime` | 08-10-02 |
| auth provider work | `runtime_auth_provider_uses_runtime` | `authentication provider bypassed Runtime` | 08-11-01 |
| routing tasks/timers/joins | `runtime_routing_tasks_use_runtime` | `routing execution bypassed Runtime` | 08-11-02 |
| NIP-11 acquisition timeout | `runtime_nip11_deadline_uses_runtime` | `NIP-11 acquisition bypassed Runtime` | 08-11-03 |
| query-source/observation polling | `runtime_query_observation_uses_runtime` | `observation polling bypassed Runtime` | 08-12-01 |
| facade relay sessions/deadlines/joins | `runtime_relay_session_uses_runtime` | `relay session execution bypassed Runtime` | 08-12-02 |
| route fan-out/cancel/join | `runtime_route_fanout_uses_runtime` | `route fan-out bypassed Runtime` | 08-12-03 |
| publisher future/panic/cancel | `runtime_publisher_future_uses_runtime` | `publisher future bypassed Runtime` | 08-13-01 |
| delivery/signer/retry/deadline/join | `runtime_delivery_signer_uses_runtime` | `delivery/signer execution bypassed Runtime` | 08-13-02 |

For every row, the task captures the single filtered Cargo command against still-unmigrated production, requires nonzero plus the exact assertion, rejects compiler/import/syntax/unrelated failures, commits RED alone, implements GREEN, runs the same test under a named type-correct deliberate break, restores checksums, and records PASS. Source-shape scans are backstops only.

## Confirmed Live Bypass Closure

| Live bypass | Modifying task | Ordering | Backstop |
|---|---|---|---|
| `crates/fava/src/query_source.rs` `tokio::spawn` | 08-12-01 | after Plan 09 compiled graph | 08-14 exact-file assertion |
| adopted `crates/fava/src/relay.rs` timeout | 08-12-02 | after Plan 03 adoption and Plan 09 graph | 08-14 exact-file assertion |
| `crates/fava-nip11-http/src/lib.rs` provider timeout | 08-11-03 | after Plan 09 compiled graph | 08-14 exact-file assertion |

The audit cannot satisfy a row unless its modifying task and exact compiled RED/GREEN/break pass.

## 500/800 Modified-Code Gate

Plans 02, 03, and 04 use cohesive private-module extraction to bring the known 557-line `fava-write/src/lib.rs`, 566-line `fava/src/relay.rs`, and 502-line `fava-write-store-memory/src/semantic.rs` below 500. Plan 14 creates `test_m8_line_gate.py`, deriving the exact code-path set from all 18 plan frontmatters. Plan 18 writes `08-LINE-GATE.tsv`; the checker rejects missing, extra, duplicate, wildcard, directory, stale-count, blank/blanket reason, any file over 800, and every unreasoned 500+ crossing.

## Spec-less Edge Disposition

| Probe row | Exact disposition | Owning evidence |
|---|---|---|
| HARD-01 unclassified | Fresh NIP-42 challenge after real restart; prior generation cannot authorize write. | 08-16-01 |
| HARD-02 unclassified | Concurrent deny/allow accesses complete independently. | 08-16-02 |
| HARD-03 boundary | Maximum frame admitted; max+1 refused before parsing/cache. | 08-03-01, 08-15-01 |
| HARD-03 precision | Each hostile class has exact session/request/generation oracle. | 08-03-02, 08-15-02 |
| HARD-04 unclassified | Live Khatru limits cause exact shortfall and independent zero-wire witness. | 08-01-02, 08-16-02 |
| HARD-05 unclassified | Offline time spends zero; each real handoff consumes one. | 08-02-01, 08-17-01 |
| HARD-06 unclassified | Real failures stop at configured ceiling within bounded time. | 08-17-01 |
| HARD-07 unclassified | Proxy sees full handoff/cut-before-OK; restart retains ambiguity. | 08-17-01 |
| HARD-08 boundary | Ledger max accepted; max+1 typed refusal before custody/effect. | 08-05-01, 08-06 |
| HARD-08 adjacency | Unrelated owner remains usable under pressured-owner refusal. | 08-06, 08-15-02 |
| HARD-08 empty | Empty input creates zero work and exact zero envelope. | 08-05-01..02, 08-06 |
| HARD-08 ordering | Barrier-controlled pressure yields deterministic order/accounting. | 08-05-02, 08-06 |
| HARD-08 precision | Each row reports exact refusal/backpressure/shortfall and maximum. | 08-05-01, 08-06-02, 08-14-02 |
| HARD-09 unclassified | Five provider modes are scoped with adjacent progress and bounded shutdown. | 08-07..14, 08-17-02 |
| HARD-10 adjacency | Each process bundle has independent relay/proxy/OS witness. | 08-15..17 |
| HARD-10 empty | Empty-success/failure bundles remain schema-valid without false success. | 08-05-02, 08-15-02, 08-18-02 |
| HARD-10 ordering | Barrier/proxy gates establish physical order; sleeps are deadlines only. | 08-15-02, 08-16, 08-17-01 |
| HARD-10 concurrency | Account/provider/hostile lanes overlap with unrelated progress. | 08-15-02, 08-16-02, 08-17-02 |

All 18 probe rows have exact tests. None is silently dismissed.

## Descriptor-less Prohibition Recall

Only project-value/safety must-NOTs remain: hostile/over-limit input cannot acquire owner state; offline time cannot spend delivery budget; ambiguity cannot be rewritten; owners cannot regain execution resources; universal crates cannot depend on `TokioRuntime`; evidence cannot silently truncate or claim unwitnessed success; the pinned Khatru module, dirty WIP, and `stash@{0}` cannot be rewritten. Each is under `must_haves.prohibitions` and has a named break, exact source/dependency gate, process witness, or final identity gate. No descriptor-less prohibition is auto-dismissed or promoted into a truth.

## Wave 0 Gate

```bash
go version | rg 'go1\.25\.'
(cd apps/canary/relays/khatru && GOTOOLCHAIN=local go mod verify && GOTOOLCHAIN=local go test ./... && GOTOOLCHAIN=local go build ./... && ./probe.sh)
```

The real probe must start, become ready, serve NIP-11, terminate, and wait/reap its exact PID.

## Final Executable Gate

After all 57 task commits:

```bash
M8_FINAL_RUNS="$(mktemp -d)"
for scenario in nip42-write-and-reconnect auth-account-isolation hostile-relay-ingress relay-limit-shortfall ambiguous-handoff attempt-ceiling provider-failure-isolation; do
  cargo run --manifest-path apps/canary/Cargo.toml -- run "$scenario" --seed "m8-final-$scenario" --runs-dir "$M8_FINAL_RUNS/$scenario"
done
python3 -m unittest tools.tests.test_m8_line_gate
git diff --check caeee9e73f2b3919934bcb70043491d33c200daa..HEAD
git diff --check
test "$(git rev-parse 'stash@{0}')" = 5faecf42c0ec903507e3faeb04962f4680a9cb44
test -z "$(git status --porcelain --untracked-files=all)"
```

`08-BREAK-MANIFEST.tsv` contains 24 exact literal marker rows; `08-THREAT-MANIFEST.tsv` contains T-08-01 through T-08-76 once each; `08-LINE-GATE.tsv` exhaustively covers every modified code path. The clean-status command runs only after Task 18-03 is committed.

## Completion Checklist

- [ ] 57 exact tasks pass; HARD-01..10 and all 18 spec-less rows are mapped.
- [ ] Plan 09 compiles the complete graph and first real provider before every owner migration.
- [ ] Every migration has a committed exact compiled behavioral RED, GREEN, same-test break, and restoration.
- [ ] Three confirmed live bypasses are modified and independently audited.
- [ ] Every modified code path passes the exhaustive 500/800 manifest gate.
- [ ] Every threat/break ID is exact and uniquely disposed.
- [ ] Seven process/socket scenarios pass; source WIP is committed; stash remains unchanged; tree is clean.

## Multi-Source Coverage Audit

| Source | Item | Coverage |
|---|---|---|
| GOAL | Exact isolated outcomes under auth, hostile input, overload, provider failure, retry, ambiguity, and shutdown pressure | Plans 02-17 |
| REQ | HARD-01 through HARD-10 | Every ID occurs in plan frontmatter and final evidence |
| RESEARCH | Existing delivery/hostile WIP closure | Plans 02-04, one adoption owner per path |
| RESEARCH | OPS-004 ledger and non-runtime bounds | Plans 05-06 and final Plan 14 reconciliation |
| RESEARCH | Authoritative Runtime/provider ownership and all owner migrations | Plans 07-14, including every confirmed bypass |
| RESEARCH | Real auth/NIP-11/hostile/delivery/provider evidence and final seal | Plans 15-18 |
| CONTEXT | No Phase-08 CONTEXT.md exists | No deferred idea or D-NN source; explicit revision decisions are implemented |

No GOAL, REQ, RESEARCH, or available CONTEXT item is missing.
