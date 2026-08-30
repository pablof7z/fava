# fava-write

`EventBuilder` carries generic Nostr fields plus optional bounded local relay
routing, and has no author: it cannot produce an unsigned event or an event
id. Calling `EventBuilder::by(author)` supplies the author and yields an
`AuthoredEventBuilder`, which carries every accumulated field plus the author
and owns the finalization boundary. `AuthoredEventBuilder`'s event-only
`build()` refuses an attached route; `into_event_and_routing()` is the
neutral consuming boundary used by the Fava publication door.
`EventBuildError` is the checked construction boundary for one generic
`UnsignedEvent`. Its exhaustive conversion to `WriteIntentError` preserves
tag cardinality, serialized-byte overflow, encoding failure, and
route-preservation refusal without assigning publication lifecycle meaning.
`WriteIntentError` is returned both before custody and by appliers applying
edits initially or after custody.
`AuthoredEventBuilder::from(unsigned)` consumes a finalized unsigned body,
preserves its author, kind, timestamp, ordered tags, and content, discards
its derived id, and starts with automatic routing; its final build boundary
computes the replacement id and reapplies generic bounds. When every field
including the author is already known up front, `AuthoredEventBuilder::from_parts`
constructs the authored form directly.
Current publication erases an applier's typed value to
`PublicationError::Routing(error.to_string())`; it does not survive that
boundary. Structured attribution is the separate work tracked by
[issue 0025](../../docs/issues/0025-publication-applier-error-attribution.md).
