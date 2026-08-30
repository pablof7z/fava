# fava

The facade re-exports the neutral `EventBuildError` and `WriteIntentError`
contracts. Appliers can return `WriteIntentError` after custody, but
current publication erases it to
`PublicationError::Routing(error.to_string())`; the typed value does not
survive that boundary. Structured attribution is owned by
[issue 0025](../../docs/issues/0025-publication-applier-error-attribution.md).

`fava.publish(builder)` accepts an `AuthoredEventBuilder` directly. An
authorless `EventBuilder` has no author of its own: `fava.publish(builder)`
and `fava.to(...).publish(builder)` refuse it with `PublishError::MissingAuthor`,
and it must instead go through `fava.by(author).publish(builder)` (composable
with `.to(...)` in either order) to supply an author. A builder without an
attached route uses automatic routers; a builder carrying local explicit
relay routing lowers its event and route together through the same durable
publication lifecycle. A facade route expression refuses when the builder
already carries a conflicting explicit route.
