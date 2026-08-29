# Edit author at the taker, not the edit

**Status:** accepted and completed
**Implementation:** `495ca42` (`ReplaceableEventEdit` final shape), `8239393`
(acceptance and recovery freeze the author), `ee38b6d` (application `by` scope)
**Authority:** WRITE-002, WRITE-003, WRITE-006; `docs/spec/ARCHITECTURE.md`
publication and write-store ownership
**Related:** `0014` owns the application publication door; `0015` owns its
scope-handle nouns.

## Adopted result

`ReplaceableEventEdit` carries the replaceable kind, optional identifier, and
opaque protocol change. It carries no author, inverse, or format field. The
application supplies the author to the publication taker:

```rust
let edit = fava_nip02::follow(bob)?;
let write = fava.by(alice).publish(edit)?;
```

The facade constructs the neutral internal form with
`WriteIntent::edit_as(edit, alice, routing)`. Acceptance persists Alice beside
the edit. Every initial or successor materialization uses that persisted key;
recovery never consults the current session to derive another author.

An unscoped edit call is a typed pre-custody refusal:

```rust
let error = fava.publish(edit).unwrap_err();
assert!(matches!(error, fava::PublishError::MissingAuthor));
```

Unsigned and pre-signed events already carry their author and cannot pass
through `PublishAs::publish`.

## Ownership

- `fava-nip02` owns the semantic change and pure materializer.
- `fava` owns the inert application author scope.
- `fava-publication` resolves the scope once and orders acceptance.
- `WriteStore` owns the accepted author, edit, materialization generations, and
  recovery facts.
- The signer signs the exact event produced for the persisted author; it does
  not choose that author.

This keeps protocol crates engine-free and preserves addressable edits: the
identifier stays in the edit because a kind-30023 edit must still name which
article changes.

## Executable evidence

```sh
cargo test -p fava --test publication_door
cargo test -p fava --test publication_scopes
cargo test -p fava --test semantic_write_publication author::
cargo test -p fava-write --test replaceable_edit
python3 -m unittest tools.tests.test_vocabulary_check
```

`author::accepted_author_scopes_sources_signing_and_every_generation` proves
source selection, signing, and successor generations stay on the accepted
author. `author::recovery_uses_persisted_author_when_only_bob_signer_is_selected`
reopens Alice's accepted edit with only Bob's signer selected and proves the
write remains Alice's. The latter is the executable falsifier: replacing the
persisted author with session-derived Bob makes the named recovery assertion
fail.

## Decision rationale

The author is mutable application/session context before acceptance and a
durable fact after acceptance. Putting it in both the edit and accepted write
would create two authorities and a contradiction check. Keeping only the
accepted owner makes account switches, rematerialization, and restart exact.
