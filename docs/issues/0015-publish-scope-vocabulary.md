# Publish scope handles: `PublishAs` and `PublishTo`

**Status:** accepted and completed
**Implementation:** `ee38b6d` (scope handles), `858b693` (ordered normalized
route), `abb7d6e` (durable ordered-route validation)
**Authority:** architectural vocabulary rules; WRITE-003 and WRITE-011
**Companion:** `0014` consumes both nouns at the application publication door.

## Adopted nouns

```rust
#[must_use = "a signer scope is inert until publish is called"]
pub struct PublishAs<'a> { /* private */ }

#[must_use = "a relay scope is inert until publish is called"]
pub struct PublishTo<'a> { /* private */ }
```

Both are owned by the `fava` facade, borrow one `Fava`, and last for one
publication expression. Neither is persisted, serialized, recovered, or passed
to providers. Dropping either produces no write, receipt, route session,
signer call, or transport work.

## `PublishAs`

- **Closest existing concept:** `WriteIntent`.
- **Observable distinction:** `PublishAs` has no payload and no custody. It
  selects one exact author before an edit is supplied and accepts only
  `EventEdit`.
- **Counterexample:** `fava.by(alice)` dropped without `publish` leaves the
  write store unchanged; an accepted `WriteIntent` has durable custody.
- **Owner and lifecycle:** created by `Fava::by` or `PublishTo::by`, consumed by
  `PublishAs::publish` or `PublishAs::to`, otherwise dropped inertly.
- **Forcing requirement:** WRITE-003 forbids a parallel author for an unsigned
  or pre-signed event, so the Rust surface must exclude those payloads.
- **Why existing state is insufficient:** `WriteIntent` already has a payload;
  `WriteRouting` has no author; neither represents an inert pre-payload author
  scope.
- **Executable falsifier:** the `PublishAs` compile-fail doctests fail if
  signer-scoped publication accepts `UnsignedEvent` or `Event`.

## `PublishTo`

- **Closest existing concept:** `WriteRouting::Explicit`.
- **Observable distinction:** `PublishTo` is an inert pre-payload route scope;
  `WriteRouting` is the accepted durable routing fact visible in a receipt.
- **Counterexample:** `fava.to([relay])?` dropped without `publish` contacts no
  relay and leaves no receipt.
- **Owner and lifecycle:** created and validated by `Fava::to`, consumed by
  `PublishTo::publish` or `PublishTo::by`, otherwise dropped inertly.
- **Forcing requirement:** WRITE-011 makes automatic routing ordinary while
  preserving exact explicit routing without an options bag on every call.
- **Why existing state is insufficient:** passing `WriteRouting` to every
  `publish` would expose neutral custody vocabulary and make the ordinary call
  carry routing syntax.
- **Executable falsifier:**
  `publication_scopes_are_inert_before_valid_payload` requires zero custody and
  provider effects after dropped or invalid scopes.

## Ordered route contract

`to([r1, r2, r1])` normalizes to `[r1, r2]`: relay identity is deduplicated and
first-occurrence order is retained. Empty and over-bound inputs refuse before
custody. Redb recovery accepts only the same normalized non-empty shape; it
does not reconstruct an unordered set.

## Executable evidence

```sh
cargo test -p fava --test publication_scopes
cargo test -p fava --doc
cargo test -p fava-write routing
cargo test -p fava-write-store-redb semantic_write_store::recovery
python3 -m unittest tools.tests.test_vocabulary_check
```

The route-validation deliberate break moved validation after acceptance and
made `publication_scopes_are_inert_before_valid_payload` observe one custody
row. Restoring pre-custody validation returned the test green. The vocabulary
registry contains both terms with their exact owner, lifecycle, distinction,
counterexample, forcing requirement, reason, and executable falsifier.
