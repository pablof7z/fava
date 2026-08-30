# 0054 — Current account is one session-owned reactive root

**Status:** approved for implementation
**Approved by:** Pablo, 2026-08-30 (`build-account-experiential-repl` apply authorization)
**Authority:** ID-001–ID-003, WRITE-003, QUERY-001, QUERY-002;
`partial-spec-api-semantics.md` current-account reactive-root contract

## Defect

`fava-session` owns only an unbounded signer map. It cannot represent a
pubkey-only account or current selection. `Fava::publish` refuses every
authorless builder/edit unless the app passes `.by(pubkey)`, and `Query` can
express only literal filters. An app must therefore own a duplicate selected
pubkey, thread it into writes, and close/rebuild observations after switches.
Those workarounds violate the authoritative owner and reactive-query model.

Concrete counterexamples:

- selecting B in an app-local alias map cannot change any Fava-owned fact;
- `fava.publish(EventBuilder::new(kind))` returns `MissingAuthor` even when a
  session account is selected;
- an open literal-A observation retains A demand and results forever after B is
  selected unless application code replaces it.

## Ownership decision

`fava-session::Session` owns the bounded account set, optional current public
key, exact signer attachments, and one monotonic session revision; that one
counter also supplies exact signer attachment generations. Adding a signer also
adds its account when absent. Removing a
signer leaves a pubkey-only account. Removing an account atomically detaches its
signer and clears current selection when applicable. Selection never deletes
cached events, accepted writes, or receipts.

`fava` snapshots current selection before converting an authorless payload into
one ordinary `WriteIntent`. No second write path, receipt lifecycle, or author
field is introduced.

`fava-query` owns declarative markers on author and tag-value axes that mean
“bind this axis to current account.” `fava-observe` binds those markers from the
session snapshot and owns automatic replacement of concrete local, route, and
relay work behind one stable application `Observation`. Empty selection
compiles to a present empty filter and opens no relay demand. Retired concrete
observations reject late work through their existing exact observation,
operation-generation, plan-revision, and wire identities.

## Approved public vocabulary

No new crate or public nominal noun is required. The existing specified nouns
`Session`, `Fava`, `Query`, `FilterSelection`, `Observation`, `PublicKey`, and
`PublishError` own the behavior. `$currentPubkey` is the account REPL’s literal
query token, not a second runtime value owner.

Approved public symbols:

```text
fava_session::Session::add_account
fava_session::Session::accounts
fava_session::Session::select_account
fava_session::Session::clear_current_account
fava_session::Session::current_account
fava_session::Session::current_account_snapshot
fava_session::Session::remove_account
fava_session::Session::revision

fava::Fava::add_account
fava::Fava::accounts
fava::Fava::select_account
fava::Fava::clear_current_account
fava::Fava::current_account
fava::Fava::current_account_snapshot
fava::Fava::remove_account
fava::Fava::session_revision

fava_query::Query::authors_current_account
fava_query::Query::tag_value_current_account
fava_query::Query::depends_on_current_account
fava_query::Query::bind_current_account
```

`FilterSelection` exposes whether its author axis and which tag axes depend on
current account so alternative observation owners can implement the same
public contract. Existing `Fava::publish` and `PublishTo::publish` resolve the
current account only for authorless payloads; `Fava::by` remains the exact
explicit override. `PublishError::MissingAuthor` remains the pre-custody typed
refusal when no current selection exists.

### Closest concept and distinction

The closest Nostr concept is an event author public key. Current account is the
session’s optional selection among known public keys; it is mutable runtime
input, not durable event authorship, signer availability, an app alias, or a
query result.

### Forcing requirement

ID-001 assigns accounts and selection to session. ID-002 requires explicit
query dependencies to update and current-account writes to resolve before
acceptance. ID-003 requires no-current refusal before receipt creation.
WRITE-003 forbids retargeting accepted work.

### Why existing state is insufficient

The signer map cannot retain pubkey-only accounts or selection. `PublishAs`
requires the app to carry the selected key. Literal `FilterSelection` cannot
retain the fact that its concrete key must change.

## Bounds and refusal

A session retains at most 64 accounts and therefore at most 64 signer
attachments. Duplicate/missing account, duplicate/missing signer, capacity,
and generation exhaustion refuse atomically. Private keys remain signer-owned;
the session stores only public keys and provider handles.

## Executable falsifiers

- Seed or add 65 distinct signer-backed/pubkey-only accounts: the 65th refuses
  and state/revision remain unchanged.
- Remove selected A: account and signer disappear, selection becomes empty, and
  exactly one revision is emitted.
- Accept an authorless write under A and select B before delayed signing: only A
  can sign the accepted event. Moving current lookup after acceptance makes the
  test fail.
- Open one `$currentPubkey` observation under A, then select B: the handle id
  stays fixed while local result and relay `REQ` move to B. Suppressing session
  wakeup leaves A current and fails.
- Select A→B→C while A/B relay work is delayed: releasing old completions cannot
  alter C’s snapshot or demand. Reusing the old concrete observation identity
  makes the test fail.
- Clear selection: local evaluation returns no events and no broad relay `REQ`
  is emitted. Treating present-empty as absent makes the test fail.

## Session-slice evidence

- Red: `cargo test -p fava-session --test session` failed with 29 missing account/current-selection symbols before implementation.
- Green: 9 owner tests, 1 public facade test, and 8 runtime-signer regression tests pass.
- Mutation: suppressing selected-account clearing in `remove_account` makes `account_set_selection_and_revision_are_atomic_and_bounded` fail with the stale selected key.
- Strict Clippy passes for `fava-session` and the focused facade target; focused rustfmt and OpenSpec strict validation pass.
