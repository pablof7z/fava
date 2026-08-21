---
phase: 07
slug: semantic-writes-and-capability-composition
status: verified
threats_open: 0
asvs_level: 1
created: 2026-08-21
verified_head: 0e87083dcd46acb0609100ccdc870d376b581433
---

# Phase 07 — Security

## Trust Boundaries

| Boundary | Data crossing | Security rule |
|----------|---------------|---------------|
| Application → write intent | raw event or authorless `{ kind, identifier, change }` edit plus accepted author | validate bounds and freeze author before custody |
| Protocol materializer → publication | unsigned event derived from selected source and injected timestamp | exact author, coordinate, timestamp, identity, and size validation before effects |
| Publication → write store | receipt and generation mutations | exact operation, materialization, event, session, attempt, and revision compare-and-set |
| Cache/write-store observations → semantic runner | changing qualified source state | independent bounded observations; failures and closure remain source-scoped |
| Publisher/transport → delivery evidence | external attempts and outcomes | bounded queues and exact generation/destination attribution |
| Durable redb bytes → recovery | schema-v2 semantic custody | strict version/invariant validation; no fallback decoder or migration |
| Capability/canary workspaces → product graph | local protocol crates and external public-only proof | locked graphs and negative dependency paths; no universal-core kind switch |

## Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation evidence | Status |
|-----------|----------|-----------|----------|-------------|---------------------|--------|
| T-07-01 | Tampering | edit decode | high | mitigate | exact authorless edit shape, bounded change, superseded actor/format/inverse fields refused | closed |
| T-07-02 | Denial of Service | edit/evidence bounds | high | mitigate | edit, receipt text, destination, retained generation, and evidence caps refuse atomically | closed |
| T-07-03 | Elevation of Privilege | materializer contract | high | mitigate | pure public contract has no signer, router, publisher, cache mutation, or receipt ownership | closed |
| T-07-04 | Repudiation | generation attribution | medium | mitigate | stable write/receipt with exact materialization, event, source, and retired evidence | closed |
| T-07-05 | Tampering | generation mutation | high | mitigate | exact current identity and source compare-and-set in memory and redb | closed |
| T-07-06 | Denial of Service | active custody | high | mitigate | store-owned global reservations and atomic capacity/evidence refusal | closed |
| T-07-07 | Repudiation | retired/failed work | medium | mitigate | bounded generation/source/event failure and retired-completion attribution | closed |
| T-07-08 | Elevation of Privilege | observer visibility | high | mitigate | notify after commit; unpublished events remain in the independent write-store source | closed |
| T-07-09 | Spoofing | source selection | high | mitigate | exact author/kind/identifier qualification and canonical timestamp/event-id winner | closed |
| T-07-10 | Denial of Service | provider/task admission | high | mitigate | bounded selection and store reservation occur before materializer/provider effects | closed |
| T-07-11 | Tampering | preview/live parity | high | mitigate | shared materialize-and-route path with zero-effect preview proof | closed |
| T-07-12 | Elevation of Privilege | materializer output | high | mitigate | exact author, coordinate, timestamp, identity, and size validation before custody | closed |
| T-07-13 | Tampering | late completion | critical | mitigate | exact generation/session/attempt/revision CAS; stale work cannot mutate current state | closed |
| T-07-14 | Denial of Service | tasks/queues/route progress | high | mitigate | bounded channels plus committed mutation results advance route revision despite transient reads | closed |
| T-07-15 | Repudiation | provider failure/panic | high | mitigate | panic/error isolation and bounded source/generation evidence | closed |
| T-07-16 | Elevation of Privilege | malformed event | high | mitigate | exact output boundary validation, including injected timestamp, before signing/routing | closed |
| T-07-17 | Tampering | redb transaction | critical | mitigate | one exact-ID transaction; stale or invalid mutations are atomic no-ops | closed |
| T-07-18 | Denial of Service | durable rows/evidence | high | mitigate | strict active/terminal/text/destination/evidence bounds at reopen and commit | closed |
| T-07-19 | Repudiation | crash recovery | high | mitigate | stable custody/source/current/retired/failure facts survive real SIGKILL | closed |
| T-07-20 | Elevation of Privilege | schema decode | high | mitigate | hard schema v2 with strict invariant validation and no fallback compatibility path | closed |
| T-07-21 | Tampering | protocol decode/rewrite | high | mitigate | strict kind/author/source validation and deterministic affected-tag edits | closed |
| T-07-22 | Denial of Service | protocol collections | high | mitigate | hostile source/tag/output bounds and typed refusal before growth | closed |
| T-07-23 | Elevation of Privilege | protocol dependencies | high | mitigate | Cargo/Bazel negative paths keep lifecycle owners out of capability crates | closed |
| T-07-24 | Information Disclosure | bookmarks | high | mitigate | public bookmarks only; no encrypted/private content parsing, keys, or claim | closed |
| T-07-25 | Elevation of Privilege | external capability | high | mitigate | public-Fava-only compilation and universal output validation | closed |
| T-07-26 | Tampering | future raw events | high | mitigate | arbitrary kind/created-at/tags/content and event identity publish unchanged | closed |
| T-07-27 | Denial of Service | external input/output | high | mitigate | shared edit/source/output/admission/evidence bounds and typed refusal | closed |
| T-07-28 | Repudiation | external late work | medium | mitigate | public receipt proves stable identity and exact retired materialization behavior | closed |
| T-07-29 | Tampering | shared corpus | high | mitigate | both capabilities run the same parameterized lifecycle; core kind-switch scan is empty | closed |
| T-07-30 | Repudiation | canary evidence | high | mitigate | exact source/current/retired/route/attempt IDs plus injected transient-route fault proof | closed |
| T-07-31 | Denial of Service | canary process/artifacts | medium | mitigate | bounded output, one absolute deadline, process-group cleanup, fixed seven-file bundles | closed |
| T-07-32 | Elevation of Privilege | N+1 invocation | high | mitigate | independent public-only manifest remains outside product Cargo/Bazel dependency graphs | closed |
| T-07-33 | Repudiation | stale-completion break | high | mitigate | causal predicate break, state counterexample, restoration, and exact regression | closed |
| T-07-34 | Tampering | feature mapping | high | mitigate | locked Cargo resolution, exact test discovery, and malformed/duplicate/zero-test refusals | closed |
| T-07-35 | Denial of Service | evidence/source growth | medium | mitigate | tag/byte/file/line/process bounds with hostile and deliberate-break proofs | closed |
| T-07-36 | Elevation of Privilege | protocol dependencies | high | mitigate | compile-negative signer reference, metadata, Cargo-tree, and Bazel path gates | closed |
| T-07-37 | Elevation of Privilege | public vocabulary | high | mitigate | exact public allowlists, declaration/re-export scans, and vocabulary checker | closed |
| T-07-38 | Repudiation | milestone record | medium | mitigate | current CAP map, independent audits, fresh CLI evidence, and clean phase-range gates | closed |

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-07-01 | T-07-SC | Existing locked third-party dependencies may be compromised upstream or contain undiscovered vulnerabilities. M7 adds no third-party package; root, canary, and external lockfiles plus normal-dependency graph gates were verified. Re-evaluate on any dependency or lockfile change and at the next milestone security audit. | Phase 07 plan threat registers | 2026-08-21 |

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-08-21 | 38 | 38 | 0 | gsd-security-auditor, ASVS L1 |

## Sign-Off

- [x] All threats have a disposition.
- [x] The single accepted low risk is documented.
- [x] `threats_open: 0` confirmed at `0e87083dcd46acb0609100ccdc870d376b581433`.
- [x] `status: verified` set.

**Approval:** verified 2026-08-21
