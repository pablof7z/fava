# fava-simple-groups

NIP-29 groups for Fava, as plain values you hand to `Fava`. A `Group` is one opaque id over a non-empty set of host relays; it mints ordinary `Query` reads and `WriteIntent` writes — no socket, store, signer, or observation lifecycle of its own. The crate also builds discovery queries and parses every NIP-29 record into a typed value.

## The group's feed

Here the app wants to show the user a chat feed from their photography group on `wss://groups.com`.

```rust
use fava_simple_groups::Group;

let photos = Group::on(["wss://groups.com"], "photos")?;

let obs = fava.observe(photos.events(Query::events().kind(Kind::from(9)).limit(50)?))?;
for ev in obs.current().events() {
    println!("{}", ev.event.content());
}

let accepted = fava.publish(photos.publish(draft)?)?;
```

## Who's in the group

Now the app wants to show the member list, and the group's name and about, for that same photography group.

```rust
use fava_simple_groups::{Group, GroupRecords};

let photos = Group::on(["wss://groups.com"], "photos")?;

let obs = fava.observe(photos.records(GroupRecords::members()))?;
for m in photos.project(obs.current()).members() { /* a pubkey the relay listed */ }

let obs = fava.observe(photos.records(GroupRecords::metadata()))?;
let meta = photos.project(obs.current()).metadata();
println!("{} — {}", meta.name(), meta.about());
```

## Discovery

Discovery is how the app finds groups you don't already have an address for — the groups you've bookmarked, or the groups the people you follow are in or help run — so it can suggest groups to join.

### Groups I've saved

The app wants to show the user the NIP-29 groups they've bookmarked.

```rust
use fava_simple_groups::{SimpleGroups, SavedGroup};

let q = SimpleGroups::saved_groups([me]);          // kind 10009, author = me
let obs = fava.observe(q)?;
for ev in obs.current().events() {
    for g in SavedGroup::from_event(&ev.event) {    // each saved row, typed
        println!("{}  @ {}  {}", g.id(), g.relay(), g.name().unwrap_or(""));
    }
}
```

### Groups the people I follow have bookmarked

Let's say the app wants to show you the list of groups the people you follow have bookmarked. Simple:

```rust
let follows = my_contacts.followed();
let q = SimpleGroups::saved_groups(&follows);      // same builder, more authors
let obs = fava.observe(q)?;
// results key by (host, id); the same id at two relays is two entries, and each
// carries which follow saved it (N29-LIST-006).
```

### Which groups are the people I follow an admin of?

You can even ask, very simply: which groups are the people I follow an admin of?

```rust
use fava_simple_groups::{SimpleGroups, GroupAdmins, Group, GroupRecords};

let follows: BTreeSet<PublicKey> = my_contacts.followed();
let q = SimpleGroups::groups_where_admin(&follows);   // kind 39001, #p ∈ follows
let obs = fava.observe(q)?;

let discovered: Vec<Group> = obs.current().events().filter_map(|ev| {
    let admins = GroupAdmins::from_event(&ev.event).ok()?;
    let host   = RelayUrl::from(ev.relay_evidence.source());
    Group::on([host], admins.id()).ok()
}).collect();

for g in &discovered {
    let obs = fava.observe(g.records(GroupRecords::metadata()))?;
    let meta = g.project(obs.current()).metadata();
    println!("{}  {}", g.id(), meta.name());
}
```

Discovery is always an ordinary `Query` — no “list all groups” door, no hidden completeness (N29-READ-009, N29-OWN-003).

## Forks

You've been in “photos” for a long time. Then its maintainers, Alice and Bob, have a fight, so Alice forks off into a new relay — Bob keeps `wss://bob.relay.com`, Alice opens `wss://alice.relay.com`. NIP-29 now sees two independent groups: their member lists and “about” text drift apart, and posts land on one relay or the other. You still like both, so in your app you decide to treat them as one — one “photos” feed showing everything from both, with a small note when the two relays disagree about who's an admin or what the group is called. When you post, you pick which side it goes to.

```rust
use fava_simple_groups::{Group, GroupRecords};

let photos = Group::on(["wss://bob.relay.com", "wss://alice.relay.com"], "photos")?;

// everything else stays the same — one feed, posts from both relays, deduped by event id.
let obs = fava.observe(photos.events(Query::events().kind(Kind::from(9)).limit(50)?))?;
for ev in obs.current().events() {
    println!("{}", ev.event.content());
}

// and the app can choose how to resolve metadata conflicts or anything like that:
let obs = fava.observe(photos.records(GroupRecords::metadata()))?;
let snap = photos.project(obs.current());
if snap.metadata_differ() {
    let bob = snap.at("wss://bob.relay.com");   // pick a side — the crate won't choose for you
    println!("about (bob's side): {}", bob.metadata().about());
}
```

## API

### `Group` — one group id over a host set
`Group::on(hosts, id) -> Result<Group, GroupError>` — non-empty host set + opaque id; refuses empty. Each host is a `RelayUrl` **or a `&str` literal** (parsed internally via `TryInto<RelayUrl>`); parse and empty-set errors fold into `GroupError` (one `?`). The group carries host set + id privately; the write door is the only thing that yields both into a `WriteIntent`.
`Group::on_many(hosts, ids)` — same host set over several group ids for one write (added when that case is real).

Reads (return ordinary `Query`):
- `group.events(selection) -> Query` — content; `#h = id`, sourced from all hosts. Refuses a `selection` that already sets `#h`.
- `group.records(which: GroupRecords) -> Query` — relay-signed records (39000–39005); `#d = id`, `only_from_relays(hosts)`.
- `group.project(&QuerySnapshot) -> GroupSnapshot` — pure projection: merged view + `per_host` + per-kind `*_differ()` + `at(host)`.

Writes (return ordinary `WriteIntent`, routed `Explicit({hosts})`, kind-blind, one `#h` row):
- `group.publish(draft)` / `publish_signed(event)` — app content.
- `group.join()` / `leave()` / `create()` / `delete()` / `create_invite()` / `delete_event(e)`.
- `group.put_users(&[...])` / `remove_users(&[...])` — kind 9000/9001, `p` rows + roles.

### Discovery builders (return ordinary `Query`)
- `SimpleGroups::saved_groups(authors)` — kind `10009` group rows `["group", id, relay, name?"]` by those authors (N29-LIST-006). Keys results by `(host, id)`; keeps same id at another relay separate; retains every author who saved it.
- `SimpleGroups::saved_relays(authors)` — kind `10009` relay-in-use rows `["r", relay-url]` by those authors (N29-LIST-002). Keys by relay-url; retains every author who saved it. Owned here — not `fava-nip65`.
- `SimpleGroups::groups_saved_by(relation)` — dynamic: every author whose `10009` named that exact group, as inputs change.
- `SimpleGroups::groups_where_admin(subjects)` — kind `39001`, `#p ∈ subjects`.
- `SimpleGroups::groups_where_member(subjects)` — kind `39002`, `#p ∈ subjects`. Caveat (N29): member lists can be absent/partial — inclusion is evidence of membership, omission is not.
- (NIP-65 kind `10002` relay lists are a different capability — they remain in `fava-nip65`.)

### Saved-list ops (semantic kind-10009; rebase over the actor's latest `10009`; need `ReplaceableEventEdit`)
- `SimpleGroups::save_group(group, name?)` / `remove_group(group)` / `rename_saved_group(group, name)`.
- `SimpleGroups::save_relay(relay)` / `remove_relay(relay)` — add/remove a relay-in-use row (N29-LIST-004).

### Rebase verbs (need `ReplaceableEventEdit`)
- `group.edit_metadata(latest, action)` (9002), `group.set_pins(items)` (9010).

### Values & parsers — pure, `from_event` (the crate parses, apps don't)
`GroupMetadata` (39000: `.name()`/`.about()`/`.picture()`/`.livekit()`), `GroupAdmins` (39001: `.id()`/`.admins()`/roles), `GroupMembers` (39002), `GroupRoles` (39003), `GroupParticipants` (39004), `GroupPins` (39005: ordered `PinnedItem`s), `SavedGroup` (10009 `["group",…]`), `SavedRelay` (10009 `["r",…]`) — each `from_event(&EventValue)`; the crate owns `GroupError` and the `h`/`d`/`p`/`e`/`a`/`r`/`participant` tag vocabulary. `GroupSnapshot` exposes the record projections typed. Depends only on `fava-state` + `fava-write`.

## Design notes

- **Per-relay authority, surfaced not merged.** Same id at two relays is two independent groups; `Group` aggregates their evidence. Member/admin lists union with per-entry host attribution; metadata is latest-`created_at`-wins per record, never field-merged. `*_differ()` / `at(host)` expose the raw per-host truth so a fork stays visible. No `group_exists` / `is_member` / `all_groups` — the crate never claims completeness.
- **Spec deviation (D2/D3).** The NIP-29 spec's `Group` is single-host (§11 “exactly one host relay”, §12.1 “no multi-host identity”). Fava adopts a multi-host `Group::on(hosts, id)` over per-relay authority — relaxing N29-ID-003 / N29-WRITE-002 — because an app legitimately wants “groupA = relayA + relayB under groupA.” Per-relay authority (N29-ID-002/004) is unchanged; forks are surfaced, never merged.
- **The crate parses; apps don't.** Every NIP-29 record decodes to a typed value (`GroupMetadata::from_event`, `GroupAdmins::from_event`, …); `GroupSnapshot` exposes them typed. Apps build queries and read typed fields, never raw tags.
- **Fork resolution is the app's (N29-LIST-008).** The crate surfaces disagreement; it never auto-picks a winning relay, declares a migration, or rewrites the user's saved list.
- **Saved relays are network-derived (N29-LIST-007).** A relay URL found in someone else's `10009` goes through Fava's ordinary relay-admission policy; the crate never auto-trusts or auto-routes to it.
- **Engine-free.** `Group` and every helper are constructible with no engine, store, signer, or runtime handle. Reads return `Query`; writes return `WriteIntent`; you drive them through `Fava::observe` / `Fava::publish`. No second observation or receipt lifecycle.
- **Kind-blind publication.** `publish` appends exactly one `#h` row and does not inspect or approve the kind. Writes route `WriteRouting::Explicit({hosts})`, bypassing the router chain.
- **Records vs content.** Content reads use `#h`; records use `#d`; helpers refuse the wrong axis.
- **Optional, non-invasive.** Adding or removing this crate needs no source change to generic Fava crates; no NIP-29 kind table or `#h` branch lives in the engine.

## Prerequisites

- A **generic tag axis on `Query`** (`#h` / `#d` / `#p` …) lands in `fava-query` first; `fava-simple-groups` ships only after it exists. NIP-29 forbids a group-specific predicate when an ordinary tag filter suffices.

## Status

Not built — this is the target surface. Open sequencing (rebase verbs / `ReplaceableEventEdit` home, spec revision for multi-host routing, FFI parity) is tracked below.
