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

## Executable evidence

The causal RED commit is `bb75ba2`. Before the implementation, the owner test
did not compile: `WriteIntentError` had no `TooManyTags` variant and no
`From<EventBuildError>` implementation.

With only the `TooManyTags` conversion deliberately mutated to `TooLarge`, all
of these exact assertions failed with `TooLarge { bytes: 2001, maximum: 2000 }`
instead of the required `TooManyTags { actual: 2001, maximum: 2000 }`:

- `fava-write` owner conversion;
- NIP-02, bookmarks, and simple-groups shipped materializers;
- exact, controlled, semantic-write support, and Redb restart fixtures; and
- the standalone external semantic-capability consumer.

The same owner mutation run left the byte and encoding conversion assertions
green (two passed, one failed), proving the failure is specific to tag
cardinality rather than a generic error-path assertion.

## Closure inventory

The exhaustive conversion is owned only by `fava-write`. The eight event-build
call sites that return `WriteIntentError` use it directly: NIP-02, bookmarks,
simple-groups, exact fixture, controlled fixture, semantic-write support
fixture, Redb restart fixture, and external semantic capability. NIP-02 keeps
its direct `EventBuildError -> ContactListError` decoder conversion exhaustive;
valid signed contact-list input is governed by its encoded-byte bound, not the
local write builder's tag bound.

Generated public-API inventories for `fava-write` and the `fava` facade include
both error enums, all variants, and the public exhaustive conversion. Routing
source and routing-test hashes remain respectively
`e97420b3d1400c75d1e0df6f4374fa5e0276b980abd11c59df3d305b84aba882`
and `fb18bfcb40e67fd8004becbb62f0ad4dab6bc3fa666cec1bde5cf96dbe0dbeb5`,
identical to the pre-slice baseline.

## Validation disposition

Green revision-9 gates:

- owner, exhaustive NIP-02, bookmarks, simple-groups, exact, controlled,
  semantic-write support, Redb process-kill, and external capability tests;
- external capability full all-target test and clippy suites;
- workspace clippy with warnings denied and workspace doctests;
- workspace all-target tests are green with only the two existing
  vocabulary-backlog tests explicitly skipped; one earlier run exposed an
  unrelated WebSocket close/reconnect flake, whose exact rerun and the complete
  subsequent workspace rerun were green;
- generated `fava-write` and `fava` public-API inventory checks;
- focused Bazel tests for `fava-write`, NIP-02, bookmarks, and simple-groups
  (Bazel reported them passed before its sandbox-forbidden macOS process
  inspection made the batch invocation exit 37); and
- diff whitespace, external rustfmt, code-size, conversion inventory, and
  byte-exact routing checks.

Repository-wide gates still red on unchanged `main` debt: the two vocabulary
governance tests and two corresponding vocabulary-tool unit tests fail on the
existing simple-groups candidate mismatch and wider vocabulary backlog;
`check_vocabulary.py` reports that backlog but no `EventBuildError` or
`WriteIntentError` defect. Repository-wide rustfmt reports pre-existing
simple-groups formatting drift. The Redb Bazel target is blocked by missing
first-party dependencies in `fava-observe/BUILD.bazel`, while the Cargo Redb
process-kill test is green.
