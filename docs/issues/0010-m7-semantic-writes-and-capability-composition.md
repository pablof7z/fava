# M7: semantic writes and capability composition

**Status:** in progress
**Branch:** `milestone/m7-semantic-writes`
**Authority:** `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, M7

## Problem

The completed M6 publication lifecycle accepts only finalized unsigned or
pre-signed events. It cannot durably retain a protocol-owned replaceable-event
edit, materialize it against current qualified source state, or replace that
materialization when newer source state arrives. Protocol crates therefore
cannot yet express edits such as follow/unfollow without applications manually
rebuilding whole events.

## Scope

- Add a neutral, bounded, persistable replaceable-event-edit value and public
  materializer contract without teaching universal owners event-kind meaning.
- Extend memory and redb write-store custody with stable operation, receipt, and
  materialization-generation identity.
- Extend publication with first-value materialization, source-driven
  rematerialization, and exact stale-generation rejection across signing,
  routing, publishing, and delivery.
- Add two unrelated protocol crates: NIP-02 follows and a bookmarks capability.
- Prove the complete behavior through the public `fava` API, a shared
  capability corpus, restart evidence, and dependency-negative checks.

## Non-goals

- Native Swift/Kotlin projection, product profiles, authentication, and M8+
  hostile-boundary expansion.
- Protocol-specific branches in `fava`, publication, query, routing, stores,
  transport, signer, publisher, or delivery owners.
- A second receipt, signing, routing, retry, transport, cache, or optimistic-row
  lifecycle owned by a protocol crate.

## Exit gates

- Every CAP-01 through CAP-09 behavior has direct falsifiable evidence.
- A first-value follow operation is accepted and visible through the ordinary
  write-store query source and receipt before relay acknowledgement.
- A newer qualified source rematerializes a live edit atomically, preserves
  unrelated source changes, keeps the same write/receipt identity, and makes
  retired completions inert but attributable.
- Follow/unfollow and bookmark/unbookmark pass one public conformance corpus.
- Adding a capability changes only that crate and selected assembly metadata;
  arbitrary/future raw kinds remain usable.
- The full proportional validation set passes and the focused issue records the
  deliberate-break evidence.

## Deliberate-break evidence

### Current-materialization identity

`DELIBERATE_BREAK_M7_STALE_COMPLETION` removed only the sole
`receipt.current.publication.materialization_id != materialization_id`
predicate from `fava-write-store::validate_current_materialization`.

- Original SHA-256: `50f73279c139469f03f01247f4e5af692e291f19cc5944fef8e189221d9fb7af`.
- Baseline discovery named exactly
  `interleavings::retired_completion_is_attributable_and_inert` and
  `first_value_edit_publishes_through_public_fava`; the complete publication
  target passed 12/12.
- While broken, the first-value tracer still passed. The exact retired test
  compiled and failed at `interleavings.rs:94`: generation-one
  `record_signer_refusal` paired with the successor event identity returned
  success instead of refusing. This is current-state mutation by a retired
  materialization, not a compile or unrelated failure.
- A scoped source edit restored the predicate. SHA-256 returned to the original
  value, the source diff was empty, and the complete publication target passed
  12/12.

Commands:

```text
cargo test -p fava --test semantic_write_publication -- --list
cargo test -p fava --test semantic_write_publication
cargo test -p fava --test semantic_write_publication first_value_edit_publishes_through_public_fava -- --exact
cargo test -p fava --test semantic_write_publication interleavings::retired_completion_is_attributable_and_inert -- --exact
shasum -a 256 crates/fava-write-store/src/lib.rs
git diff --exit-code -- crates/fava-write-store/src/lib.rs
```

DELIBERATE_BREAK_M7_STALE_COMPLETION: PASS original=50f73279c139469f03f01247f4e5af692e291f19cc5944fef8e189221d9fb7af restored=50f73279c139469f03f01247f4e5af692e291f19cc5944fef8e189221d9fb7af baseline=12/12 restored_target=12/12

### Protocol dependency direction

`DELIBERATE_BREAK_M7_PROTOCOL_DEPENDENCY` temporarily inserted only:

```rust
use fava_signer as _deliberate_break_m7_forbidden_dependency;
```

into `crates/fava-nip02/src/lib.rs`.

- Original SHA-256: `deefde7b77a75f8981c855c6dc46cae008dfeff79d5d527de56bbbda6156c0f2`.
- `cargo check -p fava-nip02 --lib` reached the protocol crate and failed with
  Rust error E0432 at the inserted line: `no external crate fava_signer`.
  This is the intended undeclared-dependency failure, not syntax or an
  unrelated target failure.
- A scoped source edit removed the one temporary import. SHA-256 returned to
  the original value, the source diff was empty, and NIP-02 passed 7 unit plus
  1 external public-API test.

DELIBERATE_BREAK_M7_PROTOCOL_DEPENDENCY: PASS original=deefde7b77a75f8981c855c6dc46cae008dfeff79d5d527de56bbbda6156c0f2 restored=deefde7b77a75f8981c855c6dc46cae008dfeff79d5d527de56bbbda6156c0f2 diagnostic=E0432 restored_target=7+1

### Exact raw event construction and bounds

Review exposed two fail-open gaps before implementation:

- Two well-formed `fava:rust=` comments before one feature scenario silently
  replaced the first pending destination with the second.
- The existing public `EventBuilder` could set raw fields one at a time, but
  did not accept all exact raw parts or ordered tags in bulk.

RED commit `e80f6f0` records both causal failures. The duplicate fixture was
accepted as one scenario with the second mapping. The Rust and independent
public-`fava` consumers failed specifically because `from_parts` and `tags`
did not exist.

The existing builder now exposes this exact construction door:

```rust
EventBuilder::from_parts(
    author: PublicKey,
    kind: Kind,
    created_at: Timestamp,
    tags: Vec<Tag>,
    content: String,
)
```

Its `tags` method accepts ordered `Tag` iterators. `new` delegates to
`from_parts`, `tag` delegates to `tags`, and only `build` validates the common
state. There is no second owner, event-parts value, wrapper, or protocol-kind
switch. Rustdoc proves the exact public method set is
`build,content,created_at,from_parts,new,tag,tags`.

The external consumer constructs kind 50001 at `created_at = 42` with the
three arbitrary tags `["something something"]`, `["x-a","poop"]`, and
`["x-future","kept","verbatim"]`. It asserts exact field order and event ID
in accepted unsigned state, query visibility, signed terminal evidence, and
published transport evidence. The canary repeats the same proof through the
public facade and records equal accepted, signed, and published IDs.

`DELIBERATE_BREAK_M7_EVENT_BUILDER_BOUND` changed only `MAX_TAGS` from 2000
to 2001. The exact hostile-bound test compiled and failed because 2001 tags
were accepted instead of returning `TooManyTags { actual: 2001, maximum:
2000 }`. A scoped edit restored SHA-256
`abaa77068de484d6b6b0cca7677414aaa263a35a0280af8288fb24533b0409e9` and
the exact test passed. The same test proves both raw-parts and bulk-tag paths
share that refusal and proves oversized serialized events still refuse.

DELIBERATE_BREAK_M7_EVENT_BUILDER_BOUND: PASS original=abaa77068de484d6b6b0cca7677414aaa263a35a0280af8288fb24533b0409e9 restored=abaa77068de484d6b6b0cca7677414aaa263a35a0280af8288fb24533b0409e9 broken=2001-tags-accepted restored_target=2/2

### Canary roster authority

The detailed M7 section requires four canaries, including
`replaceable-edit-inverse`; the global roster omits only that inverse row.
M7 follows its detailed section, and all four exact IDs are enabled, tested,
and run through the ordinary CLI.

SPEC_DISCREPANCY_M7_CANARY_ROSTER: RECORDED detailed=4 global=3 decision=detailed-M7
