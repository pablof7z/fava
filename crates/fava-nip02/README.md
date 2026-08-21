# fava-nip02

NIP-02 contact lists for Fava, as plain values you hand to `Fava`. A `ContactList` is one author's kind-3 follows — each a pubkey with an optional relay hint and petname; the crate mints ordinary `Query` reads and `ReplaceableEventEdit` writes, and parses kind 3 into a typed value. No socket, store, signer, or observation lifecycle of its own.

Every example follows one person, Alice.

## Following someone

You tap follow on Alice's profile.

```rust
use fava_nip02::follow;

let edit = follow(alice)?;
let accepted = fava.publish(edit)?;
```

Your following list updates at once — you don't wait for a relay, and you don't need a connection. Unfollow is the same call the other way, and it's the exact inverse:

```rust
use fava_nip02::unfollow;

let edit = unfollow(alice)?;
assert_eq!(edit, follow(alice)?.inverse());
```

## Following offline

You follow Alice on the subway, with no signal. Your following list updates on screen at once.

Meanwhile your laptop has been online and published a newer list — you added four people there and renamed Alice's petname from `ali` to `alice`. It's already on your relays when your phone reconnects.

The phone doesn't overwrite it. The follow you made offline applies on top of the newer list: Alice is in, the four people are still in, and her petname `alice` is kept. You see one correct list. Nothing in the app had to choose, merge, or retry.

```rust
let edit = follow(alice)?;
fava.publish(edit)?;
// made offline; a newer kind-3 is waiting when you reconnect — the follow lands on top.
```

## Who Alice follows

The app wants to render Alice's following list. Kind 3 is replaceable and per-author, so the newest record at that coordinate is the whole answer — and the crate parses it.

```rust
use fava_nip02::{contact_list, ContactList};

let obs  = fava.observe(contact_list(alice))?;
let list = ContactList::from_event(&obs.current().events().next().unwrap().event)?;
for f in list.follows() {
    println!("{}  {}  {}", f.pubkey(), f.relay().unwrap_or(""), f.petname().unwrap_or(""));
}
```

No tag walking in the app. `ContactList::from_event` decodes kind 3 once; `follows()` yields typed `Follow` values — `.pubkey()`, `.relay()`, `.petname()`. The write store's own materialization shows up through the same query the moment `publish` returns, before any relay confirms.

## Discovery

Discovery is how the app finds people through the people you already follow — the follows of your follows, or who follows you back — so it can suggest people to follow.

### Follows of follows

The app wants a "people you may know" row: everyone followed by the people Alice follows.

```rust
use fava_nip02::{contact_list, follows_of};

let first  = fava.observe(contact_list(alice))?;
let first_hop: Vec<PublicKey> = follows_of(&first.current());   // typed parse, no tag walk

let second = fava.observe(contact_list(&first_hop))?;
let second_hop: Vec<PublicKey> = follows_of(&second.current());
```

Two observations, because a `Query` takes concrete keys. The crate parses each list; the app composes the queries.

### Who follows Alice back

You can even ask, very simply: who out there follows Alice?

```rust
use fava_nip02::{followers_of, ContactList};

let obs = fava.observe(followers_of(alice))?;     // kind 3, #p = alice
for ev in obs.current().events() {
    let their_list = ContactList::from_event(&ev.event)?;
    println!("{} follows alice", their_list.author());
}
```

This needs the generic `#p` tag axis on `Query` — the same `fava-query` slice `fava-simple-groups` needs for `#h`/`#d`. Until it lands, the followers direction is unrepresentable (see Prerequisites).

## API

### Writes — `follow` / `unfollow` / `follow_with` (return `ReplaceableEventEdit`)
- `follow(target) -> Result<ReplaceableEventEdit, WriteIntentError>` — add a `["p", hex]` row. The actor is the configured signer, bound at publish (D7); `target` takes `impl TryInto<PublicKey>` so a hex `&str` works directly (target: fold the parse error into `WriteIntentError::InvalidEvent`).
- `unfollow(target)` — same signature, the exact inverse edit.
- `follow_with(target, relay: Option<RelayUrl>, petname: Option<&str>)` — attach a relay hint and petname (codec format 2); `follow` stays the common case.
- `materializer() -> Arc<dyn ReplaceableEventMaterializer>` — kind 3; the publication owner calls `materialize`, apps don't.

### Reads — `ContactList` (pure, `from_event`; the crate parses, apps don't)
- `ContactList::from_event(&EventValue) -> Result<Self, ContactListError>` — decode kind 3 once.
- `list.author() -> PublicKey`, `list.follows() -> &[Follow]`, `list.supersedes(&other) -> bool`.
- `Follow::pubkey()`, `.relay() -> Option<&RelayUrl>`, `.petname() -> Option<&str>`.

### Discovery builders (return ordinary `Query`, or a pure projection)
- `contact_list(author) -> Query` — kind 3, `authors = [author]`, newest-first, `limit(1)`. Takes one author or an iterable.
- `follows_of(&QuerySnapshot) -> Vec<PublicKey>` — pure projection over parsed lists; no reactive `ValueSet` (a separate `fava-query` project).
- `followers_of(subject) -> Query` — kind 3, `#p = subject`. **Needs the generic `fava-query` tag axis** (see Prerequisites).

### What you get back (from `fava-write`)
`edit.actor()` (the bound signer, after publish binds it), `edit.coordinate()` (`Replaceable { author, kind: 3, identifier: None }`), `edit.format()` (`1`, or `2` for `follow_with`), `edit.change()`, `edit.inverse_change()`, `edit.inverse()`. `WriteIntent::edit(edit, routing) -> Result<WriteIntent, _>`; `Fava::publish(impl Into<WriteIntent>) -> Result<AcceptedWrite, PublicationError>` — an edit publishes directly: `fava.publish(edit)?` (default `WriteRouting::Automatic`); explicit routing via `fava.publish(WriteIntent::edit(edit, WriteRouting::Explicit(..))?)?` (D8).

## Design notes

- **Edits, not events.** A follow is a durable `ReplaceableEventEdit` that survives the arrival of newer source state. Read-modify-write of a kind 3 loses concurrent changes made elsewhere — this is the whole reason the crate exists.
- **The actor is the signer.** A kind 3 is the signer's own list, so `follow(target)` doesn't take an actor — the configured signer is the author, bound at publish. An actor ≠ signer is unrepresentable, not a runtime error.
- **Publish takes the edit.** `fava.publish(edit)` wraps the edit into a write intent with default `WriteRouting::Automatic` (the author's write relays). Explicit routing is the override: `fava.publish(WriteIntent::edit(edit, WriteRouting::Explicit(..))?)?`.
- **Rebase is deterministic and re-runnable.** `materialize` is pure: edit plus optional source plus a caller-supplied timestamp in, one `UnsignedEvent` out. The same accepted edit applied to a newer source produces a successor materialization under the same receipt.
- **Add preserves, not replaces.** Following someone already listed keeps the first existing `p` tag verbatim — relay hint and petname intact — and drops duplicates. Unfollow removes every matching `p` tag. Non-`p` tags and `content` pass through unchanged, so a legacy kind-3 relay-JSON blob survives an edit by a client that doesn't understand it.
- **The crate parses; apps don't.** `ContactList::from_event` decodes kind 3; apps read typed `Follow` fields, never raw tags.
- **Bounded.** Output tags capped at 2000; source size-checked against a 131 072-byte event bound before any work. Overruns are `WriteIntentError::TooLarge`, never truncation.
- **Sources qualified before use.** The source's signature is verified, author and kind must match the edit's coordinate, `created_at` must strictly succeed the source's. Failure is `WriteIntentError::InvalidEvent`, not a silent fallback to empty.
- **The crate never picks a timestamp.** `materialize` receives `created_at` from the caller; time is the engine's authority.
- **Engine-free.** Depends only on `fava-state` + `fava-write` (the `fava-nip65` idiom). The `contact_list` / `followers_of` builders add a `fava-query` dep — the same one `fava-simple-groups` needs.
- **Vocabulary, approved (D9).** `ContactList` is reserved in `vocabulary.toml`; exporting it with `from_event`, `Follow`, `ContactListError` was a vocabulary change — approved. `vocabulary.toml` gains the symbols owned by `fava-nip02`; `tools/check_vocabulary.py` must pass.

## Prerequisites

- A **generic tag axis on `Query`** (`#p` for `followers_of`; `#h`/`#d` for `fava-simple-groups`) lands in `fava-query` first. `contact_list` and `follows_of` work without it; `followers_of` does not.
- The materializer must be registered: `Fava::builder().materializers([fava_nip02::materializer()]).build()?`.
- Publication configured; `WriteRouting::Automatic` needs a router.

## Status

### Delivered in M7 — local `main`

M7 is complete locally. Merge `caeee9e` brought the milestone branch into `main`; the verified implementation head is `f97ecd8`. The deliberately narrow public `fava-nip02` surface is `follow(target: PublicKey)`, `unfollow(target: PublicKey)`, and `materializer()`.

The delivered slice includes the shared semantic-edit infrastructure plus NIP-02 materialization from empty state and qualified newer source state, deterministic rebasing, source refusals, idempotent add, duplicate removal, preservation of content and foreign tags, and tag/byte bounds. The canonical edit shape is `{ kind, identifier, change }`; the accepted write owns the resolved author. `cargo test -p fava-nip02 --all-targets` passes seven unit tests and the public-API test.

This is local delivery, not remote delivery: local `main` is ahead of `origin/main`.

### Broader target — not yet built

The README above describes the intended crate beyond the M7 tracer. Still unimplemented: `ContactList::from_event`, `Follow`, `ContactListError`, `contact_list`, `follows_of`, `followers_of`, `follow_with`, string-literal public-key conversion, and the direct ergonomic `fava.publish(edit)` door with its explicit-routing form.

The generic `Query` tag axis remains a prerequisite only for `followers_of` (`#p`); `contact_list` and pure `follows_of` can land without it. D1–D9 remain the design record for the broader surface; M7 completion does not claim those later API additions.
