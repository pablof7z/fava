# Publication materializer error attribution

## Defect

`fava-publication` currently converts every `ReplaceableEventMaterializer` refusal into `PublicationError::Routing`, during both initial materialization and post-custody rematerialization. Builder, protocol-source, and rematerialization failures therefore lose their owning category and retained cause.

## Required outcome

Design one truthful publication-owned error boundary with distinct outcomes:

- **Initial preparation failure:** return the structured owning materializer refusal to the caller, release the temporary reservation, allocate no `WriteId`, `ReceiptId`, or `MaterializationId`, and leave zero custody, receipt, query, or publication residue.
- **Post-custody rematerialization failure:** retain bounded structured failure evidence on the existing receipt with its exact write, receipt, materialization, source, and generation identities.

Neither outcome may be called routing failure. Late post-custody completion remains isolated by exact generation identity.

Do not wrap every failure in a generic string, add compatibility aliases, or make protocol crates own publication lifecycle.

## Falsifier

An initial materialization forced to return `WriteIntentError::TooManyTags { actual: 2001, maximum: 2000 }` must return that structured caller error, release its reservation, allocate no durable identities, and leave no residue. A post-custody rematerialization returning the same refusal must not produce `PublicationError::Routing`; the retained receipt must attribute the exact existing identities and generation. A stale completion for an older generation must not alter current state.

## Sequencing

The neutral `EventBuildError -> WriteIntentError` conversion may land first. That slice must link this issue but does not claim end-to-end publication attribution is repaired.
