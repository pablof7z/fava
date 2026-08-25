# Partial Specification — Reactive Query Interface

**Status:** Working partial specification  
**Scope:** Rust query-expression surface, relay-source semantics, reactive observation, and protocol-crate query combinators.

This document captures the intended shape of Fava's query interface. Names and exact Rust signatures are illustrative. The important contract is the behavior and composition model.

## 1. Core model

Fava queries are declarative expressions.

An application describes **what events it wants** and may derive values from one query to feed another. The application does not manually execute intermediate queries, expand values, diff results, or reopen subscriptions when an input changes.

The core concepts are approximately:

```rust
Query               // a declarative set of events
ValueSet<T>         // reactive values derived from queries or other value sets
Observation         // one opened Query
QuerySnapshot       // the current materialized result
EventRecord         // an event plus Fava's evidence about it
```

`Query` and `ValueSet<T>` are inert descriptions. Relay work and observation begin only when a final query is opened.

Literal iterator collections are bounded at `fava-query`: `authors`, `ids`,
`kinds`, `tag_values`, `intersect_tag_values`, `from_relays`, and `only_from_relays` return
`Result<Query, QueryError>`. Every axis currently stops before consuming more
than 4,096 items. This number is a provisional `fava-query` implementation
shortcut for resource safety, not a Nostr limit or query-domain semantic. The
axis-specific error retains the observed `actual` and configured `maximum`.
Duplicates and infinite iterators cannot evade the cap. Singleton kind
selection uses `kinds([kind])`; there is no parallel scalar accumulator.
`tag_values` unions repeated values for one key. `intersect_tag_values` instead
adds an exact AND constraint: absent becomes the supplied set, present becomes
the set intersection, and a disjoint or empty intersection remains a
present-empty match-nothing axis. Both use the same tag-input bound and exact
`TooManyTagValues` refusal.

`Auto` routing is the default and SHOULD require no syntax in the ordinary case.

```rust
let articles = events()
    .kinds([30_023])?
    .authors(authors)?;
```

is equivalent in routing intent to an explicit `Auto` source policy.

---

## 2. Reactive values

Query fields may be supplied by reactive values.

For example:

```rust
let follows = events()
    .kinds([3])?
    .authors(CurrentAccount::pubkey())?;

let followed_pubkeys =
    follows.tag_pubkeys("p");

let articles = events()
    .kinds([30_023])?
    .authors(followed_pubkeys)?;
```

`tag_pubkeys("p")` does **not** return a `Vec<PublicKey>`. It returns a reactive value:

```rust
ValueSet<PublicKey>
```

Conceptually:

```rust
pub struct ValueSet<T> {
    expression: ValueExpression<T>,
}
```

A query field such as `.authors(...)` SHOULD accept both literal and reactive values:

```rust
.authors(alice)
.authors([alice, bob])
.authors(reactive_pubkeys)
```

When the reactive value changes, Fava updates the existing query automatically.

The application MUST NOT need to:

- hold the expanded set itself;
- diff old and new values;
- reopen the outer query;
- manage relay subscriptions affected by the change.

An empty reactive set means **match nothing**. It never means "remove this filter field."

---

## 3. Query composition

Fava SHOULD support ordinary set composition of reactive values and event selections.

At minimum:

```rust
a.union(b)
a.intersection(b)
a.difference(b)
```

should be expressible for compatible value sets.

Nested queries remain independent query expressions. Their source and freshness policies belong to the nested query itself and are not inherited from the outer query.

Example:

```rust
let followed = nip02::follows(CurrentAccount::pubkey())
    .freshness(Freshness::MaxAge(Duration::from_secs(300)));

let muted = mutes::muted_pubkeys(followed)
    .freshness(Freshness::Live);

let articles = events()
    .kinds([30_023])?
    .authors(muted)?;
```

The exact syntax may differ, but the policies MUST remain scoped to the query expression they decorate.

---

## 4. Relay source semantics

There are three important source modes.

### 4.1 Default: automatic sources

If the application says nothing:

```rust
let query = events()
    .kinds([30_023])?
    .authors(authors)?;
```

Fava uses the configured automatic router composition.

Routers may contribute relay destinations incrementally and asynchronously. The query begins using destinations already known and adds further relay work as routing knowledge arrives.

### 4.2 Ask these relays

The application may explicitly say:

```rust
let query = events()
    .kinds([30_023])?
    .authors(authors)?
    .from_relays([
        "wss://relay-a.example",
        "wss://relay-b.example",
    ])?;
```

Meaning:

> Ask exactly these relays for this query.

This bypasses automatic routers for this query.

However, this is an **acquisition constraint**, not a result-trust constraint.

A matching event already available from another local source MAY still appear, including:

- an event cached from some other relay;
- an accepted local unpublished event supplied by the write store;
- a matching event already known through another query.

In other words, `.from_relays(...)` controls **where Fava asks**, not **which matching local events the application is allowed to see**.

### 4.3 Only from these relays

The application may instead say:

```rust
let query = events()
    .kinds([30_023])?
    .authors(authors)?
    .only_from_relays([
        "wss://relay-a.example",
        "wss://relay-b.example",
    ])?;
```

Meaning:

> Ask exactly these relays, and only show me events for which one of these relays is actual source evidence.

This also bypasses automatic routers.

For an event already in the event cache to match this query, its provenance MUST include at least one relay in the specified set.

For a newly arriving event to match, it MUST arrive from one of the specified relays.

A matching event known only from another relay MUST NOT appear merely because Fava happens to have it cached.

An unpublished local event with no qualifying relay provenance MUST NOT appear.

If a locally published event later acquires qualifying provenance because one of the specified relays serves it, it may then enter the query result.

This distinction is fundamental:

```text
from_relays(...)
    = acquisition scope

only_from_relays(...)
    = acquisition scope + result provenance constraint
```

The exact source mode is part of query identity. Two otherwise identical queries using different source modes MUST NOT accidentally share evidence or local-result visibility in a way that changes either query's results.

---

## 5. Event records

The application-visible event value SHOULD be called `EventRecord`, not `Row`.

Conceptually:

```rust
pub struct EventRecord {
    /* private event-id-bound fields */
}
```

`RelayOccurrences` records exact relay-access sessions that actually served
the same event id:

```rust
pub struct RelayOccurrence { pub session: RelaySessionKey, pub observed_at: Timestamp }
```

The exact shape may grow, but the distinction is:

- the Nostr event is the protocol value;
- evidence describes what Fava actually knows about that event;
- local publication evidence describes a local accepted publication, if any.

An event coming from several sources is still one logical event record.

---

## 6. Local query sources

A live query may combine matching events from several local/query sources.

At minimum:

```text
EventCache
WriteStore
live admitted relay events
```

The application sees one merged result.

An accepted unpublished event does not need to be inserted into the event cache. The write store can independently supply its current materialized event to matching queries.

When the same signed event later arrives from a relay:

```text
WriteStore contribution
    + EventCache / relay contribution
    -> one EventRecord
```

with combined publication and relay evidence.

This avoids making the event cache authoritative for incomplete or unpublished state.

The source-mode rules above still apply. In particular, `only_from_relays(...)` excludes a write-store-only event until it has qualifying relay provenance.

---

## 7. Opening and observing a query

Opening a query returns a live latest-state observation.

A prospective API:

```rust
let mut feed = fava.observe(query).await?;
```

Once `observe()` succeeds, the initial local state is immediately readable:

```rust
render(feed.current());
```

Subsequent changes are observed as newer complete current states:

```rust
while let Ok(snapshot) = feed.changed().await {
    render(snapshot);
}
```

Conceptually:

```rust
pub struct QuerySnapshot {
    pub revision: QueryRevision,
    pub events: Arc<[EventRecord]>,
    pub evidence: QueryEvidence,
}

impl Observation {
    pub fn current(&self) -> Arc<QuerySnapshot>;

    pub async fn changed(
        &mut self,
## 8. Example: articles by people muted by people I follow

Assume protocol crates expose common typed query combinators.

The application wants:

> kind `30023` articles authored by people who have been muted by people the current account follows.

The low-level composition could be:

```rust
let follows = events()
    .kinds([3])?
    .authors(CurrentAccount::pubkey())?;

let followed =
    follows.tag_pubkeys("p");

let mute_lists = events()
    .kinds([10_000])?
    .authors(followed)?;

let muted =
    mute_lists.tag_pubkeys("p");

let articles = events()
    .kinds([30_023])?
    .authors(muted)?
    .newest_first()
    .limit(100)?;

let mut feed = fava.observe(articles).await?;
```

With protocol-crate combinators, the application SHOULD be able to write:

```rust
let followed =
    nip02::follows(CurrentAccount::pubkey());

let muted =
    mutes::muted_pubkeys(followed);

let articles = events()
    .kinds([30_023])?
    .authors(muted)?
    .newest_first()
    .limit(100)?;

let mut feed = fava.observe(articles).await?;
```

Or, if an article protocol crate provides a useful wrapper:

```rust
let articles = articles::by(
    mutes::muted_pubkeys(
        nip02::follows(CurrentAccount::pubkey())
    )
);

let mut feed = fava.observe(articles).await?;
```

All forms describe one dependency graph. They do not manually sequence queries.

### Reactive behavior

If the current user follows Alice and Bob:

```text
follows = { Alice, Bob }
```

and their mute lists resolve to:

```text
Alice mutes Carol
Bob mutes Dave
```

then:

```text
muted = { Carol, Dave }
```

and the final query contains current kind-30023 articles by Carol and Dave.

If the current user then follows Eve:

```text
follows = { Alice, Bob, Eve }
```

Fava adds only the new dependency needed for Eve.

If Eve mutes Frank:

```text
muted = { Carol, Dave, Frank }
```

Frank's matching articles enter the same open feed. Cached articles may appear immediately; additional relay work may be added asynchronously by the configured routers.

If Alice later unmutes Carol:

```text
muted = { Dave, Frank }
```

Carol's articles retract from the current feed automatically.

The application does not manually filter them out.

If the current account changes, `CurrentAccount::pubkey()` changes as a reactive root and the same open query reroots to the new account's dependency graph.

---

If Alice later unmutes Carol:

```text
muted = { Dave, Frank }
```

Carol's articles retract from the current feed automatically.

The application does not manually filter them out.

If the current account changes, `CurrentAccount::pubkey()` changes as a reactive root and the same open query reroots to the new account's dependency graph.

---

## 9. Rendering in a Rust application

The UI SHOULD render immutable query snapshots rather than embed Fava query logic in rendering code.

For example, with an `egui`-style application:

```rust
struct ArticleFeedState {
    snapshot: Arc<QuerySnapshot>,
}
```

When the feed opens:

```rust
let mut feed = fava.observe(articles).await?;

let state = Arc::new(RwLock::new(ArticleFeedState {
    snapshot: feed.current(),
}));
```

An async task owns the live observation:

```rust
let state_for_task = state.clone();
let ctx = egui_ctx.clone();

tokio::spawn(async move {
    while let Ok(snapshot) = feed.changed().await {
        state_for_task.write().unwrap().snapshot = snapshot;
        ctx.request_repaint();
    }
});
```

Rendering is ordinary application code:

```rust
fn show_feed(
    ui: &mut egui::Ui,
    state: &ArticleFeedState,
) {
    for record in state.snapshot.events.iter() {
        ui.heading(article_title(record.event()));
        ui.label(article_summary(record.event()));
        ui.small(format!("by {}", record.event().pubkey()));
        ui.separator();
    }
}
```

The renderer does not know:

- how follows were expanded;
- which mute lists changed;
- which relays supplied each dependency;
- how routers discovered more relays;
- whether an event came from the event cache or write store;
- which subscriptions were shared or regrouped.

Those are Fava concerns.

---

## 10. Protocol-crate query combinators

Protocol crates expose the Nostr relationships applications commonly think in
through ordinary queries and pure snapshot projections. NIP-02 provides the
current concrete model:

```rust
contact_list(authors) -> Result<Query, QueryError>
followers_of(subject) -> Result<Query, QueryError>
follows_of(snapshot) -> Vec<PublicKey>
```

`contact_list(authors)` asks for kind-3 replaceable events and does not apply a
global result limit; the query evaluator selects the newest event independently
at each author coordinate. `followers_of(subject)` adds the exact lowercase
`p` tag value. Both query builders return `QueryError` when the neutral query
owner refuses bounded construction. `follows_of(snapshot)` is a bounded, pure
projection over the current `QuerySnapshot`, not another observation.

`ContactList` accounts for every `p` row in source order. Valid rows expose
typed pubkeys, relay hints, and UTF-8 petnames; malformed, duplicate, and
uninterpreted rows remain typed evidence. Follow/unfollow edits preserve
unknown tags, extensions, malformed unrelated rows, unrelated valid rows,
content, and first-occurrence order.

### Composing protocol crates

An application can express two-hop discovery without a protocol-specific
observation:

```rust
let first = fava.observe(contact_list(alice)?).await?;
let first_hop = follows_of(first.current().as_ref());
let second = fava.observe(contact_list(first_hop.as_slice())?).await?;
```

Protocol crates MUST NOT depend on one another merely to enable this composition.

Protocol crates depend on generic query vocabulary, not on each other. They
return `Query` and pure values rather than protocol-specific observation types
such as:

```text
Nip02FollowSubscription
BookmarkObservation
```

The single observation lifecycle remains `fava.observe(...)`.

Generic reactive `ValueSet<T>` composition is a separate unpromised boundary.
No current NIP-02 or simple-groups API returns it.

### Multi-relay simple groups

`fava-simple-groups` applies the same rule to NIP-29. A `SimpleGroup` is an
inert query/write description over one simple group id and a non-empty
relay sequence:

```rust
let bob = RelayUrl::parse("wss://bob.relay.example")?;
let alice = RelayUrl::parse("wss://alice.relay.example")?;
let photos = SimpleGroup::from_relays("photos", vec![bob, alice])?;

let feed = photos.events(Query::events().kinds([Kind::from(9)])?.limit(50)?)?;
let state = photos.meta_events([SimpleGroupStateEventKind::Metadata])?;
```

`feed` uses query-owned `Query::intersect_tag_values` to constrain the `h` axis
to the exact group id without widening existing values, then applies
`from_relays(relays)`. A disjoint axis stays present-empty and matches nothing.
This keeps explicit acquisition and
optimistic local write visibility without moving query algebra into the
protocol crate. `state` lowers to the exact `d` tag-value axis and
`only_from_relays(relays)` because relay-generated state is authoritative per
relay. Kind, tag-value, and relay inputs delegate to `fava-query`; the helper
returns exact `QueryError` values and adds no private result limit.

The observation remains an ordinary `QuerySnapshot`. Applications use its
generic `EventRecord::relay_evidence` for relay-local selection. The protocol
crate decodes each individual state event; it does not project snapshots,
compare relays, select a fork, or verify events.

Kind-10009 uses one ordinary query and one decoded value per event:

```text
saved_group_lists(authors) -> Result<Query, QueryError>
SavedGroupList::from_event(event) -> Result<SavedGroupList, SavedGroupListDecodeError>
```

The list exposes ordered saved-group and relay entry results. Crate-root save,
rename, remove, and relay functions return pure `ReplaceableEventEdit` values;
the materializer joins Fava's ordinary semantic-write lifecycle.

Simple group publication produces an ordinary event or replaceable edit. The
application supplies the selected relays to the universal door with
`fava.to(simple_group.relays()).publish(payload)`. `prepare` accepts only an
unsigned event, preserves every existing tag, and appends one matching `h` tag
only when none already matches. It creates no protocol-specific publication or
receipt lifecycle.

---

## 11. Design rules

1. **Auto is the default.** Applications mention routing only when they want an explicit source constraint.
2. **Acquisition and trust are separate.** `from_relays(...)` chooses where to ask; `only_from_relays(...)` additionally constrains which local/live events may enter the result.
3. **Reactive values stay declarative.** Apps do not receive intermediate `Vec`s they must manage.
4. **The final observation is latest state.** Apps render current snapshots rather than replaying Fava's internal mutation history.
5. **Protocol helpers lower to core expressions.** They never create private subscription or observation lifecycles.
6. **Protocol crates compose through core types, not dependencies on one another.**
7. **Local sources are hidden behind the query.** Event cache, write store, and live relay arrivals merge into one event result.
8. **`EventRecord` is the event-domain value.** `Row` is reserved for actual database/UI implementation terminology if used at all.
9. **Source policy is part of query meaning.** Different source/trust policies cannot be collapsed merely because their Nostr filters are identical.
10. **Routers remain invisible to ordinary query code.** Automatic routers can add relay work asynchronously without requiring the application to reopen or mutate its query.
