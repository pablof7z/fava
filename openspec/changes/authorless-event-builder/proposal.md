## Why

`EventBuilder::new(author, kind)` pins an author at construction, so every function that merely *describes* an event has to accept and forward an identity it does not own. `ReplaceableEventEdit` already does the opposite: the edit constructors state no author, and `fava.by(author).publish(edit)` supplies it at publish time. The result is that two protocol crates give opposite answers to the same question — `fava_nip02::follow(target)` and `fava_bookmarks::bookmark_event(target)` take no author, while the nine `fava_simple_groups::management` constructors take one — for no reason other than which payload type they return.

Nothing about an event body requires the author early. It is needed at exactly two moments: computing the deterministic id in `build()`, and selecting the signer in `Fava::sign`. Both are finalization, not construction.

## What Changes

- **BREAKING** `EventBuilder::new(kind)` loses its author parameter and becomes authorless. An authorless builder describes an event body and has no `build()`.
- **BREAKING** New `EventBuilder::by(author) -> AuthoredEventBuilder`. Only an authored builder can `build()` or `into_event_and_routing()`. Authorless-and-built becomes unrepresentable rather than a runtime refusal.
- `EventBuilder::from_parts(author, ..)` and `From<UnsignedEvent>` keep their author and now yield `AuthoredEventBuilder` directly — these are the reconstruct paths, where the author must be exact because the resulting id has to match a specific event.
- **BREAKING** `Fava::publish` accepts an authored builder; `fava.by(author).publish(builder)` accepts an authorless one, exactly as it already does for `ReplaceableEventEdit`. Publishing an authorless builder without an author scope is refused with the existing `PublishError::MissingAuthor`. Publishing an authored builder through `by()` is excluded, on the same grounds that already exclude `UnsignedEvent` and `Event` — the payload carries its own author.
- **BREAKING** The nine `fava-simple-groups` management constructors (`create_group`, `edit_metadata`, `invite`, `join_request`, `put_user`, `remove_user`, `delete_event`, `delete_group`, `leave_group`) drop their `author: PublicKey` first parameter, along with the private `management::build` helper's.
- Reconstruct and sign-now call sites keep an explicit author: the replaceable-event materializers, `fava-nip02::contact_list`'s bound validation, and `fava-publisher-nip01`'s kind-22242 auth response.

## Capabilities

### New Capabilities
- `write/event-construction`: how an unsigned event body is assembled, when an author becomes part of it, and which builder states can produce a deterministic event id.
- `publication/author-scope`: how a publication expression supplies an author to a payload that does not carry one, and which payload kinds accept or refuse an author scope.

### Modified Capabilities

None. There are no existing specs under `openspec/specs/`; this change introduces the first two.

## Impact

- `crates/fava-write/src/builder.rs` — the `EventBuilder` type splits into an authorless builder and an authored one; `build_event`, `build`, and `into_event_and_routing` move to the authored type.
- `crates/fava/src/publication.rs` — `PublishPayload` gains an authorless-builder impl that reads the facade author, and `PublishAs::publish` widens beyond `ReplaceableEventEdit`.
- `crates/fava-simple-groups/src/management.rs` — nine public signatures lose a parameter; the module's doc examples and publish-path prose change with them.
- `crates/fava-nip02/src/contact_list.rs` and `crates/fava-publisher-nip01/src/lib.rs` — unchanged in behavior, but now explicitly on the authored path.
- The replaceable-event materializers for NIP-02, bookmarks, and the kind-10009 saved-group list are also on the authored path. Their host crate is unsettled: `fava-builtin-codecs` is to be deleted, so this change names no crate for them and applies wherever they land.
- Public API surface of `fava-write`, `fava`, and `fava-simple-groups` changes; the affected declarations need re-signing under Symbol Gate.
- No wire-format, event-id, or relay-visible behavior changes. Every event this produces is byte-identical to what it produces today.
