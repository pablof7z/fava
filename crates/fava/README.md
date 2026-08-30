# fava

The facade re-exports the neutral `EventBuildError` and `WriteIntentError`
contracts. Appliers can return `WriteIntentError` after custody, but
current publication erases it to
`PublicationError::Routing(error.to_string())`; the typed value does not
survive that boundary. Structured attribution is owned by
[issue 0025](../../docs/issues/0025-publication-applier-error-attribution.md).

`fava.publish(builder)` accepts an `AuthoredEventBuilder` directly. For an
authorless `EventBuilder` or `EventEdit`, `fava.publish(builder)` and
`fava.to(...).publish(builder)` resolve the session's current account before
custody. They refuse with `PublishError::MissingAuthor` when no account is
selected. `fava.by(author).publish(builder)` remains the exact explicit override
and composes with `.to(...)` in either order. A later account switch never
retargets accepted work. A builder without an attached route uses automatic
routers; a builder carrying local explicit relay routing lowers its event and
route together through the same durable publication lifecycle. A facade route
expression refuses when the builder already carries a conflicting explicit
route.
