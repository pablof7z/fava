# 0023 — Relay access, live state, and query authority have split owners

**Status:** all in-scope non-signature review blockers repaired and validated; uncommitted
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
- Keep the separately approved NIP-65 decoder pilot in its donor worktree. The
  state-foundation tree retains `WrongKind(u16)`, `InvalidRelay(String)`, and
  pre-pilot empty-marker behavior, with no pilot README or integration-test
  directory.

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

The named RED tests and deliberate breaks are those in revision 92's
`Executable proof ledger`. Focused tests precede workspace checks, Clippy,
formatting, API inventory, vocabulary, falsifier, Bazel, and live canary
validation. Evidence is recorded here before the slice is reported complete.

### RED and mutation evidence

- Four fresh causal behavioral REDs fail assertions rather than compilation or
  stale callers: hostile relevant-tag short-circuit
  (`/tmp/fava-state-foundation-final-red-hostile-sibling.log`), ignored exact
  access (`/tmp/fava-state-foundation-final-red-exact-access.log`), one-host
  truncation (`/tmp/fava-state-foundation-final-red-exact-host.log`), and
  cache-refusal-gated live admission
  (`/tmp/fava-state-foundation-final-red-admission-without-retention.log`).
- Fresh state/query mutations fail the intended behavior: erased query access
  (`/tmp/fava-state-foundation-final-mutation-access-identity.log`), reversed
  equal-time id ordering
  (`/tmp/fava-state-foundation-final-mutation-comparator-tie-break.log`),
  selection after coordinate choice
  (`/tmp/fava-state-foundation-final-mutation-selection-after-winner.log`), and
  later immutable replay overwrite
  (`/tmp/fava-state-foundation-final-mutation-immutable-replay.log`).
- Fresh boundedness mutations fail the intended behavior: changing 4,096 to
  4,097 (`/tmp/fava-state-foundation-final-mutation-exact-live-bound.log`) and
  refusing all changes merely because current state is at capacity
  (`/tmp/fava-state-foundation-final-mutation-capacity-replacement-deletion.log`).
  A fresh acceptance mutation that retains the preceding accepted revision's
  retractions fails exactly at `the refused overflow revision must not repeat
  replacement/deletion retractions`
  (`/tmp/fava-state-foundation-acceptance-mutation-stale-retractions.log`);
  restoration passes 2/2
  (`/tmp/fava-state-foundation-acceptance-overflow-restored.log`).
- Fresh closure mutations fail the intended proof: an unlisted Rust source
  calling the comparator
  (`/tmp/fava-state-foundation-final-mutation-unlisted-comparator-file.log`),
  disabled `self.helper` traversal
  (`/tmp/fava-state-foundation-final-mutation-self-helper-decoy.log`), README
  byte drift (`/tmp/fava-state-foundation-final-mutation-readme-byte-identity.log`),
  and hidden current `StateSlice` guidance
  (`/tmp/fava-state-foundation-final-mutation-hidden-current-path.log`).
  Every mutation was restored before final validation.

### Environmental blockers

- Explicit-session Pad reads work. Owner resolution and the final direct note
  remain blocked by sandbox refusal: `EPERM` opening
  `/Users/pablofernandez/.pad/owners.json` and
  `/Users/pablofernandez/pad/fava/2026-08-cross-crate-cleanup-proposals/notes.md`.
- Moving the isolated worktree under the repository succeeded physically, but
  Git could not repair its central worktree metadata:
  `could not open '/Users/pablofernandez/Work/fava/.git/worktrees/fava-state-foundation-approved/gitdir' for writing: Operation not permitted`.
  The nested worktree remains usable on `codex/state-foundation-approved`; no
  commit or merge was made.

### Final validation

- Acceptance-blocker proofs pass independently: comparator closure 5/5
  (`/tmp/fava-state-foundation-acceptance-comparator.log`), public cross-layer
  query-access lifecycle 1/1
  (`/tmp/fava-state-foundation-acceptance-query-access.log`), live overflow and
  stale-retraction behavior 2/2
  (`/tmp/fava-state-foundation-acceptance-overflow.log`), and structural
  subtraction/causality checks 12/12
  (`/tmp/fava-state-foundation-acceptance-subtraction-final.log`).

- The fresh post-change Cargo matrix passes formatting, workspace/all-target
  checking, and workspace/all-target/all-feature Clippy with warnings denied.
  The workspace test aggregate reaches its only two non-green tests in
  `vocabulary_governance`: the deliberately unsigned approval gate and the
  existing repository-wide 207-item terminal-name review backlog. A full
  workspace aggregate with those two cases explicitly skipped passes 550/550
  tests across 127 result groups, with two filtered tests total. The independent
  Bazel aggregate executes all 84/84 test targets successfully. Final logs are
  retained at `/tmp/fava-state-foundation-acceptance-check.log`,
  `/tmp/fava-state-foundation-acceptance-clippy.log`,
  `/tmp/fava-state-foundation-acceptance-fmt.log`,
  `/tmp/fava-state-foundation-acceptance-cargo-all.log`,
  `/tmp/fava-state-foundation-acceptance-cargo-green.log`, and
  `/tmp/fava-state-foundation-acceptance-bazel.log`.
- The fresh Python matrix passes all 138 vocabulary/subtraction unit tests and
  reports byte-current README inventories for `fava-relay`, `fava-state`,
  `fava-query`, and `fava-simple-groups`. The standalone vocabulary scanner
  still reports the unrelated repository-wide architectural review backlog;
  it is recorded as non-green rather than hidden. The final log is retained at
  `/tmp/fava-state-foundation-acceptance-python.log`; README and standalone
  scanner logs are `/tmp/fava-state-foundation-acceptance-readme-api.log` and
  `/tmp/fava-state-foundation-acceptance-vocabulary.log`.
- Both external falsifiers pass their tests and Clippy with warnings denied.
  The canary package passes check and Clippy. Current logs are retained at
  `/tmp/fava-state-foundation-acceptance-external-null-cache-test.log`,
  `/tmp/fava-state-foundation-acceptance-external-null-cache-clippy.log`,
  `/tmp/fava-state-foundation-acceptance-external-semantic-test.log`,
  `/tmp/fava-state-foundation-acceptance-external-semantic-clippy.log`,
  `/tmp/fava-state-foundation-acceptance-canary-check.log`, and
  `/tmp/fava-state-foundation-acceptance-canary-clippy.log`. Its read-only
  public-relay reconnaissance reached EOSE from `wss://relay.damus.io` with two
  preserved frames after the application selected Rustls's `ring` provider.
  That result proves live relay/TLS reachability only, not Fava state behavior.
  Final logs for that earlier live reconnaissance are retained at
  `/tmp/fava-state-foundation-final-live-canary.log` and
  `/tmp/fava-state-foundation-live-canary/public-relay-recon-92e6685847fb70f0`.
- The fresh causal RED and named mutation logs listed above remain the final
  mutation matrix. Every break failed its intended behavioral or closure
  assertion and was restored before the Cargo/Python/Bazel/falsifier matrix.
- The earlier `dx-flows` aggregate is withdrawn as current canary evidence for
  signer/account walls. Flows 3 and 4 now exercise the current public
  `Fava::add_signer` path and explicit-author publication instead of emitting
  the retired wall. The old run's relay readback measurements remain
  historical only; issue 0023 makes no current signer/account wall claim and
  does not reuse that aggregate as current evidence.
- Vocabulary candidate research is mechanically current and its unit tests
  pass, but blocked candidate rows are not claimed approved or
  aligned. The standalone scanner's unrelated repository-wide review backlog
  and the two signature/governance tests remain explicitly non-green.
- No vocabulary signatures, commit, or merge were made. The branch remains
  `codex/state-foundation-approved` at pre-slice HEAD
  `0aea6bbbcebaa38d352eda84a72b53cf6820e539`.
