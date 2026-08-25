# 0027 — Vocabulary approvals are not bound to Rust structure

**Status:** implemented, awaiting independent review
**Raised:** 2026-08-25, by Pablo

## Problem

Kind-9999 approval content currently binds only the hand-written vocabulary
record. A term can keep that text while its Rust declaration, exported path,
re-export, field, variant, method, signature, or approved private state changes.
The old signature then remains authoritative for structure the signer never
reviewed.

The approval page also offers a bulk signing path and renders a structured
interpretation of the content rather than the exact bytes submitted to the
signer. Neither behavior proves an explicit per-term decision over a visible
payload.

## Resolution

One pinned compiler-derived snapshot owns the Rust structure attached to every
registry and reviewable candidate term. Its per-term records contain:

- every exact compiler-rendered public declaration rooted at that term,
  including fields, variants, methods, and signatures;
- every exact public re-export path and its source path; and
- every non-public nominal declaration whose exact name and owning crate are
  classified by that term, including its source path and declaration body.

The canonical kind-9999 content appends the deterministic JSON form of that
single term's structural record. An explicit empty record is signed for a term
with no implemented Rust structure, so later implementation also invalidates
the prior authority. A term structure above 192 KiB is refused rather than
truncated or made impossible to submit through the bounded approval endpoint.

`tools/vocabulary_structure.py` recompiles the snapshot with the pinned nightly
rustdoc and `cargo-public-api`. CI rejects any committed snapshot that differs
from fresh compiler output. Approval startup performs the same check, and the
POST boundary refuses input-file drift after startup. The approval page shows
the exact event content in a literal block and exposes only one per-term signing
action; no bulk signing path remains.

`docs/internals/approvals.jsonl` remains append-only. Introducing structural
content intentionally makes existing text-only events stale without rewriting
or deleting them. A new signature appends beside all earlier events.

## Proof contract

- Changing a bound field, variant, method signature, exported path, re-export,
  or classified private declaration makes snapshot check fail.
- Refreshing the snapshot after that change makes the old event content fail
  exact canonical-payload matching.
- An unrelated implementation-body change requires snapshot recompilation but
  does not change a term's structural payload.
- Python and Rust render the same structural approval payload.
- The page contains no multi-term signing control and displays exactly
  `term.markdown` before the per-term signing button.
- Replaying one event is idempotent; signing changed structure appends without
  mutating historical lines.

## Validation

- `python3 -m unittest tools.tests.test_vocabulary_structure` — 9 passed.
- Focused approval payload, history, presentation, classification, and research
  tests excluding the independently red candidate-coverage class — 57 passed.
- `python3 -m unittest tools.tests.test_vocabulary_check` — 36 passed.
- Rust vocabulary governance excluding the two independently red repository
  gates — 8 passed.
- Fresh compiler snapshot generation and byte-for-byte `check` — passed.
- All 22 existing historical events load unchanged; zero match the new
  structurally bound current payload, as required.

The combined SimpleGroup tree closes the earlier 23 candidate-research
mismatches. The all-terms-approved repository gate remains red because owner
signatures are external; this issue does not approve or hide that backlog.
