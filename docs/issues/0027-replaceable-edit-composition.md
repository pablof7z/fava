# Universal same-coordinate replaceable-edit composition

**Status:** ready for independent re-review
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

## Independent-review blocker closure

Commit `cda66754` adds causal red proofs for two independent defects in
`31a87f1a`:

- an anonymous reservation can be consumed by another coordinate and an active
  coordinate can grow the reservation set without bound; and
- a failed durable-sequence refresh still advances the runner's local signing
  generation, after which successor installation accepts a replay carrying
  only the final edit. Memory, redb, and post-SIGKILL stores all accept that
  incomplete successor.

Commit `b186aa61` closes both ownership defects. Every reservation now carries
its exact author/kind/identifier coordinate in write-store state, with one
reservation per coordinate and inactive reserved coordinates counted against
the global active bound. Mismatched acceptance refuses without consuming the
owner's reservation; matching post-coordinate failures still consume it.

Durable custody reads now require the exact current `MaterializationId`.
Publication retries failed reads without reopening signing or routing, re-reads
the receipt if another composition supersedes the failed target generation,
and advances only after the newest exact sequence is available. Successor
installation receives the complete applied edit slice and refuses unless it
equals durable custody after exact write, receipt, generation, and source
validation.

## Restart-admission blocker closure

Commit `ddb56702` adds deterministic causal proofs for the remaining restart
race. Memory, clean-redb reopen, and redb SIGKILL recovery each place a newer
qualified source in the initial restart snapshot, build the facade, and submit
a same-coordinate edit immediately without yielding the current-thread
runtime. The returned facade itself is therefore the admission barrier. After
that composition, a second late source must replay the complete accepted
sequence. The custody-refresh failure proof now waits on an observed failed
durable read after source arrival instead of sleeping.

Commit `fdf5d5ce` makes initial semantic reconciliation part of synchronous
publication recovery rather than background-runner startup. The facade is not
returned until every recovered coordinate has considered its complete durable
sequence against the initial qualified source snapshot. Each runner state also
carries the exact `MaterializationId` of its loaded sequence; initialization
exact-refreshes custody whenever the receipt advanced before opening signer or
route work. Both reconciliation loops are bounded by the existing retained
materialization-evidence capacity.

## Exact-generation initialization blocker closure

Review r3 identified two remaining windows. Recovery could read generation N,
then materialize its stale loaded sequence after custody had advanced to N+1;
the store rejected installation, but the stale provider invocation had already
occurred. Separately, a router session opened for generation N could advance
custody during `open`, after which initialization paired that stale session
with its final generation-N+1 receipt and accepted the session's next route as
current.

The behavior-first proof places deterministic barriers at both windows. The
custody-read barrier composes a second edit after recovery's first receipt read
and requires exactly two materializer calls for the complete two-edit sequence,
with no stale one-edit invocation. The router-open barrier composes while the
first session opens and requires that session to close, a second session to
open from the current event, and only its destination to commit. The latter
runs through memory restart, clean redb restart, and redb SIGKILL recovery.

Publication now uses one bounded generation-activation path for initialization
and later successors. It exact-refreshes durable custody, re-reads the complete
receipt, materializes only unchanged custody, opens routing for that exact
event, re-reads again, and closes/restarts on any mismatch. Route application
uses the write, receipt, materialization, and event identity accepted by that
activation; signing starts only after it succeeds.

## Validation gates

Green evidence:

- `semantic_write_store`: memory 15 passed; redb 25 passed.
- `semantic_write_failures`: 19 passed, including a persistent custody-read
  failure across a further durable composition and newer source arrival, proved
  through an observed read barrier without a timing sleep, plus recovery
  generation change between receipt and custody reads.
- `semantic_write_publication`: 23 passed, including restart reconciliation,
  immediate same-coordinate admission, and later complete replay;
  `semantic_write_contract`: 5 passed;
  `semantic_write_capabilities`: 4 passed.
- Redb `process_kill`: 12 passed, including clean-reopen and SIGKILL
  restart-then-immediate-edit barriers, complete late-source replay, and route
  session replacement when custody advances during initialization.
- Bookmark, NIP-02, simple-groups, and the external semantic-capability
  falsifier all passed.
- `cargo check --workspace --all-targets --locked` passed.
- Focused workspace and external-falsifier clippy passed with `-D warnings`.
- `python3 -m unittest tools.tests.test_vocabulary_check`: 36 passed.
- Every Rust file changed from `31a87f1a` passes `rustfmt --check`; `git diff
  --check` passes.
- After rebasing onto main closure `863eef72`, all five focused Bazel targets
  pass: `semantic_write_contract`, `semantic_write_store`,
  `semantic_write_publication`, `semantic_write_failures`, and
  `semantic_write_capabilities`.

Independent residual gates:

- Full vocabulary-checker output is byte-for-byte identical to `31a87f1a` and
  remains red on its pre-existing terminal-name/approval inventory.
- Full `cargo fmt --all -- --check` reports the same eleven pre-existing files
  as `31a87f1a`.
