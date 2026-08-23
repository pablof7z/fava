# Transport / wire / ingest audit

**Area slug:** `transport-wire-ingest`
**Scope:** `crates/fava-transport`, `crates/fava-transport-websocket`, `crates/fava-transport-testkit`, `crates/fava-wire`, `crates/fava-ingest`
**Mode:** read-only.

## Scope checked

Read in full (601 lines of production+test source across the five crates):

- `crates/fava-transport/src/lib.rs` (72)
- `crates/fava-transport-websocket/src/lib.rs` (188), `tests/conformance.rs` (97)
- `crates/fava-transport-testkit/src/lib.rs` (56)
- `crates/fava-wire/src/lib.rs` (21), `tests/nip01.rs` (28)
- `crates/fava-ingest/src/lib.rs` (58), `tests/admission.rs` (81)

Read as the consuming/adjacent path: `crates/fava/src/relay.rs` (344, full),
`crates/fava-event-cache/src/lib.rs` (trait head), `crates/fava-diagnostics/src/lib.rs`
(bounding), `crates/fava-publisher/src/lib.rs`, `crates/fava-publisher-nip01/src/lib.rs`.

Authority read: `docs/spec/ARCHITECTURE.md` §`fava-wire` (286-355), §`fava-transport`
(1555-1612), §`fava-transport-websocket` (1614-1631), §`fava-ingest` (2029-2056),
ownership ledger (2960-3010), crate inventory (3560-3630), testing packages (3645-3660);
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` QUERY-015 (483-489),
RELAY-005 (1070-1084), RELAY-006 (1085-1090), OPS-004 (1420-1437), OPS-005 (1439-1450),
WRITE-lane connection sharing (930), anti-goals (1627, 1634), exit criteria (1687);
`docs/internals/vocabulary.toml` (`Transport` 696-720, `RelayIngest` 849-855,
`ClientMessage`/`RelayMessage` 88-110).

Searches actually run (workspace-wide, `--include=*.rs` over `crates/`):
`\.admit\(`, `admit_subscription_event`, `impl (Transport|RelaySession) for`,
`dyn Transport`, `timeout|Deadline|deadline|Instant|elapsed`,
`WebSocketConfig|max_message_size|max_frame_size|connect_async_with_config`,
`CommittedCacheChange|VerifiedRelayEvent`, `encoded_len`, `fava_transport_testkit`.

---

## Transport-owned state ledger (ARCHITECTURE.md:1586-1594)

| Architecture-named owned state | Verdict | Evidence |
|---|---|---|
| DNS/TCP/TLS/WebSocket resources | PRESENT | `fava-transport-websocket/src/lib.rs:66` (`connect_async`), `:87-88` |
| relay URL and relay-access session key | PRESENT | `fava-transport-websocket/src/lib.rs:83`, `:92-94` |
| connection **and reconnect** generation | PARTIAL / OWNED-ELSEWHERE | connect-counter present `fava-transport-websocket/src/lib.rs:23,55-65`; *reconnect* generation is driven by `crates/fava/src/relay.rs:126-168` |
| connection backoff | OWNED-ELSEWHERE | fixed 50 ms sleep at `crates/fava/src/relay.rs:135`; no backoff state in either transport crate |
| bounded inbound and outbound byte queues | **ABSENT** | no queue type anywhere in `fava-transport/src/lib.rs` or `fava-transport-websocket/src/lib.rs`; `send` writes straight to the sink at `:119-125` |
| exact byte-handoff outcomes | PARTIAL | `fava-transport/src/lib.rs:11-25` — present but carries no session/generation identity and no typed reason |
| session health and transport errors | PARTIAL | one `AtomicBool` `closed` `fava-transport-websocket/src/lib.rs:86`; no liveness/keepalive/dead-session state (`Ping`/`Pong` discarded at `:156`) |
| current **and retiring** session lifecycle | **ABSENT** | `Transport::open_session` returns one `Arc<dyn RelaySession>` and forgets it (`fava-transport/src/lib.rs:31-34`); the transport holds no registry, so it cannot have a "current" or a "retiring" session |
| shutdown and resource joining | **ABSENT** | `Transport` has no `shutdown`/`join`; `WebSocketTransport` holds only `max_frame_bytes` + `next_generation` (`:21-24`) and can join nothing |
| URL normalization and admission (:1620) | OWNED-ELSEWHERE | delegated to `RelayUrl` in `fava-state`; `connect_async(key.relay.as_str())` at `:66` performs no admission |
| keepalive and dead-session detection (:1624) | **ABSENT** | `:156` drops `Ping`/`Pong`/`Frame` with no liveness accounting |
| bounded reads and writes (:1623) | PARTIAL | writes bounded (`:110`); reads unbounded by any Fava-owned bound (see `inbound-frames-unbounded-by-declared-bound`) |
| transport-level replay hooks for current subscription plans (:1627) | **ABSENT** | no replay API on either trait; replay is re-done by `crates/fava/src/relay.rs:137-154` |

**Net:** 6 of 13 architecture-named transport facts are absent from the transport crates,
2 more are owned by the `fava` facade, and 3 are present only in degraded form.

---

## Findings

### `relay-session-trait-cannot-multiplex` — critical — ownership / replaceability

**Authority.** `docs/spec/ARCHITECTURE.md:1578` — `fn messages(&self) -> Box<dyn RelayMessageStream>;`
Each caller receives *its own* inbound stream handle, which is what lets one physical session
serve several logical demands. Reinforced by `docs/spec/ARCHITECTURE.md:1610`: "Every inbound
frame and handoff completion carries exact session generation and relay-access identity."

**Implementation.** `crates/fava-transport/src/lib.rs:48-51`:
```rust
fn next_message(
    &self,
) -> Pin<Box<dyn Future<Output = Result<String, TransportError>> + Send + '_>>;
```
and `crates/fava-transport-websocket/src/lib.rs:144`: `let message = self.stream.lock().await.next().await;`

This is a **competing-consumer** shape, not a fan-out shape. Two tasks awaiting `next_message()`
on the same `Arc<dyn RelaySession>` each remove a *different* frame from the socket; there is no
broadcast and no per-subscription routing. Second half of the same gap:
`Transport::open_session(key)` (`crates/fava-transport/src/lib.rs:31-34`) takes only a key and
always opens a **new** connection — there is no `key -> existing session` lookup, no refcount,
and no way to ask for a session that already exists. So the trait pair is *structurally
incapable* of "several writes for one relay SHOULD share connection/backoff ownership"
(`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:930`).

**What must change (precisely).**
1. `RelaySession::next_message()` must be replaced by `fn messages(&self) -> Box<dyn RelayMessageStream>`
   returning an independently-pollable, per-consumer, bounded inbound handle — otherwise a second
   logical subscription on one session silently steals the first one's EVENT frames.
2. `Transport::open_session` must take the spec's `OpenRelaySession` request and return a
   refcounted/attached handle to the *current* session for that key (spec keeps
   `RelaySessionIdentity` at `:1570`), so equivalent demand attaches instead of dialing.
3. `RelaySession` must gain a retire/replace step so reconnect produces a new generation
   *inside* the session object the holders already have (RELAY-006, GOALS:1085-1089); today a
   reconnect must swap the `Arc` in the caller (`crates/fava/src/relay.rs:157`), which is only
   possible because there is exactly one holder.

**Observable distinction.** Open two live queries on the same relay URL with different filters.
Through the public API the application sees two TCP connections and two handshakes to that relay
(observable at any relay), and — if a future change routes both through one session — one query
would begin dropping matching events into the other query's reader.

**Proposed falsifier.**
```rust
#[tokio::test]
async fn one_physical_session_fans_out_every_inbound_frame_to_every_consumer() {
    let (session, script) = scripted_session().await;          // one relay session
    let a = session.messages();                                 // spec shape
    let b = session.messages();
    script.push(r#"["EOSE","one"]"#).await;
    assert_eq!(a.next().await.unwrap(), r#"["EOSE","one"]"#);
    assert_eq!(b.next().await.unwrap(), r#"["EOSE","one"]"#);   // fails: no `messages()`; frame stolen
}
```

**Confidence.** confirmed.

---

### `transport-has-no-byte-queues` — critical — boundedness / failure isolation

**Authority.** `docs/spec/ARCHITECTURE.md:1590` — owned state includes "bounded inbound and
outbound byte queues". `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1437` — "Exceeding a bound
MUST produce refusal, backpressure, or exact shortfall."

**Implementation.** There is no queue of any kind in `crates/fava-transport/src/lib.rs` or
`crates/fava-transport-websocket/src/lib.rs`. `send` writes synchronously through a mutex:
```rust
// crates/fava-transport-websocket/src/lib.rs:119-125
match self.sink.lock().await.send(Message::Text(frame.into())).await {
```

**Can a slow/stalled peer apply unbounded backpressure into Fava? Yes — confirmed, three ways.**
1. `SinkExt::send` awaits `poll_ready`. Once the kernel socket buffer fills because the relay
   stopped reading, this future never completes and there is no deadline (see
   `no-fava-owned-deadline`). The *caller's* task is parked indefinitely.
2. The stall is held under `self.sink.lock()`, so every other sender on that session — including
   unrelated logical subscriptions and publication attempts — is parked behind it.
3. The parked caller is on the **cancellation path**. `crates/fava/src/relay.rs:328`
   (`withdraw`) awaits `session.send(CLOSE)` before `session.close()`. A stalled relay therefore
   makes query close and engine shutdown hang forever, with the session's resources still held.

Because there is no outbound queue, there is also nothing to refuse: the contract's designed
escape hatch `HandoffOutcome::NotHandedOff` is never reachable for a full-buffer condition — the
only bounded refusal implemented is frame *size* (`:110`).

**Observable distinction.** Point a live query at a relay that completes the WebSocket handshake
and then stops reading. `QueryHandle::close()` / `Fava::shutdown()` never return. With a bounded
outbound queue the caller would get `NotHandedOff` (or a shortfall) and close would complete.

**Proposed falsifier.**
```rust
#[tokio::test]
async fn stalled_relay_yields_bounded_refusal_not_an_unbounded_park() {
    let (session, _stalled) = session_against_relay_that_never_reads().await;
    for _ in 0..10_000 { let _ = session.send("x".repeat(1024)).await; } // must not park
    let outcome = session.send("one-more".into()).await;
    assert!(matches!(outcome, HandoffOutcome::NotHandedOff { .. })); // hangs today
}
```

**Confidence.** confirmed.

---

### `no-fava-owned-deadline` — critical — failure isolation

**Authority.** `docs/spec/ARCHITECTURE.md:1624` — transport owns "keepalive and dead-session
detection". `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:424` — "Timeout, disconnect, retry
exhaustion, silence, local cancellation, and relay refusal MUST remain distinct and MUST NOT be
reinterpreted as EOSE or emptiness." A timeout that Fava never owns cannot be reported as a
distinct fact.

**Implementation.** A workspace-wide grep for `timeout|Deadline|deadline|Instant|elapsed` across
`crates/*/src/` returns exactly four hits, all in the publication lane
(`crates/fava-publisher/src/lib.rs:27`, `crates/fava-publisher-nip01/src/lib.rs:58,111`,
`crates/fava-publication/src/delivery.rs:180`). **Zero** in transport. Concretely:

| Deadline | Site | Owner today |
|---|---|---|
| establishment (DNS+TCP+TLS+WS handshake) | `crates/fava-transport-websocket/src/lib.rs:66` | OS TCP connect timeout / peer |
| write / handoff | `crates/fava-transport-websocket/src/lib.rs:119-125` | peer's read rate |
| idle / silence | `crates/fava-transport-websocket/src/lib.rs:144` | peer (awaits forever) |
| close | `crates/fava-transport-websocket/src/lib.rs:180-185` | peer's close handshake |
| liveness probe | none — `:156` discards `Ping`/`Pong` | peer |

**Every transport deadline in Fava is delegated to the OS or to the peer.** There is no keepalive
ping, so a silently black-holed connection (NAT idle-drop, iOS suspend — GOALS:1498) is
indistinguishable from a quiet relay and `next_message()` parks until the OS eventually gives up,
if ever.

**Observable distinction.** Against a relay whose TCP handshake completes but which never sends
the WebSocket 101 response, `Fava::observe` never returns and never reports a distinct timeout
diagnostic. Against a black-holed established socket, the query reports no fact at all rather
than a disconnect, and reconnect (`crates/fava/src/relay.rs:126`) is never triggered because
`next_message` never errors.

**Proposed falsifier.**
```rust
#[tokio::test(start_paused = true)]
async fn silent_socket_becomes_a_transport_owned_timeout_not_an_indefinite_park() {
    let (session, _blackhole) = session_that_accepts_then_never_speaks().await;
    let outcome = tokio::time::timeout(Duration::from_secs(120), session.next_message()).await;
    assert!(matches!(outcome, Ok(Err(TransportError::Disconnected(_))))); // today: Err(Elapsed)
}
```

**Confidence.** confirmed.

---

### `ingest-attribution-check-is-a-no-op` — critical — ownership

**Authority.** `docs/spec/ARCHITECTURE.md:2045` (owned lifecycle step 2) — "attribute an event to
an accepted wire subscription and logical demand". Ownership ledger `:2966` — "Event-id/signature
admission | `fava-ingest`". `docs/internals/vocabulary.toml:851` — "RelayIngest decides whether
the message belongs to current work and may enter the EventCache."

**Implementation.** `crates/fava-ingest/src/lib.rs:34-45` asks the *caller* to supply both sides
of the comparison:
```rust
pub fn admit_subscription_event(
    cache: &dyn EventCache, session: &RelaySessionKey,
    expected_subscription: &SubscriptionId,
    actual_subscription: &SubscriptionId, ...
) -> Result<bool, RelayIngestError> {
    if actual_subscription != expected_subscription { return Err(RelayIngestError::WrongSubscription); }
```
The **sole production caller** performs attribution itself and then passes the same value twice.
`crates/fava/src/relay.rs:264-277`:
```rust
let id = subscription_id.into_owned();
let Some(filter) = attribution.get(&id) else { ...; return; };   // <- real attribution decision
if let Err(error) = admit_subscription_event(cache, session.key(), &id, &id, filter, ...)
```

So `RelayIngestError::WrongSubscription` is **structurally unreachable through the real public
path**. The only place it can be produced is `crates/fava-ingest/tests/admission.rs:29-38`, which
calls the function directly with hand-made mismatched ids. This is exactly the failure mode the
brief names: evidence written to match the shape of the function rather than the authority. The
authoritative attribution fact lives in a `BTreeMap` privately owned by
`crates/fava/src/relay.rs:26`, not in `fava-ingest`.

**Observable distinction.** A relay that returns an EVENT tagged with a subscription id Fava did
open but which is *not the one that filter belongs to* is indistinguishable from a correct frame:
`relay.rs` looks the filter up **by the id the relay chose**, so the relay picks its own filter.
A correct implementation would hold the accepted plan inside ingest and refuse.

**Proposed falsifier.**
```rust
#[tokio::test]
async fn relay_cannot_choose_which_accepted_filter_its_event_is_checked_against() {
    // one query -> two accepted subscriptions on one session: sub-A(kind 1), sub-B(kind 30023)
    let fava = fava_with_scripted_relay(two_subscription_plan()).await;
    relay.push_event("sub-B", kind_1_event()).await;    // valid for A's filter, tagged as B
    assert!(fava.diagnostics().failures().iter().any(|f| f.contains("wrong subscription")));
    assert_eq!(cache.len().unwrap(), 0);                 // today: admitted
}
```

**Confidence.** confirmed.

---

### `unbounded-reconnect-storm` — critical — boundedness

*(New consequence of the known baseline "reconnect lives in `crates/fava/src/relay.rs` as a 50ms
facade loop"; the storm/exhaustion properties below are not in the baseline list.)*

**Authority.** `docs/spec/ARCHITECTURE.md:1589` — transport owns "connection backoff";
`:1625` — websocket transport owns "reconnect backoff".
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:930` — "Several writes for one relay SHOULD share
connection/backoff ownership rather than creating independent reconnect storms."
`:1437` — bounds must produce "refusal, backpressure, or exact shortfall".

**Implementation.** `crates/fava/src/relay.rs:126-168`. The loop sleeps a **constant**
`Duration::from_millis(50)` (`:135`) — no exponential growth, no ceiling, no jitter — and has
**no attempt bound and no terminal shortfall**: on failure it only records a diagnostic (`:161-165`)
and loops forever. There is no backoff state in `WebSocketTransport`
(`crates/fava-transport-websocket/src/lib.rs:21-24` holds only `max_frame_bytes` and
`next_generation`), so nothing is shared: because each `OpenedRelay` owns its own loop, N live
queries on one down relay produce N independent 20 Hz dial loops against the same host.

A second consequence: on a successful reconnect the previous session is dropped without
`close()` (`crates/fava/src/relay.rs:157` overwrites `self.session`), so a half-open generation is
released by `Drop` rather than by the deterministic close the spec assigns to transport
(`ARCHITECTURE.md:1626` "deterministic close").

**Observable distinction.** Take a relay offline while three live queries are open. The relay host
observes ~60 connection attempts per second that never decay and never stop. The application is
never told the relay is unreachable — no exhaustion or shortfall fact is ever emitted, only an
unbounded stream of `failed` diagnostics.

**Proposed falsifier.**
```rust
#[tokio::test(start_paused = true)]
async fn reconnect_backs_off_and_is_shared_across_queries_on_one_relay() {
    let dials = Arc::new(AtomicUsize::new(0));
    let fava = fava_with_counting_refusing_transport(dials.clone()).await;
    let (_a, _b, _c) = (fava.observe(q()).await?, fava.observe(q()).await?, fava.observe(q()).await?);
    tokio::time::advance(Duration::from_secs(60)).await;
    assert!(dials.load(SeqCst) < 20); // today: ~3600
}
```

**Confidence.** confirmed.

---

### `handoff-outcome-not-attributable` — major — failure isolation

**Authority.** `docs/spec/ARCHITECTURE.md:1610` — "Every inbound frame and handoff completion
carries exact session generation and relay-access identity. Reconnected sessions are new
authorities." Contract at `:1572-1576` passes a `HandoffCorrelation` into `send`; `:1600-1602`
types the reasons as `TransportFailure` / `TransportAmbiguity`.

**Implementation.** `crates/fava-transport/src/lib.rs:46`:
`fn send(&self, frame: String) -> Pin<Box<dyn Future<Output = HandoffOutcome> + Send + '_>>;`
— no correlation parameter. And `:11-25`: no variant of `HandoffOutcome` carries a
`RelaySessionKey`, a generation, or a correlation; `NotHandedOff`/`Ambiguous` carry a bare
`String`. `next_message()` (`:49-51`) likewise returns a bare `String` with no generation.

**Is `HandoffOutcome` exhaustive?** As a three-way *classification* (definitely-not / definitely /
unknown) it is exhaustive and correctly used: `crates/fava-transport-websocket/src/lib.rs:105-130`
maps closed→`NotHandedOff`, oversize→`NotHandedOff`, sink error→`Ambiguous`, success→`HandedOff`,
and consumers match all three arms (`crates/fava/src/relay.rs:196-206`,
`crates/fava-transport-testkit/src/lib.rs:14-32`). **Is it attributable? No** — this is the defect.
Callers must read identity out-of-band via a separate `session.generation()` call
(`crates/fava/src/relay.rs:258, 330`). That read is not synchronized with the handoff, so on a
session that reconnects a completion can be filed under a generation it did not belong to —
precisely what RELAY-006 (`GOALS:1087`) forbids.

Secondary: `crates/fava/src/relay.rs:332` collapses `NotHandedOff` and `Ambiguous` into one
diagnostic arm, discarding the definitely-didn't-leave vs might-have-left distinction that
`ARCHITECTURE.md:1606-1608` says this boundary exists to preserve.

**Observable distinction.** A CLOSE that was definitely refused locally and a CLOSE whose fate is
unknown produce byte-identical diagnostics, so an application cannot tell whether a withdrawn
subscription is still consuming relay-side quota.

**Proposed falsifier.**
```rust
#[tokio::test]
async fn handoff_completion_names_its_own_session_generation() {
    let session = transport.open_session(key.clone()).await?;
    let outcome = session.send(frame, HandoffCorrelation::new(id)).await;
    assert_eq!(outcome.session(), &key);                 // no such accessor today
    assert_eq!(outcome.generation(), session.generation());
}
```

**Confidence.** confirmed.

---

### `inbound-frames-unbounded-by-declared-bound` — major — boundedness

**Authority.** `docs/spec/ARCHITECTURE.md:1623` — websocket transport owns "bounded reads **and**
writes". `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1420-1432` lists "frame and message
sizes" among resources Fava MUST bound.

**Implementation.** The declared bound is applied on the outbound path only —
`crates/fava-transport-websocket/src/lib.rs:110`:
`if frame.len() > self.max_frame_bytes.get() { return HandoffOutcome::NotHandedOff {...} }`.
`next_message()` (`:139-169`) applies no size check whatsoever, and the socket is built with
`connect_async(key.relay.as_str())` (`:66`) — the no-config constructor. A workspace grep for
`WebSocketConfig|max_message_size|max_frame_size|connect_async_with_config` returns **zero hits**,
so the only inbound ceiling is the tungstenite library default (64 MiB message / 16 MiB frame) —
an OS/library bound, not a Fava-owned one, and not the bound the application configured via
`WebSocketTransport::bounded`. The type's own doc comment at `:20` claims otherwise: "WebSocket
relay transport with **an exact text-frame size bound**".

**Observable distinction.** An application constructs `WebSocketTransport::bounded(4096)`
expecting a 4 KiB frame ceiling in both directions. A relay sends a 40 MiB EVENT frame; it is
accepted, fully allocated, decoded, and admitted. Nothing refuses it and no shortfall is reported.

**Proposed falsifier.**
```rust
#[tokio::test]
async fn inbound_frame_over_the_configured_bound_is_refused_not_allocated() {
    let t = WebSocketTransport::bounded(NonZeroUsize::new(4096).unwrap());
    let session = t.open_session(key).await?;
    relay.send_text("x".repeat(1 << 20)).await;          // 1 MiB, over the 4 KiB bound
    assert!(matches!(session.next_message().await, Err(TransportError::InvalidFrame(_))));
}
```

**Confidence.** confirmed.

---

### `wire-error-is-not-typed` — major — failure isolation / dependency direction

**Authority.** `docs/spec/ARCHITECTURE.md:330-337` — `decode_relay_frame(bytes) -> Result<RelayMessage, WireError>`
and `encode_client_message(..) -> Result<Bytes, WireError>`. `:345` — owned meaning includes
"typed malformed-frame errors".

**Implementation.** `crates/fava-wire/src/lib.rs:10` and `:19` both return
`Result<_, serde_json::Error>`. There is no `WireError` type in the crate — the whole crate is 21
lines and defines no type at all, only two `pub fn` wrappers plus a `pub use` of `nostr`.

**Is encode/decode total? Are malformed frames a typed refusal rather than a panic/silent drop?**
Totality: yes — both are pure `serde_json` calls with no `unwrap`/`expect`/slicing, so a malformed
frame cannot panic, and it is not silently dropped either (`crates/fava/src/relay.rs:106-116`
records a diagnostic). **Typed: no.** `serde_json::Error` is a third-party type that is not `Clone`
and not `Eq` (unlike every other Fava error in scope — `TransportError`
`crates/fava-transport/src/lib.rs:58` and `RelayIngestError` `crates/fava-ingest/src/lib.rs:11` are
both `Clone + Eq`), it cannot be matched on, and it conflates "not JSON at all", "JSON but not an
array", "unknown relay verb", and "known verb with a bad event object" into one opaque value that
consumers can only stringify (`crates/fava/src/relay.rs:112`). It also forces every consumer of
`fava-wire`'s public contract into `serde_json`'s error vocabulary, which fixes the encoding
implementation into the neutral contract.

**Observable distinction.** A relay speaking a newer NIP with an unrecognized-but-well-formed verb
and a relay emitting corrupt bytes produce the same untyped, unmatched diagnostic; an application
cannot filter one and alert on the other.

**Proposed falsifier.**
```rust
#[test]
fn malformed_and_unknown_relay_frames_are_distinct_typed_refusals() {
    assert!(matches!(decode_relay_frame(b"not json"), Err(WireError::Malformed { .. })));
    assert!(matches!(decode_relay_frame(br#"["FUTURE","x"]"#), Err(WireError::UnknownMessage { .. })));
}
```

**Confidence.** confirmed.

---

### `wire-has-no-length-or-text-bounds` — major — boundedness

**Authority.** `docs/spec/ARCHITECTURE.md:336-337` — `pub fn encoded_len(message: &ClientMessage) -> Result<usize, WireError>;`
`:346` — owned meaning: "exact byte length used for relay-advertised message limits".
`:347` — owned meaning: "**bounded preservation of relay-provided message text**".

**Implementation.** `crates/fava-wire/src/lib.rs` is 21 lines and contains neither. A workspace
grep for `encoded_len` finds it only in `crates/fava-nip02/src/{edit,bounds}.rs` — an unrelated
crate that had to build its own. There is no length accessor for relay-advertised NIP-11
`max_message_length`, and `decode_relay` (`:19`) preserves relay-supplied text unbounded.

**Any unbounded allocation from peer-controlled frame size? Yes.** `serde_json::from_str` at `:20`
allocates in proportion to the peer-chosen frame with no pre-check, and nothing upstream caps it
(see `inbound-frames-unbounded-by-declared-bound`). The retained-text consequence is concrete: an
unbounded `NOTICE`/`CLOSED` string flows from `decode_relay` into
`crates/fava/src/relay.rs:295` and `:302`, into `Diagnostics`, where `push_bounded`
(`crates/fava-diagnostics/src/lib.rs:215`) bounds the entry **count** to 256
(`:65`) but never the size of an entry. A relay sending 256 large `NOTICE` frames pins
256 × (frame size) bytes of relay-controlled text in Fava's retained diagnostics — the exact
"bounded preservation" that `:347` assigns to `fava-wire` and that no layer performs.

**Observable distinction.** An application reading `Diagnostics` after connecting to a hostile
relay observes process memory growing with relay-chosen message text and no shortfall or
truncation marker, contradicting `GOALS:1437` ("MUST NOT silently discard work while claiming
success" — here it silently *retains* instead).

**Proposed falsifier.**
```rust
#[test]
fn relay_supplied_message_text_is_preserved_within_an_exact_bound() {
    let frame = format!(r#"["NOTICE","{}"]"#, "n".repeat(1 << 20));
    let RelayMessage::Notice(text) = decode_relay_frame(frame.as_bytes()).unwrap() else { panic!() };
    assert!(text.len() <= fava_wire::MAX_RETAINED_MESSAGE_BYTES); // no such bound today
}
```

**Confidence.** confirmed.

---

### `ingest-owns-no-admission-order` — major — ownership

**Authority.** `docs/spec/ARCHITECTURE.md:2055` — "`fava-ingest` owns current ingress operation
identity and **serialized admission order**." `:2043` (owned lifecycle step 1) — "validate
relay-frame shape and bounds". Ordering rule `:3032` — relay ingest owns
`wire attribution -> event verification -> admitted occurrence`.

**Implementation.** `crates/fava-ingest/src/lib.rs` exposes exactly one stateless free function
(`:34`). It has no type, no state, no handle, and therefore **no ingress operation identity and
no serialization primitive** — the crate cannot order anything. Nor does it validate frame shape
or bounds (step 1): it receives an already-decoded `Event` and performs no size check.

This is not theoretical. The admission it delegates to is a non-atomic read-then-write:
`crates/fava-event-cache/src/lib.rs:19-28` reads `self.events()?`, computes
`admission_mutations`, then `self.commit(mutations)?`. Two relay sessions admitting concurrently
interleave between the read and the commit, so replacement/deletion decisions
(`fava-state` semantics) are computed against a stale snapshot. Serializing this is exactly the
ownership `:2055` assigns to `fava-ingest`, and there is no place in the crate for it to live.

**Observable distinction.** Two relays deliver two versions of the same replaceable event
(kind 30023, same `d` tag, `created_at` N and N+1) at the same instant. With serialized admission
the cache deterministically retains the newer one. Today both admissions can compute mutations
from the pre-existing snapshot and the retained winner depends on commit interleaving — the
application can observe the *older* revision surviving.

**Proposed falsifier.**
```rust
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_relay_admissions_of_one_replaceable_coordinate_are_serialized() {
    let ingest = RelayIngest::new(cache.clone());                    // no such owner today
    let (older, newer) = (addressable(1000), addressable(1001));
    tokio::join!(ingest.admit(sess_a(), newer.clone()), ingest.admit(sess_b(), older));
    assert_eq!(cache.event(newer.id).unwrap().unwrap().event, newer); // flaky/false today
}
```

**Confidence.** confirmed.

---

### `ingest-emits-no-committed-change` — major — behavioral proof

**Authority.** `docs/spec/ARCHITECTURE.md:2047-2050` — owned lifecycle steps 5-8:
"construct `VerifiedRelayEvent`" … "commit the decision through `EventCache`" … "emit
`CommittedCacheChange` and per-relay evidence". The `EventCache` contract in the same document
(`:801`) is declared as `) -> Result<CommittedCacheChange, EventCacheError>;`.

**Implementation.** `crates/fava-ingest/src/lib.rs:42` returns `Result<bool, RelayIngestError>`,
and `crates/fava-event-cache/src/lib.rs:19` is `fn admit(&self, ...) -> Result<bool, EventCacheError>`.
Workspace grep for `CommittedCacheChange` and `VerifiedRelayEvent` across `crates/`: **zero hits**
— neither noun exists anywhere in the implementation. The sole production caller discards even the
`bool` (`crates/fava/src/relay.rs:269` binds only the `Err` arm), so an admitted live event
produces **no emitted fact at all** — which is why nothing downstream can be woken by it.

**Observable distinction.** An application holding an open live query cannot learn *which*
coordinate a relay event affected, or whether admission replaced/deleted an existing event, or
which relay supplied the evidence — the ingest path returns an unattributed boolean that is then
dropped on the floor.

**Proposed falsifier.**
```rust
#[test]
fn admitting_a_relay_event_emits_an_attributed_committed_change() {
    let change = admit_subscription_event(&cache, &session(), &plan, event, now).unwrap();
    assert_eq!(change.relay(), &session());
    assert_eq!(change.affected_coordinate(), Some(coordinate)); // no CommittedCacheChange today
}
```

**Confidence.** confirmed.

---

### `event-cache-admit-bypasses-ingest` — major — replaceability / ownership

**Authority.** `docs/spec/ARCHITECTURE.md:2966` (ownership ledger) — "Event-id/signature admission |
`fava-ingest` | observations, cache, publication reconciliation". `AGENTS.md` gate 3: "defaults
have no private bypass". `docs/spec/ARCHITECTURE.md:2029` — `fava-ingest`'s responsibility is to
"turn untrusted relay frames into committed event-cache facts".

**Is `fava-ingest` genuinely the single authority for cache admission?** *In production, yes —
with a caveat.* The workspace grep for `\.admit(` returns six call sites:
`crates/fava-ingest/src/lib.rs:53` (the authority) and five in test code
(`crates/fava/tests/source_contract.rs:83`, `crates/fava/tests/local_source_merge.rs:283,302`,
`crates/fava/tests/support/semantic_write_capability_lifecycle.rs:125,230`). **No production crate
other than `fava-ingest` admits directly.** That part conforms and I state it as a positive result
of a search that ran.

The finding is that the boundary is unenforced rather than violated: `EventCache::admit` is a
public trait method **with a default body** (`crates/fava-event-cache/src/lib.rs:19-28`) that
performs signature verification and commit on its own. Any crate holding `&dyn EventCache` — and
several do, including `crates/fava/src/relay.rs:22` — can commit relay-shaped state without
attribution or filter checks, and five call sites already do. The ledger's single-authority claim
therefore rests on convention, not on the contract.

**Observable distinction.** A third-party protocol crate (the replaceability case the architecture
is built around) can be given the same `Arc<dyn EventCache>` the engine uses and insert events
that no accepted subscription ever attributed. Queries then return events with `RelayEvidence`
for a relay that never sent them.

**Proposed falsifier.**
```rust
#[test]
fn cache_admission_requires_ingest_attribution() {
    // EventCache::commit stays public; admit(CachedEvent, ..) must not be callable
    // without a fava-ingest-issued attribution token.
    let token = ingest.attribution_for(&session, &subscription).unwrap();
    cache.admit(token, cached_event, now).unwrap();   // today: no token parameter exists
}
```

**Confidence.** confirmed (as an unenforced boundary; no production bypass found).

---

### `testkit-ships-no-relay-fake` — major — behavioral proof

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1439-1450` (OPS-005,
"Application-facing test infrastructure is part of the product") — Fava MUST ship supported test
facilities for "scripted relay frames and protocol misbehavior" and "connection failure and
reconnect". `docs/spec/ARCHITECTURE.md:3654` lists `fava-transport-testkit` among the shipped
conformance kits; `:3661` — "Each conformance kit is versioned with its contract and can be used
by external provider crates."

**Implementation.** `crates/fava-transport-testkit/src/lib.rs` is 56 lines containing four
assertion helpers (`require_handoff_success`, `require_handoff_refusal`, `require_disconnect`,
`require_idempotent_close`). It ships **no scripted relay, no fake `Transport`, no
connection-failure or reconnect harness**. Its only consumer is
`crates/fava-transport-websocket/tests/conformance.rs`, which hand-rolls a raw `TcpListener` in
every test (`:16-23`).

The consequence is measurable: a grep for `impl Transport for` finds **eleven** independently
hand-written fakes across the workspace —
`ScriptedTransport` (`crates/fava/tests/multi_relay.rs:50`, `crates/fava/tests/explicit_live.rs:107`),
`RecordingTransport` (`crates/fava/tests/automatic_routes.rs:61`),
`SpyTransport` (`crates/fava/tests/simple_groups.rs:773`),
`PendingTransport` / `FirstOpenThenPendingTransport` (`crates/fava/tests/explicit_live.rs:57,78`),
and five separate `NoopTransport`s (`crates/fava/tests/automatic_publication.rs:206`,
`crates/fava/tests/write_settlement.rs:474`, `crates/fava/tests/explicit_publication.rs:271`,
`crates/fava/tests/support/semantic_write.rs:411`,
`crates/fava-write-store-redb/tests/process_kill/semantic.rs:428`).
None is available to an application or to an external provider author, and none of them is
reconnect-capable — which is a direct reason the reconnect requirements (QUERY-015 `GOALS:483-489`,
RELAY-006 `:1085-1089`) have no falsifier today.

**Observable distinction.** An application (or a third-party `Transport` implementor) that depends
on `fava-transport-testkit` cannot write a test for reconnect-restores-demand or for relay
protocol misbehavior, because the crate exposes nothing that can script a frame or fail a
connection. That is a shipped-product gap, not an internal one.

**Proposed falsifier.**
```rust
#[tokio::test]
async fn testkit_scripts_a_relay_and_a_connection_failure() {
    let relay = fava_transport_testkit::ScriptedRelay::new();   // does not exist
    relay.fail_next_open(TransportError::ConnectionRefused("down".into()));
    relay.then_accept().push(r#"["EOSE","one"]"#);
    fava_transport_testkit::require_reconnect_restores_demand(&relay).await.unwrap();
}
```

**Confidence.** confirmed.

---

### `websocketrelaysession-unapproved-lifecycle-noun` — minor — vocabulary

**Authority.** `AGENTS.md` vocabulary policy — a new "**lifecycle owner**" is a vocabulary change;
`docs/internals/vocabulary.toml` is the source of truth. The `Transport` term
(`docs/internals/vocabulary.toml:696-720`) enumerates its approved symbols at `:703-707`:
`fava_transport::{HandoffOutcome, RelaySession, Transport, TransportError}` and
`fava_transport_websocket::WebSocketTransport`.

**Implementation.** `crates/fava-transport-websocket/src/lib.rs:82` declares
`struct WebSocketRelaySession` — the sole owner of the socket sink, the socket stream, the
connection generation, the frame bound, and the `closed` flag (`:83-89`), i.e. the entire physical
relay-session lifecycle. It is not listed in `vocabulary.toml`. Because it is private,
`tools/check_vocabulary.py` (which the brief notes only scans `pub struct|enum|trait|type`) cannot
see it — this is the gate hole the brief asked me to look through, and this is what is behind it
in my area. I searched the other four crates for the same shape and found no other private
lifecycle owner: `fava-transport`, `fava-wire`, `fava-ingest`, and `fava-transport-testkit` declare
no private structs at all.

Related, same file: `docs/internals/vocabulary.toml:716` lists `fava-relay-lab` as a `spec_crate`
for the `Transport` term, but `docs/spec/ARCHITECTURE.md:3659` states explicitly: "no
`fava-relay-lab` crate is created." The vocabulary file names a crate the authority forbids.

**Observable distinction.** Weak by itself (a private type is not externally visible). It is
reportable because it is the *named owner* of every transport fact the architecture assigns, so it
is the type that must appear in the ledger when transport is corrected — and today the vocabulary
gate certifies the crate as clean while its only lifecycle owner is unregistered.

**Proposed falsifier.**
```python
# tools/check_vocabulary.py — extend the scan
def test_private_lifecycle_owners_are_registered():
    owners = scan(r"^\s*(pub\(crate\)|pub\(super\)|)\s*struct (\w*(Session|Transport|Relay))")
    assert "WebSocketRelaySession" in registered_symbols()   # fails today
```

**Confidence.** confirmed.

---

## Conforming (verified, not merely unexamined)

These were checked against the authority and found correct. Each rests on a search or a full read
that actually ran.

- **`fava-ingest` is the only production cache admitter.** Grep `\.admit(` over all of `crates/`
  returns six sites; the one non-test site is `crates/fava-ingest/src/lib.rs:53`. No other
  production crate writes to the event cache. (See `event-cache-admit-bypasses-ingest` for the
  unenforced-contract caveat, which does not contradict this result.)
- **`fava-ingest` refusal ordering is correct and pre-cache.** `crates/fava-ingest/src/lib.rs:43-51`
  checks subscription, then `event.verify()`, then `filter.match_event`, and only then calls
  `cache.admit`. A forged or off-filter event cannot reach the cache, and
  `crates/fava-ingest/tests/admission.rs:66` proves the cache stays empty across all three refusals.
  This matches `ARCHITECTURE.md:2044-2047` steps 2-4 in order.
- **`RelayIngestError` is a typed, `Clone + Eq` refusal** (`crates/fava-ingest/src/lib.rs:11-25`)
  with a transparent `#[from] EventCacheError` for provider failure — no stringly-typed collapse,
  and provider refusal stays attributable to the provider.
- **`fava-wire` encode/decode is total.** Full read of all 21 lines: two `serde_json` calls, no
  `unwrap`, `expect`, `panic!`, indexing, or slicing. A malformed frame cannot panic. It is also not
  silently dropped — `crates/fava/src/relay.rs:106-115` records a diagnostic and returns.
  (The *type* of the error is the finding, not its existence.)
- **`HandoffOutcome`'s three-way classification is exhaustive and correctly produced.**
  `crates/fava-transport-websocket/src/lib.rs:105-130` covers closed, oversize, sink-error, and
  success; every consumer matches all three arms with no catch-all that could swallow a variant
  (`crates/fava/src/relay.rs:196-206`, `crates/fava-transport-testkit/src/lib.rs:14-32`).
- **`close()` is genuinely idempotent** via `self.closed.swap(true, SeqCst)`
  (`crates/fava-transport-websocket/src/lib.rs:177`), and post-close handoff is a definite
  `NotHandedOff` (`:105-109`) rather than an ambiguous one. Proved through the public path by
  `crates/fava-transport-websocket/tests/conformance.rs:83-96`.
- **Outbound frame size is bounded and refused, not truncated.** `:110-118` returns
  `NotHandedOff` with the exact size and the exact bound in the reason, satisfying `GOALS:1437`
  for the outbound direction. Proved at `tests/conformance.rs:47-63`.
- **Generation allocation cannot silently wrap.** `fetch_update(.., |c| c.checked_add(1))`
  (`crates/fava-transport-websocket/src/lib.rs:55-65`) converts exhaustion into
  `TransportError::ConnectionRefused` rather than overflowing — a correct typed refusal at a bound.
- **Inbound control frames do not leak as data.** `Message::Binary` becomes
  `TransportError::InvalidFrame` (`:151-155`) rather than being lossily stringified, and
  `Ping`/`Pong`/`Frame` are skipped without terminating the read loop (`:156`).
- **Remote close and stream end are reported as `Disconnected`, never as EOSE or emptiness**
  (`:147-166`), satisfying `GOALS:424`. Proved at `tests/conformance.rs:65-80`.
- **Dependency direction in all five crates is legal.** `fava-transport` depends only on
  `fava-state` + `thiserror`; `fava-wire` only on `nostr` + `serde_json`; `fava-ingest` on
  `fava-event-cache`/`fava-state`/`nostr`. No contract crate depends on a provider crate, and
  `fava-transport-websocket` depends on `fava-transport` and not the reverse.

## Open questions

1. **Where should the reconnect/backoff state machine physically live?**
   `ARCHITECTURE.md:1589` and `:1625` put backoff in transport, but RELAY-006 (`GOALS:1089`)
   requires "active logical demand is replayed automatically after reconnect", and the demand is
   owned by `fava-observe`. Transport cannot replay a plan it does not know. `:1627`
   ("transport-level replay hooks for current subscription plans") implies transport holds a
   *handle* to the current plan supplied by observe. The exact shape of that hook is not specified
   and I could not settle it from the authority alone. Flagging it because
   `relay-session-trait-cannot-multiplex` and `unbounded-reconnect-storm` cannot both be fixed
   without deciding it.
2. **Does `HandoffCorrelation` need to be added to `vocabulary.toml`, or is it intentionally
   dropped?** The architecture contract names `HandoffCorrelation`, `RelaySessionIdentity`,
   `OpenRelaySession`, `TransportFailure`, `TransportAmbiguity`, and `RelayMessageStream`
   (`ARCHITECTURE.md:1565-1602`), but `vocabulary.toml:714` approves only
   `spec_symbols = ["HandoffOutcome", "RelaySession", "Transport"]`. Either the vocabulary is
   behind the architecture or the architecture's contract sketch is illustrative. Authority order
   puts ARCHITECTURE.md above `AGENTS.md`/vocabulary, which argues the vocabulary is behind — but
   this affects how `handoff-outcome-not-attributable` should be fixed, so it needs a ruling.
3. **Should `fava-wire` decode from `&[u8]` rather than `&str`?** Spec `:330` says `bytes: &[u8]`;
   the implementation takes `&str` because tungstenite has already validated UTF-8. The spec shape
   matters if a future transport delivers unvalidated bytes. Not reported as a finding — no
   observable distinction exists today through the current transport.
4. **`vocabulary.toml:716` lists `fava-relay-lab`, which `ARCHITECTURE.md:3659` forbids.**
   Noted inside `websocketrelaysession-unapproved-lifecycle-noun`; it may belong to whichever area
   owns the vocabulary file rather than to this area.
