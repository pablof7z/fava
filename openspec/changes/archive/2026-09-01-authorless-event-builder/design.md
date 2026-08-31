## Context

See `proposal.md` — Why. The relevant current state:

`EventBuilder` is one struct with a non-optional `author: PublicKey` field, set by `new(author, kind)` and by `from_parts(author, ..)`. `build()` and `into_event_and_routing()` both funnel through a private `build_event()` that constructs `UnsignedEvent::new(self.author, ..)` and calls `ensure_id()`.

On the publication side, `PublishPayload::into_intent(self, author: Option<PublicKey>, routing)` already threads an optional author to every payload kind. Three of the four impls — `UnsignedEvent`, `EventBuilder`, `Event` — take it as `_author` and discard it; only `EventEdit` reads it, failing with `PublishError::MissingAuthor` when absent. `PublishAs` is the author scope and today accepts only `EventEdit`.

The mechanism this change needs therefore already exists end to end. What is missing is a builder state that can travel through it.

## Goals / Non-Goals

**Goals:**

- One rule for where an author enters a payload, applying equally to event bodies and replaceable edits.
- The absent-author state unrepresentable at finalization rather than a runtime refusal — no `MissingAuthor` variant added to `EventBuildError`.
- Byte-identical output. Same events, same ids, same routes.
- The reconstruct paths stay direct: no caller that already holds an exact author is made to route through an authorless intermediate.

**Non-Goals:**

- Any change to signing, signer attachment, or `Fava::sign`. Signer selection continues to read `event.pubkey` on a finalized unsigned event.
- Introducing a current-account or ambient-identity concept. `by(author)` stays explicit at every call site.
- Where the edit appliers live. They are in their protocol crates and stay there; this change only requires they use the authored construction path.
- Touching the decoded read-side types that carry an `author` field (`SimpleGroupMetadata`, `ContactList`, `RelayList`, and siblings). Those record the author of an observed event and are unrelated.

## Decisions

### Two types, not one type with an optional author

`EventBuilder` becomes authorless. A second type — `AuthoredEventBuilder` — carries the author and owns `build()`, `into_event_and_routing()`, and the private `build_event()`. `EventBuilder::by(author)` moves the accumulated body across.

*Why:* it is the only shape that makes "finalized without an author" impossible to write, which is the property the user selected. The body-shaping methods (`created_at`, `content`, `tags`, `tag`, `event_tags`, `to_relays`) are duplicated across both types so a caller can keep shaping after supplying an author.

*Alternative — `Option<PublicKey>` with a `MissingAuthor` build error:* one type, smallest diff, but it converts a compile-time impossibility into a runtime refusal and leaves every caller of `build()` handling an error variant that cannot occur on the authored path. Rejected on the same grounds the existing design rejects publishing an unsigned event through `by()`: the type should not permit the expression.

*Alternative — typestate generic `EventBuilder<A>`:* avoids duplicating the shaping methods, at the cost of a generic parameter in every public signature that mentions the builder, including nine `fava-simple-groups` return types. Rejected as worse to read for a two-state machine.

*Method duplication:* the shaping methods are mechanical and identical. If the duplication proves annoying, a private shared body struct that both types wrap collapses it without changing either public surface. Not doing that up front — two small impls are easier to read than an indirection.

### `from_parts` and `From<UnsignedEvent>` return the authored type

Both already take an exact author, and both exist precisely to reconstruct a specific event whose id must come out matching. Routing them through an authorless intermediate would add a `.by(event.pubkey)` hop that restates what the input already said.

This keeps every current reconstruct call site a one-line change or none at all: `fava-nip02::contact_list::validate_unsigned_bound`, the NIP-02, bookmark, and saved-group-list edit appliers all call `from_parts(..).build()` or reopen an `UnsignedEvent`, and continue to compile unchanged.

`fava-publisher-nip01`'s kind-22242 auth response is the one production `new(author, kind)` call that is genuinely authored — it builds an event it is about to sign with a known key. It becomes `EventBuilder::new(kind).tag(..).tag(..).by(pubkey).build()`.

### `PublishPayload` gains an impl; `PublishAs` widens

`impl PublishPayload for EventBuilder` (now the authorless type) reads the `author: Option<PublicKey>` argument it currently discards, and returns `PublishError::MissingAuthor` when it is `None` — the same line `EventEdit` already has. `impl PublishPayload for AuthoredEventBuilder` keeps the current body and continues to ignore the argument.

`PublishAs::publish` changes from taking `EventEdit` to taking a payload bounded by a marker for authorless payloads, implemented by `EventBuilder` and `EventEdit` only. That is what excludes `AuthoredEventBuilder`, `UnsignedEvent`, and `Event` from the author scope, extending the existing exclusion rather than inventing one.

The route-merge logic in the builder's `into_intent` — explicit-vs-explicit conflict, automatic falling back to the facade route — is identical for both builder types and is factored into one function they share.

### Protocol constructors return the authorless builder

The nine `fava-simple-groups` management constructors and the private `build` helper drop their `author` parameter and return the authorless `EventBuilder`. Their doc examples change from `create_group(author.public_key(), &group)?` to `create_group(&group)?` followed by `fava.by(author).publish(builder)?`, which is what the module's publish-path prose already describes for edits.

This is what makes `fava_simple_groups::invite(&group, code)` read the same way `fava_nip02::follow(target)` and `fava_bookmarks::bookmark_event(target)` already do.

## Risks / Trade-offs

- **A caller who wants a local `UnsignedEvent` from a protocol constructor now needs an extra `.by(author)`.** → That is the intended cost, and it is one call at the point where identity genuinely enters. `Fava::sign` still takes a finalized `UnsignedEvent`, so the sign-without-publishing path is `invite(&group, code)?.by(author).build()?` then `fava.sign(event)`.

- **Method duplication across the two builder types drifts.** → The shaping methods are pure field assignment with no branching; a private shared body struct is available if drift appears. Flagged here so a later reader knows the collapse is deliberate to defer, not overlooked.

- **`AuthoredEventBuilder` is a new public name in `fava-write`'s surface, re-exported by `fava`.** → Accepted. Naming the authored state is the point; hiding it behind the same name as the authorless one is what produced the current confusion.

- **The change touches `fava-write`, `fava`, `fava-simple-groups`, `fava-nip02`, and `fava-publisher-nip01` in one commit.** → The compiler locates every call site, and no behavior is in flight during the change: it is a signature migration with byte-identical output. There is no partial-rollout state to be in.

## Migration Plan

No runtime migration. No persisted data, wire format, event id, or relay-visible behavior changes — an event constructed after this change serializes identically to one constructed before it.

This is a source-level breaking change to `fava-write`, `fava`, and `fava-simple-groups`. Per `AGENTS.md` the project has no public consumers and takes API breaks directly, with no compatibility path, deprecation, or alias. Changed public declarations drop their Symbol Gate signatures and are re-signed as part of the change.

Rollback is a revert.
