# Fava consumer audit — 2026-08-23

Building a real Nostr client on Fava, as an outside developer, through
`apps/canary`. No workarounds: everything that could not be expressed through
the public facade is recorded here and left failing.

- Fava revision: `645e80a` (branch `main`), worktree branch
  `worktree-agent-adeb07cd770edf371`.
- Relay: **real**. `nostr-rs-relay` 0.10.0 running in Docker on
  `ws://127.0.0.1:7447`, reached through the canary's transparent WebSocket
  proxy so frame and connection counts are witnessed outside Fava.
  Public relays were reachable (`relay.damus.io` answered) but were not written
  to.
- Command: `canary run dx-flows --relay-url ws://127.0.0.1:7447 --seed …`
- Evidence: `apps/canary/runs/dx-flows-*/flows.json`, `report.md`,
  `wire/flows.jsonl`.

Both previously-claimed fixes were checked by using the library. **Neither is
fixed on `main`.**

---

## Verdict

| | |
|---|---|
| Can an application create an account at runtime and use it without restarting? | **No.** |
| Does a relay that fails to connect keep the app responsive? | **Partly.** A refused connection no longer freezes automatic routing. A relay that drops packets still freezes `observe` for the full TCP timeout — measured at **75 s** on macOS. |
| Flows completed as an ordinary consumer | 4 of 10 clean; 6 hit walls |
| Show-stoppers | 4 |

### The single worst thing about building on Fava today

**An application cannot give the engine a signer after `build`.** Signers are
consumed by `FavaBuilder::build` into an immutable
`BTreeMap<PublicKey, Arc<dyn Signer>>` inside `Publication`, and `Fava` exposes
no mutation of any kind. An app that starts before its user has an account has
no publication subsystem at all — `publish` returns
`PublicationError::NotConfigured` — and there is no door to acquire one. Every
Nostr client's first-run experience is "create an account, then use it". On
Fava today that is "create an account, then restart".

---

## Flow results

Verbatim from `flows.json`.

| Flow | Result |
|---|---|
| 1. Engine + live query before any account | **passed** — engine assembles with no signer, `observe` returned in 4 ms |
| 2. Every relay unreachable, local view immediately | **wall — show-stopper** |
| 3. Create account at runtime, attach signer, publish | **wall — show-stopper** |
| 4. Second account, switch, publish as each | **wall — major** |
| 5. Read profile + contact list, follow, unfollow | **passed** |
| 6. Publish a note to automatically-routed relays | **passed** (app-relay routing; outbox routing is blocked, see WALL-5) |
| 7. Observe the same query twice, one connection | **wall — major** |
| 8. One relay up, one down; responsive and distinguishable | **wall — show-stopper** |
| 9. Cancel a write before delivery | **passed** |
| 10. Close the engine cleanly, nothing hangs | **wall — minor** |

### What worked, plainly

These were pleasant and needed no tricks:

- **Assembly is honest.** `FavaBuilder` names every required role and refuses
  by name (`MissingEventCache`, `MissingPublisher`, …). Nothing is silently
  defaulted. An accountless read-only engine is a legal, first-class assembly.
- **Opening a query before any account exists works and is fast.** 4 ms to a
  populated local snapshot.
- **NIP-02 is genuinely good.** `fava_nip02::follow(pk)` /
  `unfollow(pk)` produce edits, `fava.by(author).publish(edit)` accepts them,
  the materializer merges against the current list, and
  `fava_nip02::contact_list(pk)` + `follows_of(&snapshot)` read them back. The
  full follow → read → unfollow → read cycle passed against a real relay
  without a single internal import. Profile publish and read-back likewise.
- **Automatic write routing over application relays works.** `fava.publish(e)`
  with an `AppRelayRouter` reached the relay; an independent raw-WebSocket query
  outside Fava confirmed the relay stored the event.
- **`Write::settled(all())` is the right shape.** Awaiting a predicate over the
  receipt, with the terminal receipt returned as evidence on failure, is a
  better API than a bare "did it publish" boolean.
- **Cancellation works and is atomic.** `cancel_publication` returned
  `Cancelled` and `open_receipts()` dropped to zero.

---

## Walls

### WALL-1 — No runtime signer attachment · show-stopper

**Trying to do:** start the app, let the user create an account, attach its
signer to the running engine, publish. No restart.

**Code written** (`apps/canary/src/flows.rs`, `flow_03_runtime_signer`):

```rust
// The application starts before its user has an account.
let engine = read_only_engine(&[live.clone()])?;

// The user now creates an account.
let account = Keys::generate();

// The line an application needs here does not exist:
//
//     engine.add_signer(Arc::new(LocalSigner::new(account.clone())));

let note = EventBuilder::new(account.public_key(), Kind::TextNote)
    .content("hello")
    .build()?;
let refusal = engine.publish(note);
```

**What happened:** `publish` returned `publication is not configured`. There is
no `add_signer`, `attach_signer`, `set_account`, `session`, or any other
mutator on `Fava`. The only registration sites are `FavaBuilder::signer` and
`FavaBuilder::signers`, both consumed by `build`.
`fava_publication::Publication` stores
`signers: Arc<BTreeMap<PublicKey, Arc<dyn Signer>>>`, built once in
`Publication::new`, with no interior mutability.

Worse: `FavaBuilder::build` only constructs a `Publication` at all if a signer,
materializer, publisher, or delivery policy was selected. An app that honestly
starts with no account gets `publication: None` — not just "no signer", but no
publication subsystem, forever.

**What an outside developer would conclude:** Fava assumes the set of accounts
is known at process start. That is false for every consumer Nostr client. The
app must either restart to use a new account, or pre-register a throwaway key
and rebuild the whole engine behind the UI when the real account appears —
losing every open observation, the event cache, and any in-flight write.

**Note:** the shared checkout is on branch `architecture/runtime-signer-lifecycle`
with a phase-07.2 SPEC for exactly this. It is specified, not delivered. On
`main` it does not exist.

---

### WALL-2 — Network establishment is still inside `observe` · show-stopper

**Trying to do:** open a query while offline and get the local view
immediately; and open a query against a relay that is simply not answering.

**Code written** (`flow_02_offline_local_view`):

```rust
let engine = read_only_engine(&[blackhole.clone()]);   // ws://192.0.2.1:8080
let started = Instant::now();
tokio::time::timeout(
    Duration::from_secs(5),
    engine.observe(Query::events().kind(Kind::TextNote)),
).await
```

**What happened:** three different results for three kinds of "unreachable".

| Case | Result |
|---|---|
| Automatic routing, connection **refused** | local view returned in 0 ms — good |
| Explicit `Query::from_relays([down])` | **refused**: `relay query refused: relay session open refused: IO error: Connection refused (os error 61)` — no local view at all |
| Automatic routing, packets **dropped** | **froze**; still blocked at the 5 s budget |

The blackhole case is the original bug, unmoved. `Fava::observe` →
`routes::open` → `add_relays` → `OpenedRelay::open` → `establish` →
`transport.open_session(...).await` → `tokio_tungstenite::connect_async`, which
has **no connect timeout**. `observe` cannot return until the OS gives up. A
direct TCP measurement to the same address took **75.0 s** before
`ETIMEDOUT`. `add_relays` also loops over relays sequentially and awaits each,
so N dead relays cost N × 75 s.

An application on a captive-portal Wi-Fi, a VPN half-up, or a relay whose host
has gone away sits with a spinner for 75 seconds per relay. There is no
timeout, budget, or deadline parameter anywhere in the public API.

**What an outside developer would conclude:** the M4 fix moved connection
*failure* out of the critical path but not connection *establishment*. `observe`
is documented as returning a handle that "already contains local state", and
that promise is broken exactly when it matters — offline.

---

### WALL-3 — Explicit relay acquisition is all-or-nothing · show-stopper

**Trying to do:** name a live relay and a dead relay on one query — the normal
case, since some relay in a user's list is always down — and still see results
from the live one.

**Code written** (`flow_08_mixed_relay_health`):

```rust
let query = Query::events()
    .kind(Kind::TextNote)
    .from_relays([live.clone(), down.clone()])?;
engine.observe(query).await
```

**What happened:** `relay query refused: relay session open refused: IO error:
Connection refused (os error 61)`. `fava::live::open_explicit` opens each relay
in sequence and, on the first failure, closes the observation, aborts every
relay already opened, and returns `Err`. One dead relay in the list destroys the
whole query, including the healthy relay's results and the purely local view.

This is not symmetric with automatic routing, which degrades correctly:
`add_relays` records a `route_shortfall` and keeps going. The two acquisition
modes have opposite failure semantics and nothing says so.

**What an outside developer would conclude:** `Query::from_relays` is unusable
in production. Any app that lets a user paste relay URLs must implement its own
per-relay health probing and fan-out before it dares call `observe` — which is
the job it delegated to Fava.

**The good half:** under automatic routing the app *can* tell the two apart.
`fava.diagnostics()` gave `sessions: ["ws://127.0.0.1:50134 gen 1"]` and
`route_shortfalls: ["revision 1: RelaySessionKey { relay: ws://127.0.0.1:50135 … }: relay session open refused: Connection refused"]`,
and the observation opened in 3 ms. So flow 8's "stay responsive and tell them
apart" is achievable — but only through the automatic path, and only by parsing
a `Debug`-formatted `RelaySessionKey` out of a shortfall string (see WALL-9).

---

### WALL-4 — No local-state door · show-stopper

**Trying to do:** put an event into local state — restore a cache, import a
backup, seed a first run, hold a local-only draft.

**What happened:** there is no public API. `Fava::accept_event` is explicitly a
documented `compile_fail` in the facade's own doc comment
(`crates/fava/src/lib.rs`). The only route is to hold your concrete provider and
call the internal contract:

```rust
use fava_event_cache::EventCache;                       // internal
use fava_state::{CachedEvent, RelayEvidence, RelaySessionKey, RelayAccess}; // internal

cache.admit(
    CachedEvent::new(event, RelayEvidence::one(
        RelaySessionKey::new(RelayUrl::parse("wss://m1.local")?, RelayAccess::public()),
        Timestamp::from(11),
    )),
    Timestamp::from(11),
)?;
```

That is what the deleted `apps/canary/src/local.rs` did, for a relay called
`wss://m1.local` that never existed. Three internal crates, and the consumer
must **invent relay provenance** for an event that came from disk.

**What an outside developer would conclude:** Fava can only ever be populated by
Fava. Offline-first is not buildable: no import, no export, no restore, no
seeded tests. The four M1 scenarios existed only because the canary reached
around this.

---

### WALL-5 — Outbox routing cannot be assembled · show-stopper for real routing

**Trying to do:** wire up NIP-65 outbox routing, which is what "automatically
routed" means in Nostr.

**Code that is required:**

```rust
let outbox = OutboxRouter::new("nip65", [discovery_relay], queries)?;
//                                                          ^^^^^^^
//                                          Arc<dyn QuerySource> — from where?
let fava = Fava::builder().router(outbox) /* … */ .build()?;
```

`OutboxRouter` needs a `QuerySource` to look up relay lists. `Fava` implements
`QuerySource`. Routers must be handed to the builder **before** `build`. The
dependency is circular and the public API offers no cell, no `Weak`, no
post-build `attach`, no lazy hook.

The old canary broke the cycle by constructing **a second `Fava`** with its own
`MemoryWriteStore` and its own `WebSocketTransport` and handing that to the
router — the exact thing WRITE-014's acceptance forbids. It then called
`OutboxRouter::remember(...)` with hand-built kind-10002 events so the second
engine never actually had to fetch anything, which means the second engine was
not even doing the job it existed for.

**What an outside developer would conclude:** outbox routing — the single most
important routing behaviour in Nostr — cannot be assembled by an application at
all. `fava-router-outbox` ships but is unusable from outside.

---

### WALL-6 — One connection and one REQ per observation · major

**Trying to do:** render the same feed in two places (a timeline and a badge
count) and pay for one relay connection.

**Measured** (`flow_07_shared_connection`, witnessed by the canary's WebSocket
proxy, not by Fava's own diagnostics):

```json
{ "relay_connections_opened": 2, "engine_sessions": 2, "engine_subscriptions": 2 }
```

Two `observe` calls with an **identical** `Query` opened two TCP connections,
two WebSocket handshakes and two REQs. `Query` derives `Hash` and `Eq`, so
identity is available — nothing uses it. `Observer::open`, `live::open`, and
`routes::open` each build a fresh `OpenedRelay` per observation.

**What an outside developer would conclude:** every widget that observes costs a
socket. A modest client with twenty live views has twenty connections per relay,
and relays with `max_ws_connections` will start refusing. The app must build its
own observation cache and fan-out in front of Fava.

This is also why `subscription-grouping-equivalence` could never be a public
scenario: with one REQ per observation there is nothing for a grouping planner
to group, so the canary drove `SubscriptionPlanner`, its own `Transport`,
`fava_wire` codecs and `fava_ingest::admit_subscription_event` by hand,
bypassing `Fava::observe` entirely — and then wrote `result_equivalence: true`
and `relay_source_evidence_equivalence: true` into the retained manifest as
literals rather than as measured values.

---

### WALL-7 — No public write-route preview · major

`Fava::preview_routes(&Query)` previews **read** routes only. There is no
`preview` for a write, so an application cannot show "this will be sent to
these 4 relays" before the user commits, and cannot compare two write profiles.
The removed M6 scenarios called `fava_routing::preview(&routers, &RouteRequest::Write(event))`
directly — reaching into an internal crate for a facade feature that does not
exist.

---

### WALL-8 — No publication lifecycle signal · major

`Fava::receipt_changes()` broadcasts committed receipts. It does not report
materialization installed, signer refused, route applied, or attempt started.
An application that wants to show "waiting for your hardware wallet" versus
"sending" versus "1 of 3 relays acked" cannot distinguish them.

The removed `semantic_write_store.rs` obtained this by implementing the internal
`WriteStore` contract in full — **twenty-three forwarding methods** over
`MemoryWriteStore` — solely to intercept `install_signed`. That is the cost of
observing a state transition.

---

### WALL-9 — The facade does not re-export types it returns · major

`DiagnosticsSnapshot` is public and re-exported by `fava`. Its fields are
`Vec<(RelaySessionKey, u64)>`, `Vec<(u64, Vec<RelaySessionKey>)>`, and so on.
**`RelaySessionKey` is not re-exported.** Neither is `RelayAccess`. Neither is
`EventId`, which appears throughout query results. An application that wants to
type a function over what `fava.diagnostics()` returns must add
`fava-state = { path = "…" }` to its manifest.

Likewise the provider traits: `Signer`, `Router`, `Transport`, `EventCache`,
`WriteStore`, `Publisher`, `DeliveryPolicy`, `SubscriptionPlanner`,
`QuerySource` are all *taken* by `FavaBuilder` but none are re-exported.
Passing a concrete `Arc<LocalSigner>` works; holding `Arc<dyn Signer>` or
implementing a NIP-46 remote signer does not, without the internal crate.

Route shortfalls arrive as `String` containing a `Debug`-formatted
`RelaySessionKey`. To answer "which relay failed?" an application parses a
debug string.

---

### WALL-10 — No current-account concept · major

`PublishError::MissingAuthor` says "requires an author selection" and the
`Fava` docs mention "no selected current account", but **nothing selects one**.
Every publication site must carry the author: `EventBuilder::new(pubkey, kind)`
for events, `fava.by(pubkey).publish(edit)` for edits. An app with multiple
accounts threads a `PublicKey` through every call site and gets no help
ensuring the signer for it is even registered — that failure surfaces only at
publication time.

Flow 4 did publish as two accounts successfully, so multi-account *works*; it
just has no ergonomics and inherits WALL-1 (both keys must exist before
`build`).

---

### WALL-11 — No `close` on the engine · minor

`Fava` has no `close`, `shutdown`, or `flush`. It is `Clone`, holds
`Arc`-shared state, and spawns detached tasks (`tokio::spawn` in `live::open`,
`routes::open`, `query_source::open`) whose only stop signal is an
`Observation`'s cancellation channel. `Publication` spawns delivery work with no
handle at all.

Measured with a real child process that builds an engine, observes, publishes,
closes the observation and drops the engine: it exited cleanly in **26 ms**. So
in practice nothing hangs. But that is luck, not contract: there is no way to
await in-flight publication before exiting, so an application that drops the
engine while a write is being delivered cannot know whether it was sent.

---

### WALL-12 — Cache membership is not publicly readable · minor

The surviving M5 scenarios assert "the relay echo landed in the event cache" by
calling `EventCache::event(id)` on their own provider (internal crate). A
`cache_only` query merges the event cache and the write store, so there is no
public way to distinguish "this came back from a relay" from "this is my own
optimistic write". `EventRecord` carries `relay_evidence` and `publication`, so
the information exists — it is just not answerable as a question.

---

## Workarounds removed, and what each was hiding

Thirteen scenarios were deleted along with the code that made them pass. Each is
registered `"status": "blocked"` in `scenarios.json` and refuses loudly with its
wall text; the ledger lives in `apps/canary/src/blocked.rs`.

| Removed | Workaround deleted | Wall it hid |
|---|---|---|
| `async-recipient-routing` | second `Fava` with its own `MemoryWriteStore` + `WebSocketTransport` to feed `OutboxRouter`; `OutboxRouter::remember` with hand-built kind-10002 lists | WALL-5 |
| `hint-routing` | `HintRouter::remember` fed with records the canary projected out of an observation | routers are never offered Fava's own ingested evidence |
| `route-preview-parity` | direct `fava_routing::preview` call | WALL-7 |
| `app-relay-versus-fallback-profile` | direct `fava_routing::preview` call | WALL-7 |
| `replaceable-edit-first-value` | `NoopTransport` that refuses every connection + canary `Publisher` returning `Acknowledged` without sending | no deterministic publication path; advertised as a real Fava execution |
| `replaceable-edit-rematerialization` | `NoopTransport` + gate `Publisher` + `CompletionStore` (23 forwarding `WriteStore` methods) | WALL-8 |
| `replaceable-edit-opposing-operations` | same | WALL-8 |
| `protocol-crate-n-plus-one` | same, plus hand-built `RelayEvidence` for `wss://m7-semantic.example` | provenance can only be fabricated |
| `subscription-grouping-equivalence` | hand-driven planner + transport + `fava_wire` + `fava_ingest`, bypassing `observe`; `result_equivalence: true` and `relay_source_evidence_equivalence: true` written as literals | WALL-6 |
| `local-source-merge` | `EventCache::admit` with fabricated `RelayEvidence` for `wss://m1.local` | WALL-4 |
| `local-replaceable-shadow-and-cancel` | same, plus `WriteStore::accept_materialized` | WALL-4 |
| `local-source-removal` | same, plus `EventCache::expire` | WALL-4 |
| `slow-consumer-latest-state` | 256 × `WriteStore::accept_materialized` | WALL-4 |

Also removed, without losing a scenario:

- `publication.rs` read a receipt through the internal `WriteStore` trait before
  assembling the engine. Replaced with `Fava::receipt` after assembly — a real
  public path existed, so the internal use was gratuitous.
- `use fava_state::{RelayUrl, Timestamp}` and `use fava_write::{…}` in six
  modules, where the facade already re-exports those types.
- Eight internal crates dropped from `Cargo.toml`: `fava-bookmarks`,
  `fava-ingest`, `fava-wire`, `fava-publisher`, `fava-transport`,
  `fava-write-store`, `fava-query`, `fava-subscriptions`. Plus
  `fava-router-hints` and `fava-router-outbox`, now unused.
- Five internal crates remain, each annotated in `Cargo.toml` with the hole that
  forces it: `fava-event-cache` (WALL-12), `fava-routing` (Router trait),
  `fava-signer` (Signer trait), `fava-state` (WALL-9), `fava-write` (`EventId`).

`GateSigner` — a signer that asks before signing — was **kept** and moved to
`apps/canary/src/gate_signer.rs`. It is a genuine provider role (a hardware
wallet, NIP-07, or NIP-46 bunker all behave this way), not a stub. It still
needs `fava-signer` because the trait is not re-exported: WALL-9.

## Scenarios that now fail

- Thirteen blocked scenarios refuse with their wall text and exit nonzero.
- `dx-flows` exits nonzero while any of its six walls stands.
- Enabled relay-process scenarios (M0, M2–M5, Croissant) could **not be
  executed** in this environment: the pinned prerequisite `nostr-rs-relay`
  0.8.12 no longer compiles on Rust 1.90 (`time` fails with E0282). They are
  unchanged apart from the import cleanups above and the `Fava::receipt`
  substitution. This is a canary prerequisite problem, not a Fava defect, but it
  means the surviving relay scenarios are currently unverified.
- `cargo test -p canary`: 16 pass. Three Croissant tests fail on the absent
  Croissant executable, unchanged from before this audit.

## Recommended order of repair

1. **Runtime signer/account lifecycle.** Nothing else matters until an app can
   create an account and use it. (WALL-1, WALL-10)
2. **Get `connect` out of `observe`.** A connect deadline, or better, opening
   sessions in the background and letting the observation report relay state as
   it changes. (WALL-2)
3. **Make explicit acquisition degrade like automatic acquisition.** (WALL-3)
4. **A public door into local state**, with provenance the caller does not have
   to invent. (WALL-4)
5. **Break the outbox/QuerySource cycle** so NIP-65 routing is assemblable.
   (WALL-5)
6. **Share connections and subscriptions across observations of equal queries.**
   (WALL-6)
7. Re-export what the facade returns and what its builder accepts. (WALL-9)
