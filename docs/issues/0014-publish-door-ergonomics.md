# The publish door: scope the publish, then name the payload

**Status:** proposed (awaiting Pablo approval — spec amendment)
**Authority:** `AGENTS.md` (focused local issue before implementation; vocabulary
change; `:72` "Make invalid use unrepresentable or refuse it before opening
work"), `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` WRITE-002
(`:706`), WRITE-003 (`:716`), WRITE-004 (`:728`), WRITE-005 (`:745`), WRITE-011
(`:808`), WRITE-013 (`:831`), WRITE-018 (`:877`), ID-003 (`:1168`),
`docs/spec/ARCHITECTURE.md` `:490`, `:495`, `:844`,
`docs/spec/partial-spec-api-semantics.md` `:26` and §11 rule 1 (`:621`).
**Depends on:** `docs/issues/0015-publish-scope-vocabulary.md` — this issue
consumes `PublishAs` and `PublishTo` and does not declare them (`AGENTS.md:58`,
"A feature change cannot approve its own new vocabulary").
**Companion:** `docs/issues/0013-edit-author-at-taker.md` owns *where the author
lives* (edit → accepted write) and its WRITE-003 amendment. This issue owns *the
door the application knocks on*. Neither lands alone.
**Code references** are against `milestone/m7-semantic-writes`, where the edit
payload and `fava-nip02` exist.

## Product result

```rust
let w = fava.publish(nip02::follow(bob)?)?;                        // active signer, auto routing
let w = fava.by(carol).publish(nip02::follow(bob)?)?;              // carol's signer
let w = fava.to([r1, r2]).publish(nip25::like(id))?;               // named relays
let w = fava.by(carol).to([r1, r2]).publish(nip02::follow(bob)?)?; // both

w.settled(all()).await?;                        // every destination terminal
w.settled(at_least(2)).await?;                  // two acknowledgements
w.settled(|r| r.acknowledged() >= 2).await?;    // the same, written out
```

Publishing in the ordinary case is one line with no routing syntax, no options
type, and no intent ceremony. Narrowing happens to the left of the payload,
which is the Rust spelling of `publish(edit, as: carol, to: […])` — the keyword
arguments Rust does not have. `?` works directly; there is no terminal call.

## The two axes, and why only one of them is conditional

- **Which signer.** Free only for a replaceable-event edit. An unsigned event
  already carries its `pubkey` and a pre-signed event has already used a signer
  (WRITE-002, `:706`), so naming a signer for either could only be ignored or
  contradictory — the parallel author field WRITE-003 forbids (`:724`, "No
  parallel author field may contradict the event or edit").
- **Which relays.** Free on every payload (WRITE-011, `:808`).

`fava.by(carol).publish(nip25::like(id))` **does not compile**: `PublishAs::publish`
accepts only an edit, and a reaction is not one. This is a type error, not a
runtime refusal and not a `compile_fail` test guarding a runtime check —
`AGENTS.md:72`.

## Surface

```rust
impl Fava {
    pub fn publish(&self, w: impl IntoWrite) -> Result<Write, PublishError>;
    pub fn by(&self, signer: PublicKey) -> PublishAs<'_>;
    pub fn to(&self, relays: impl IntoRelays) -> PublishTo<'_>;
}

#[must_use] pub struct PublishTo<'a> { /* private */ }
impl PublishTo<'_> {
    pub fn publish(&self, w: impl IntoWrite) -> Result<Write, PublishError>;  // any payload
    pub fn by(self, signer: PublicKey) -> PublishAs<'_>;
}

#[must_use] pub struct PublishAs<'a> { /* private */ }
impl PublishAs<'_> {
    pub fn publish(&self, edit: impl IsEdit) -> Result<Write, PublishError>;  // edits only
    pub fn to(self, relays: impl IntoRelays) -> PublishAs<'_>;
}
```

Protocol crates are untouched by this door. `fava_nip02::follow(bob)` returns a
`ReplaceableEventEdit` and nothing else; author, routing, signing, and
publication are not NIP-02 and do not appear in its signature, its types, or its
dependencies. Adding a protocol crate later costs one `IntoWrite` (or `IsEdit`)
impl; both axes come free.

`by` names a public key, and signers are indexed by public key
(`fava-publication/src/lib.rs:60-66`), so naming the key names the signer. Per
`0013` the author is resolved once at acceptance and persisted with the edit,
never re-derived. With no `by` and no session, ID-003 (`:1168`) requires a
refusal before acceptance rather than a silent pick.

## Publish is synchronous; the value it returns is what you await

Acceptance is a durable local commit and the caller must learn it immediately.
WRITE-005 (`:745`): "Every accepted materialized event MUST appear immediately in
matching open and newly opened queries … **Acceptance:** two matching queries show
the accepted event before any relay is contacted." WRITE-013 (`:831`): "Starting
automatic routing MUST NOT wait for network acquisition." WRITE-004 (`:728`)
fixes what must be committed atomically before `Accepted` is reported. A
synchronous return states all of that in the type: no await means no waiting.

Delivery is the thing worth awaiting, and the threshold belongs to the caller:

```rust
impl Write {
    pub fn receipt_id(&self) -> ReceiptId;
    pub fn write_id(&self) -> WriteId;
    pub fn receipt(&self) -> Receipt;
    pub async fn settled(&self, enough: impl Fn(&Receipt) -> bool)
        -> Result<Receipt, PublishError>;
}

pub fn all() -> impl Fn(&Receipt) -> bool;
pub fn at_least(n: usize) -> impl Fn(&Receipt) -> bool;
```

This matches the facade's existing grammar rather than adding to it: `Fava` has
exactly two public `async fn` — `observe` and `wait_terminal` — and both are async
because the call genuinely waits on external progress. `wait_terminal(receipt_id)`
already *is* this door, spelled as a free method taking an id; moving it onto the
returned value and generalizing past the single terminal threshold is what stops
applications writing their own reducer, which is what WRITE-018 (`:877`) asks for:

> The application MUST also be able to await one terminal result for the whole
> write without implementing its own reducer. Mixed outcomes remain visible rather
> than collapsing into a misleading boolean. The receipt SHOULD expose derived
> counts such as acknowledged destinations over total destinations so every
> application need not reimplement that arithmetic.

A predicate makes that SHOULD-clause load-bearing: `Receipt` must expose
`acknowledged()`, `rejected()`, and `desired()` or every closure hand-matches
`RelayDeliveryOutcome` variants. It also keeps the denominator honest — under
`Auto` the destination set grows as routers report (WRITE-015, WRITE-028), so a
share-of-total predicate must say `r.route_settled && …` and the question answers
itself instead of hiding inside a threshold type.

**Termination is the engine's job, not the predicate's.** The predicate is
evaluated on every receipt change, and the await also resolves the moment the
receipt leaves `ReceiptOutcome::Open`, satisfied or not. Retry is bounded
(WRITE-019, `:895`), so terminality always arrives and a predicate that never
fires cannot hang. Terminal-without-satisfaction is
`PublishError::NotReached { receipt }`, so `w.settled(at_least(2)).await?` reads
as "two acknowledgements, or fail" with the facts in the error — WRITE-018's
"mixed outcomes remain visible" enforced by the type.

The predicate is `Fn`, not `FnMut`: it is a question about current facts, and one
accumulating state across receipt revisions is a bug factory.

## `WriteIntent` demoted from the application surface, kept internally

`WriteIntent` is a spec symbol (`vocabulary.toml:395-409`,
`spec_symbols = ["WriteIntent", "WritePayload", "WriteRouting"]`;
`ARCHITECTURE.md:495`), so this is a spec amendment, not a free refactor.

It earns its keep *inside* the engine and stays `pub` in `fava-write` for
cross-crate use: `preview_write_routes` takes one, rematerialization rebuilds one
(`fava-publication/src/run.rs:179`), and redb re-validates recovered state against
one (`fava-write-store-redb/src/validation.rs:183`). What does not earn its keep
is making the *application* hand-construct one.

Demotion is precisely: stop re-exporting `WriteIntent` and `WritePayload` from the
`fava` facade. `WriteIntentError` stays exported — applications see refusals.
`WriteRouting` stays exported — `Receipt.routing` is public state applications
read. The door builds the intent internally, supplying `WriteRouting::Automatic`
when no `to` was named.

Direction of travel: the spec's `WriteIntent` (`ARCHITECTURE.md:495`) is a plain
struct with `pub payload` / `pub routing`. The opaque validating constructor is a
code invention. Making it internal moves *toward* the spec's shape.

## Fold-in: `NonEmptyVec` routing

`ARCHITECTURE.md:490` specifies `Explicit(NonEmptyVec<RelayUrl>)`. The code uses
`BTreeSet<RelayUrl>` with a runtime emptiness refusal
(`fava-write/src/lib.rs:182-184`, `WriteIntentError::EmptyExplicitRelays`) — an
unrepresentable-invalid-use miss sitting in the exact code this issue touches.

Two facts before implementing:

- `NonEmptyVec` has no implementation anywhere; `git grep NonEmptyVec` returns
  only `ARCHITECTURE.md:490` and `:559`, and it has no `vocabulary.toml` entry.
  Introducing it is a vocabulary addition in its own right and does not block this
  door.
- `:559` is `QueryRouting::Explicit(NonEmptyVec<RelayUrl>)` — the read side has the
  identical shape. One type serves both; this issue owns the write side and must
  not silently change query routing.

**`to([a, b, a])` semantics:** normalize — dedup by relay identity, preserve
first-occurrence order, refuse empty. Sending to the same relay twice is redundant,
not invalid, so normalize rather than refuse; order is preserved for router
priority and fallback. `MAX_EXPLICIT_RELAYS` stays a runtime bound — an upper
bound is not expressible in the type and its refusal is correct.

Also unnoted until now and in scope for anything touching `WriteRouting`: the spec
spells the automatic variant `Auto` (`ARCHITECTURE.md:490`) and the code spells it
`Automatic` (`fava-write/src/lib.rs:57`).

## Blast radius

| crate | sites |
|---|---|
| `fava` | `publish` becomes `publish` + `by` + `to`; new `PublishAs`/`PublishTo`; `publish` returns `Write` instead of `AcceptedWrite`; `wait_terminal` superseded by `Write::settled`; re-exports drop `WriteIntent`/`WritePayload` |
| `fava-write` | `WriteRouting::Explicit` payload type; `validate_routing` loses the emptiness arm, keeps the bound; `Receipt` gains derived counts (`acknowledged`, `rejected`, `desired`) per WRITE-018 |
| `fava-publication` | no signature change — consumes `WriteIntent` as today |
| `fava-write-store-{memory,redb}` | `Explicit` payload type only |
| tests | every `WriteIntent::edit(.., WriteRouting::Automatic)` call site collapses to `publish(edit)?` |

## Exit-gate evidence

- `fava.publish(nip02::follow(bob)?)?` compiles with no routing syntax and no
  `WriteIntent` in application scope.
- `fava.by(carol).to([r1, r2]).publish(nip02::follow(bob)?)?` compiles and
  resolves the author at acceptance per `0013`.
- `fava.by(carol).publish(unsigned_event)` and `fava.by(carol).publish(presigned)`
  **fail to compile**. No runtime refusal exists for this condition.
- `fava_nip02` contains no reference to author, relay, signer, or publication —
  `git grep -n "author\|relay\|signer\|publish" crates/fava-nip02/src` is empty
  apart from doc prose.
- Two matching queries show the accepted event before any relay is contacted
  (WRITE-005 acceptance), and `publish` returns without awaiting any router.
- `w.settled(at_least(2)).await?` resolves on the second acknowledgement, and
  resolves with `NotReached { receipt }` — not a hang — when the receipt reaches a
  terminal outcome with one acknowledgement.
- A dropped `PublishAs` or `PublishTo` produces zero write-store rows, zero
  receipts, and zero provider work; both carry `#[must_use]`.
- An empty explicit relay set is unrepresentable; `.to([a, b, a])` normalizes
  (dedup + first-occurrence order), tested.
- Query routing (`ARCHITECTURE.md:559`) is unchanged by this issue.
- `python3 tools/check_vocabulary.py` and its unit tests pass.

## Falsifier evidence

A `publish` reachable with a signer named for an unsigned or pre-signed event
reintroduces the parallel author field WRITE-003 forbids; the type system must
make the call unwritable rather than the engine refusing it. A `WriteRouting::Explicit`
still constructible empty keeps a refusal the type should have made impossible. A
`publish` that awaits route settlement violates WRITE-013; the deliberate break is
the write-side twin of `FAVA_REWRITE_IMPLEMENTATION_PLAN.md:554` — make publish
await settlement of every router, and the immediate-acceptance scenario must time
out before the delayed router completes. A `settled` predicate that can hang past
receipt terminality violates the bound this issue claims.
