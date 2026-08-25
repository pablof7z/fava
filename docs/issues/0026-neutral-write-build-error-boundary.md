# Neutral write-build error boundary

## Defect

`EventBuilder` distinguishes tag cardinality, serialized event bytes, and
encoding failures, but shipped and test materializers independently erase
`EventBuildError::TooManyTags` into either byte overflow or invalid-event text.
NIP-02 and bookmarks also duplicate the generic 2,000-tag construction bound
before calling the neutral builder.

## Required outcome

`fava-write` owns one exhaustive, field-preserving
`EventBuildError -> WriteIntentError` conversion. Every shipped materializer,
the internal fixtures, the Redb restart fixture, and the standalone external
capability use it. A materializer that asks `EventBuilder` to build 2,001 tags
returns exactly `WriteIntentError::TooManyTags { actual: 2001, maximum: 2000 }`.

Remove protocol-local copies of the generic construction cap and conversion.
Do not change routing behavior or import bookmark/simple-group domain changes.

## Boundary

This issue owns only the neutral refusal algebra and its callers. Publication's
initial-versus-post-custody attribution remains separately open in
[issue 0025](0025-publication-materializer-error-attribution.md). This slice
must not change `fava-publication`, receipt persistence, or routing behavior.

## Falsifier

Deliberately map only `EventBuildError::TooManyTags` to
`WriteIntentError::TooLarge`, then run the owner, three shipped materializer,
four internal fixture including Redb restart, and external capability tests.
Every exact tag-refusal assertion must fail while byte and encoding conversion
assertions remain green.
