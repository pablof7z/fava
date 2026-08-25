# Universal same-coordinate replaceable-edit composition

**Status:** in progress
**Branch:** `fix/replaceable-edit-composition`
**Authority:** WRITE-004/006/007/024/029, PROTO-001/002/003, M7
**Approved by:** Pablo, 2026-08-25

## Defect

The write stores index one live semantic receipt per replaceable coordinate but
retain only one edit under that receipt. A distinct second edit is therefore
refused while the first materialization is still unsigned. Publication also
selects only signed events as semantic source state, so it cannot apply the
second edit to the exact current unsigned body.

This rejects ordinary offline composition such as two follows or two bookmark
changes even though one coordinate already has a bounded, generation-fenced
publication obligation.

## Decision

One replaceable coordinate has one active write and receipt. Before signature,
a distinct same-coordinate edit appends to that operation's durable ordered
edit sequence, materializes over the exact current unsigned body, advances the
`MaterializationId`, and returns the same `WriteId` and `ReceiptId`.

Source-driven rematerialization replays the ordered sequence through the
selected protocol materializer. The sequence is persisted by every write-store
provider and recovered before new commands. Its length cannot exceed the
existing bounded retained-generation evidence: every appended edit retires
exactly one materialization, and overflow refuses atomically.

The write store remains the sole coordinate, custody, operation, receipt, and
generation owner. Protocol materializers remain pure and receive an ordinary
signed or unsigned event value; no protocol crate owns a queue, batch, receipt,
or recovery path.

## Outcomes

- Two distinct pre-signature edits at one author/kind/identifier coordinate
  produce one deterministic current body in acceptance order.
- Both publish calls address the same exact write and receipt; the second body
  has the next exact materialization generation.
- A signer completion for the retired generation is inert and attributable to
  its original operation, generation, and event identity.
- Redb reopen recovers the ordered edit sequence and replays it over a newer
  qualified source without losing either accepted change.
- The sequence and retired evidence refuse overflow without partial mutation.
- The behavior is protocol-neutral and is proved through the public `fava`
  door; no bookmark-specific or NIP-02-specific lifecycle exists.

## Exclusions

- Post-signature supersession and cross-route policy changes.
- Protocol-specific edit normalization or inverse detection.
- Compatibility schema readers, aliases, queues, or batching APIs.

## Root-cause falsifier

With a blocking signer and active capacity one, publish two distinct edits for
the same coordinate. The second publish must succeed, return the first
operation identity at generation two, expose the ordered composed body, and
make a completion carrying generation one plus its event id refuse without
mutation. Restoring either the coordinate-conflict refusal or signed-only
source selection must fail this proof causally.

## Red evidence

`cargo test -p fava --test semantic_write_publication \
distinct_unsigned_edits_compose_under_one_exact_operation -- --exact` compiled
the public proof and failed at the second `publish`: `Publication(Store(Refused(
"bounded write-store capacity 1 reached")))`. The first edit had already entered
custody and started generation-one signing. This isolates the first causal gate:
reservation counts a same-coordinate composition as a second active operation.

## Validation gates

- Focused public behavior and stale-completion tests.
- Memory and redb write-store owner tests, including overflow atomicity.
- Redb close/reopen/rematerialize proof.
- Protocol-capability corpus for NIP-02 and bookmarks.
- `python3 tools/check_vocabulary.py` and its unit tests.
- Focused Cargo/Bazel targets, formatting, lint, and `git diff --check`.
