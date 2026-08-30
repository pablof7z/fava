# 0055 — Resolve current account before write custody

**Status:** implemented; focused gates pass
**Authority:** ID-002, ID-003, WRITE-003
**Parent:** `0054-current-account-reactive-root.md`

## Defect

`Fava::publish` and `PublishTo::publish` always passed no author while lowering an
authorless `EventBuilder` or `EventEdit`. A session current account therefore had
no effect and apps still had to thread `.by(pubkey)` through every write.

## Decision

The existing publication doors snapshot `Session::current_account()` during the
synchronous call and pass it to the existing payload-to-`WriteIntent` lowering.
Authored builders, unsigned events, and signed events ignore that value because
they already carry authorship. `Fava::by` remains the explicit override.

No current account still produces `PublishError::MissingAuthor` before the
publication owner or write store receives work. Once lowering creates the
ordinary intent, accepted event authorship, signing, routing, and receipt state
follow the existing lifecycle and cannot be retargeted by later session changes.
No new scope, author field, receipt path, or persisted state is introduced.

## Evidence

- Red: `accepted_author_does_not_follow_current_account` failed with
  `MissingAuthor` after Alice had been selected.
- Green: current-account publication tests pass for no-current pre-custody
  refusal, routed Alice acceptance, switch to Bob, and later Bob acceptance.
- Regression: publication-door, publication-scope, and eight runtime-signer
  tests pass.
- Mutations: replacing current-account resolution with `None` independently at
  `Fava::publish` and `PublishTo::publish` makes the public test fail at the
  corresponding door.
- Strict focused Clippy, rustfmt, OpenSpec validation, and diff checks pass.
