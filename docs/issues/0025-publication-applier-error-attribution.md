# Publication applier error attribution

## Defect

`fava-publication` currently converts every `EditApplier` refusal into `PublicationError::Routing`, during both initial edit application and post-custody reapplication. Builder, protocol-source, and reapplication failures therefore lose their owning category and retained cause.

## Required outcome

Design one truthful publication-owned error boundary with distinct outcomes:

- **Initial preparation failure:** return the structured owning applier refusal to the caller, release the temporary reservation, allocate no `WriteId`, `ReceiptId`, or `RevisionId`, and leave zero custody, receipt, query, or publication residue.
- **Post-custody reapplication failure:** retain bounded structured failure evidence on the existing receipt with its exact write, receipt, revision, source, and generation identities.

Neither outcome may be called routing failure. Late post-custody completion remains isolated by exact generation identity.

Do not wrap every failure in a generic string, add compatibility aliases, or make protocol crates own publication lifecycle.

## Falsifier

An initial edit application forced to return `WriteIntentError::TooManyTags { actual: 2001, maximum: 2000 }` must return that structured caller error, release its reservation, allocate no durable identities, and leave no residue. A post-custody reapplication returning the same refusal must not produce `PublicationError::Routing`; the retained receipt must attribute the exact existing identities and generation. A stale completion for an older generation must not alter current state.

## Sequencing

The neutral `EventBuildError -> WriteIntentError` conversion may land first. That slice must link this issue but does not claim end-to-end publication attribution is repaired.
