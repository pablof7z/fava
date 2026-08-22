# fava-nip02

Typed NIP-02 contact-list reads and lossless follow edits for Fava. The crate
returns ordinary `Query` and `ReplaceableEventEdit` values; `Fava` owns
observation and publication.

## Follow and unfollow

A follow edit names the person being followed. The application selects whose
kind-3 list is being edited with `by(...)`, then publishes through Fava's one
durable publication door.

```rust
use fava::all;
use fava_nip02::{follow, unfollow};

let write = fava.by(me).publish(follow(alice)?)?;
let receipt = write.settled(all()).await?;

let write = fava.by(me).publish(unfollow(alice)?)?;
let receipt = write.settled(all()).await?;
```

`publish` returns a `Write` after local acceptance. It does not wait for relay
delivery. The accepted materialization is immediately visible through ordinary
queries, including while offline.

Add an optional relay hint and exact petname with `follow_with`:

```rust
use fava::{all, RelayUrl};
use fava_nip02::follow_with;

let relay = RelayUrl::parse("wss://relay.example")?;
let edit = follow_with(alice, Some(relay.clone()), Some("alíce"))?;
let write = fava.by(me).to([relay])?.publish(edit)?;
let receipt = write.settled(all()).await?;
```

`by(...)` and `to(...)` are inert scopes. Either order is valid:

```rust
let first = fava.by(me).to([relay.clone()])?.publish(follow(alice)?)?;
let second = fava.to([relay])?.by(me).publish(follow(bob)?)?;
```

Unsigned and pre-signed events already carry their author, so `by(...)` accepts
only a `ReplaceableEventEdit`.

## Read a contact list

`contact_list` accepts one author or a finite collection. It creates a kind-3
query with a concrete author axis and no global result limit. The ordinary
query evaluator selects the newest replaceable event independently for each
author.

```rust
use fava_nip02::{ContactList, contact_list};

let observation = fava.observe(contact_list(alice)).await?;
let snapshot = observation.current();

for record in &snapshot.events {
    let list = ContactList::from_event(&record.event)?;
    for followed in list.follows() {
        println!(
            "{} {:?} {:?}",
            followed.pubkey(),
            followed.relay(),
            followed.petname(),
        );
    }
}
```

An empty kind-3 list is valid. `follows()` returns valid first-occurrence `p`
rows in source order. Each `Follow` exposes its source index, public key,
optional valid relay hint, and optional petname. Petnames preserve their UTF-8
bytes without normalization and distinguish an absent petname from a present
empty one.

## Row evidence

Parsing accounts for every `p` row. Rows that cannot safely become `Follow`
remain exact typed `ContactListRowEvidence` with their source index and raw
columns:

- `MissingTarget`
- `InvalidPublicKey`
- `InvalidRelayHint`
- `DuplicateTarget`
- `UninterpretedExtraColumns`

Invalid pubkeys and relay hints do not invalidate an otherwise valid kind-3
event. Wrong-kind, unfinalized, unverifiable, or over-bound events return
`ContactListError` before row decoding.

```rust
let list = ContactList::from_event(&record.event)?;
for row in list.evidence() {
    println!("row {}: {:?}", row.source_index(), row.raw_row());
}
```

## Discovery

Project followed keys from the current snapshot, then use them as the concrete
author axis of the next ordinary query:

```rust
use fava_nip02::{contact_list, follows_of};

let first = fava.observe(contact_list(alice)).await?;
let first_hop = follows_of(first.current().as_ref());

let second = fava.observe(contact_list(first_hop.as_slice())).await?;
let second_hop = follows_of(second.current().as_ref());
```

`follows_of` is a bounded pure projection. It opens no observation and owns no
mutable state. An empty author collection remains a present-empty author axis,
so it matches nothing instead of broadening to every kind-3 event.

Ask who follows a subject with the exact lowercase `p` tag axis:

```rust
use fava_nip02::{ContactList, followers_of};

let observation = fava.observe(followers_of(alice)).await?;
let snapshot = observation.current();

for record in &snapshot.events {
    let list = ContactList::from_event(&record.event)?;
    println!("{} follows Alice", list.author());
}
```

`followers_of` is still an ordinary `Query`; it makes no relay-global
completeness claim.

## Lossless shared-document edits

Kind 3 is shared with clients and extensions Fava may not understand. Every
materialization therefore begins from the newest qualified source and changes
only rows whose lowercase `p` target parses as the requested public key.

- `follow` keeps the first matching target row byte-for-byte and removes later
  matching duplicates. If no matching row exists, it appends one canonical
  row.
- `follow_with` uses its relay hint and petname only when it appends a missing
  target. An existing first row remains authoritative.
- `unfollow` removes every matching target row.
- Event content and every non-target row retain their bytes and order. This
  includes unknown rows such as `["something-something"]`, extension rows such
  as `["t", "nostr"]`, malformed unrelated `p` rows, extra columns, and
  unrelated valid follows.

The accepted author and receipt stay fixed while the publication owner may
rematerialize the edit over newer qualified source state. Stale signing or
delivery work from an older materialization cannot advance the current one.

## API surface

```text
follow(target) -> Result<ReplaceableEventEdit, WriteIntentError>
unfollow(target) -> Result<ReplaceableEventEdit, WriteIntentError>
follow_with(target, Option<RelayUrl>, Option<&str>)
    -> Result<ReplaceableEventEdit, WriteIntentError>
materializer() -> Arc<dyn ReplaceableEventMaterializer>

contact_list(authors) -> Query
followers_of(subject) -> Query
follows_of(&QuerySnapshot) -> Vec<PublicKey>

ContactList::from_event(&EventValue) -> Result<ContactList, ContactListError>
ContactList::{author,follows,evidence,supersedes}
Follow::{source_index,pubkey,relay,petname}
ContactListRowEvidence::{source_index,raw_row}
```

Targets accept `PublicKey`, hex strings, and owned hex strings. Invalid input is
a typed refusal without echoing the raw value. Parsing and edits enforce event,
tag-count, and byte bounds; over-bound input is refused, never truncated.

## Executable evidence

The README surface is exercised by:

- `crates/fava-nip02/tests/public_api.rs` for every exported NIP-02 function and
  type used above;
- `crates/fava-nip02/src/tests/contact_list.rs` for empty lists, ordered typed
  rows, complete row evidence, exact UTF-8 petnames, and event-level refusal;
- `crates/fava-nip02/src/tests/query.rs` for one/many/empty author axes,
  per-author newest selection, exact follower queries, and two-hop projection;
- `crates/fava-nip02/src/tests/edit.rs` for metadata encoding, rebasing,
  foreign-tag/content preservation, bounds, and redacted errors;
- `crates/fava/tests/publication_scopes.rs` and
  `crates/fava/tests/semantic_write_publication.rs` for `by`/`to` composition,
  synchronous acceptance, immediate query visibility, and `Write::settled`.

Run the focused contract:

```sh
cargo test -p fava-nip02 --doc
cargo test -p fava-nip02 --test public_api
cargo test -p fava-nip02 --all-targets
cargo test -p fava --test publication_scopes
cargo test -p fava --test semantic_write_publication
```
