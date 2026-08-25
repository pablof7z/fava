# 0023 — Relay access, live state, and query authority have split owners

**Status:** reconstructed on current main; current validation recorded below
**Raised:** 2026-08-25, approved by Pablo in pad
`fava/2026-08-cross-crate-cleanup-proposals` as `STATE-ARCH-1`,
`RELAY-ID-1`, and `QUERY-ACCESS-1`

## Problem

Relay/access identity is currently an arbitrary serializable string owned by
`fava-state`. Universal state is cache-shaped, so optional retention can gate
live delivery. Query evaluation merges relay provenance before checking exact
access and can select one replaceable winner per relay rather than one winner
per coordinate. `fava-simple-groups` then repeats same-id merge, provenance,
winner, and disagreement projection after query evaluation.

Those paths contradict the approved ownership model and the authoritative
specification passages cataloged in revision 92 of the pad.

## Resolution

- Add inert `fava-relay` ownership of exactly `RelayAccess` and
  `RelaySessionKey`.
- Replace cache-shaped universal values with atomic `RelayEvent`, event-bound
  `RelayOccurrences`, exact `EventStateMutation`, and pure state functions.
- Make relay admission complete before optional retention and give each open
  observation an independent live relay source.
- Bound each observation's live state to 4,096 retained events per exact
  `RelaySessionKey`. A transition whose atomic result would exceed the bound is
  refused without partial mutation and reported as
  `QueryShortfall::LiveRetentionLimit { session, limit, refused }`.
- Preserve exact access in query identity, planning, transport, observation,
  lifecycle completion, and evaluation. Filter atomic relay contributions
  before same-id merge, then choose one cross-source winner per coordinate.
- Discover every Rust file beneath every Cargo workspace member plus application
  and falsifier targets for comparator closure. Resolve free helpers,
  cross-file aliases, associated-function aliases, and `self.helper` paths to a
  fixed point; retain the exact path/module/impl/signature sink manifest as the
  closed allowlist. Classify every remaining arbitrary timestamp- or event-ID-
  shaped ordering expression in a second exact manifest, including standalone
  `<`, `<=`, `>`, `>=`, `cmp`, `partial_cmp`, `lt`, `le`, `gt`, `ge`, `max`, and
  `min` forms. Assert that the NIP-02, NIP-65, and subtracted simple-groups
  modules own no local selection. This discovery found and removed two
  additional raw comparator copies in the memory and Redb write-store
  source-qualification helpers.
- Delete the complete simple-groups snapshot/projection family. Return one
  bounded exact-host query per configured host.
- Replace the approved authoritative passages, all callers, manifests, Bazel
  targets, API inventories, current planning state, and vocabulary in the same
  slice. No aliases, wrappers, or compatibility paths remain.
- Reconcile stale vocabulary-research names and evidence locations to the
  current source without claiming that blocked candidates are approved or
  aligned. The unrelated `ContactListRowEvidence` candidate remains blocked
  pending a separate NIP-02 architecture decision.
- Prove `QUERY-ACCESS-1` through one real public `Fava` lifecycle: facade query
  identity, no-grouping planning, exact transport sessions, observation
  evidence and provenance, authenticated reconnect generation, and isolated
  facade withdrawal.
- Clear accepted live retractions before evaluating a later overflowing
  revision. The overflow test first observes exact replacement and deletion
  retractions, then proves the refused revision has none and preserves the
  complete accepted event-ID vector.
- Preserve the current tolerant NIP-65 decoder while removing its duplicate
  event identity and winner comparator. `relay_lists` composes the ordinary
  query owner; `RelayList` only parses the selected event record.

The exact cache contract, errors, maintenance API, provider lifecycle, and
conformance testkit remain deferred to the separately reviewed cache pilot.
This slice changes only cache dependencies forced by the approved universal
state values.

### Authorized provisional architecture decision

The recorded provisional overnight decision authorizes `fava-observe` to use
one fixed 4,096-event bound per exact `RelaySessionKey` and extend the existing
`QueryShortfall` enum with `LiveRetentionLimit`. This is Fava observation
policy, not a Nostr rule. It adds no configuration concept, provider contract,
lifecycle owner, alias, or compatibility path. Replacement/deletion batches
with a bounded final size still apply at capacity; an overflowing batch is
refused atomically and increments a saturating refusal count. Pablo may
overrule the fixed capacity or atomic-refusal policy before merge.

## Evidence

Reconstruction starts from current main `da9d322`, using `9b66b37` only as a
donor for individually reconciled files and hunks. The rescue snapshot was not
merged or cherry-picked.

Behavior-first commit `aaec9fd` establishes the accepted contracts. Against
unchanged current-main production code, `event_state_model` fails to resolve
the new state vocabulary and the 12 structural subtraction checks report eight
failures, three errors, and one pass. Production commit `5a0e5a7` then supplies
the owning implementation. `c9fdee9` repairs a current-main test fixture whose
empty `unresolved` set became terminal after settlement was made derived;
runtime behavior is unchanged. `194a65e` removes a test-only nominal wrapper.

Fresh deliberate breaks prove the named boundaries causally:

- ignoring `Query::with_relay_access` fails `access_identity` 0/1;
- reversing the equal-time event-ID tie-break fails `event_state_model` 0/1;
- changing the live bound from 4,096 to 4,097 fails
  `relay_occurrence_bound` 0/1 at the overflow deadline.

Every break was restored. The restored focused suites pass: query access 1/1,
state model 5/5, and live occurrence bounds 2/2. The structural subtraction
suite passes 12/12.

Current validation:

- `cargo check --workspace --all-targets --all-features`, strict workspace
  Clippy, and formatting pass.
- The workspace/all-target/all-feature test aggregate passes with only the two
  repository-wide vocabulary approval cases explicitly filtered. Those remain
  independently non-green: unsigned approvals and the existing terminal-name
  review backlog.
- All 151 tooling unit tests pass. README API inventories are byte-current for
  `fava-relay`, `fava-state`, `fava-query`, `fava-nip65`, and
  `fava-simple-groups`.
- Both external falsifiers pass tests and strict Clippy under locked manifests.
  The canary passes locked check and strict Clippy.
- `python3 tools/check_vocabulary.py` reaches only the independent repository
  approval, terminal-name, undocumented-symbol, and future-crate backlog; the
  reconstructed candidate evidence has no validation error. No signature is
  fabricated and no blocked candidate is claimed approved.
- Bazel reaches repository analysis after adding the missing current NIP-65
  architecture source target. The aggregate remains environmentally blocked:
  extraction of the Rust 1.90 rustfmt toolchain fails with `No space left on
  device` while only 186 MiB is free. Bazel is not claimed green.

The linked-worktree Git directory is read-only in this sandbox. Review commits
therefore live in the adjacent writable Git directory
`/private/tmp/fava-state-reconstruction-review.git`, with this checkout as its
work tree. A portable bundle is produced after the final catalog commit.
