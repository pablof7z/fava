## Context

See `proposal.md` — Why. The constraints that shape the approach:

- Six `RelaySession::send` call sites exist, in three crates, each building and encoding its own envelope: `crates/fava-auth/src/authenticator/answer.rs:187`, `crates/fava-observe/src/operations.rs:282` (fed by `ClientMessage` values at `:134`, `:167`, `:209`), and `crates/fava-publisher-nip01/src/lib.rs:103`, `:191`, `:201`, `:299`.
- `HandoffCorrelation` is minted by all three and read by none. Production code matches on the `HandoffOutcome` variant and drops the correlation the outcome carries. Only `crates/fava-transport-testkit/src/lib.rs:156` asserts it echoes, and `crates/fava-transport-testkit/src/session.rs:183`, `:240` use it to track frames the fake strands on disconnect.
- `pump` (`crates/fava-transport-websocket/src/driver.rs:137`) is the only code polling the socket, and it both writes queued outbound frames and reads inbound ones in one `select!` inside one task. `admit_frame` (`:258`) is the single point every inbound frame passes, right after the `max_frame_bytes` check.
- `fava_wire::decode_relay` returns an owned `RelayMessage<'static>`, and `nostr::message::RelayMessage` derives `Debug, Clone, PartialEq, Eq, Ord, Hash`, so `RelayInbound`'s existing derives survive unchanged.
- Per-consumer bounded queues with exact typed loss (`crates/fava-transport-websocket/src/fanout.rs`) are load-bearing: one consumer must never park the reader or remove another's item.
- Subscription identifiers are minted by the planner today: `crates/fava-subscriptions-standard/src/wire.rs:32` produces `<prefix>-<revision>-<ordinal>`, unique within a plan and across plans of one session, and hands it to `fava-observe` inside the plan. Two things depend on the planner knowing the identifier before it sends: it checks the identifier against the relay's NIP-11-declared maximum and raises `ShortfallReason::SubscriptionIdTooLong` (`crates/fava-subscriptions-standard/src/lib.rs:277`, `crates/fava-subscriptions-no-grouping/src/lib.rs:224`, exercised at `crates/fava-subscriptions/src/conformance.rs:278`), and it mints a probe identifier to measure a `REQ`'s encoded length while grouping (`crates/fava-subscriptions-standard/src/lib.rs:145`).
- `.planning/REQUIREMENTS.md` OWN-02 reserves retained logical demand and the desired wire-subscription plan to `fava-observe`; OWN-03 reserves establishment, generation, reconnect, backoff, and close to the transport; SUB-05 requires that replaceable implementations use only public contracts.

## Goals / Non-Goals

**Goals:**

- Envelope construction and encoding exist in exactly one place.
- One decode per frame, at the reader.
- One owner per wire key, established by the act of sending, not by a second registration that can drift from it.
- Three landings that each compile and pass green, so the outbound move, the inbound move, and the deletions are separately reviewable and revertible.

**Non-Goals:**

- Moving subscription-to-observation attribution into the transport. `Slot.installed` maps a wire subscription to the observations that own it; that is semantic ownership and stays in `fava-observe`.
- Verbs for client messages Fava does not send. `ClientMessage` has eight variants; Fava sends `EVENT`, `REQ`, `CLOSE`, and `AUTH`. `COUNT` and the three negentropy messages get verbs when Fava sends one, not before.
- Changing session establishment, generation minting, reconnect, backoff, or close.
- Grouping, planning, or filter semantics.

## Decisions

### The verb is the claim

`route-relay-messages`, which this change supersedes, registered an interest explicitly and then sent a frame naming the same key. Two steps that must agree, with a silent failure mode when they do not: a `REQ` sent without its claim becomes unclaimed traffic and the observation appears to hang.

A verb removes the second step. `req(filters)` mints and therefore knows the identifier; `event(signed)` knows the event id because it is building the envelope from it. The claim is registered by the same code that emits the frame, in the same call, so they cannot disagree.

The earlier rejection of "infer the claim from the outbound frame" was against *parsing* frames the transport only writes. Constructing is not parsing. That objection does not survive the verbs.

### Verbs return handles, and each handle has its own narrow type

```rust
// Proposed API. Exact names and error types are task 2.1's business.
pub trait RelaySession: Send + Sync {
    fn identity(&self) -> RelaySessionIdentity;
    fn req(&self, filters: Vec<Filter>) -> ReqFuture<'_>;    // -> Subscription
    fn event(&self, signed: Event) -> EventFuture<'_>;       // -> Acknowledgement
    fn auth(&self, signed: Event) -> EventFuture<'_>;        // -> Acknowledgement
    fn auth_challenges(&self) -> Box<dyn ChallengeStream>;
    fn close(&self) -> ReleaseFuture<'_>;                    // closes the session
}

pub enum SubscriptionItem { Event(Box<Event>), EndOfStoredEvents, Closed { reason }, Lost { dropped }, Ended { reason } }
pub enum Settlement       { Accepted { message }, Rejected { message }, Ended { reason } }
```

Each `await?` reports the handoff outcome of the frame the verb built — definitely not sent, sent, or unprovable — and on success yields the handle.

The narrow types are the point. A shared inbound enum forces every consumer to write arms for variants that cannot reach it, which is how three crates each ended up with a hand-written filter. A subscription can only ever see its own traffic, its closure, its loss, or its generation ending; an acknowledgement can only settle three ways. `Ended` carries whether the connection dropped or the reconnect budget ran out, because `.planning/REQUIREMENTS.md` HARD-07 requires an ambiguous publication to name which.

This is also why `HandoffCorrelation` goes. It exists so a caller can recognize its own completion among many, but `send` is already `async` and returns exactly one completion to exactly one awaiter. Nothing in production reads it. Removing it deletes `fava-observe`'s `1_u64..` counter (`operations.rs:132`), `fava-auth`'s generation-derived token (`answer.rs:186`), and the publisher's attempt-number reuse.

Alternative considered: keep `send(ClientMessage)` — typed, so nobody encodes JSON, but callers still build envelopes. Rejected: it closes the encoding leak and leaves the construction leak, which is the one that matters, and a bare `ClientMessage` gives the session no place to put the handle.

### `messages()`, `RelayInbound`, and `RelayMessageStream` are deleted, not narrowed

An earlier draft kept `messages()` as a session-wide stream for lifecycle transitions and unclaimed messages. Once challenges have their own accessor and every handle carries its own lifecycle, what remains is a notice, a message naming a subscription nobody holds, an acknowledgement for an event nobody sent, and bytes that do not parse. None of that is a stream a component consumes — it is a counter with a bounded reason, and it belongs in `fava-diagnostics`.

Keeping the stream would have preserved the failure it exists to prevent: a component would `match` challenges out of a mixed feed, which is a hand-written filter, which is what this change removes everywhere else.

The counter still matters. It is the only signal that traffic arrived for something nobody holds, which would otherwise present as a silent hang.

### The session mints the subscription identifier

Whoever knows every identifier currently in use is the one that can guarantee a new one does not collide. That is the session: it holds every live handle. The planner knows only its own plan.

Today a collision is already structurally impossible for Fava-minted identifiers — `<prefix>-<revision>-<ordinal>` is unique within a plan and never repeats across plans of one session — so this is not fixing a live bug. It matters because SUB-05 lets a third party replace the planner, and because with verbs any component can call `req`. Moving the mint makes the guarantee a property of the connection rather than a convention each planner must re-establish.

Identifiers are opaque and fixed-width: a namespace prefix and a counter, inside the 64 characters NIP-01 obliges every relay to accept. Three things follow. A collision is impossible by construction. The planner measures a `REQ`'s exact encoded length from the declared width, replacing the probe mint at `crates/fava-subscriptions-standard/src/lib.rs:145`. And `ShortfallReason::SubscriptionIdTooLong` becomes unreachable and is deleted with both checks that raise it.

No open-time check against the relay's declared maximum. An earlier draft had one; it cannot be built. NIP-11 is fetched by `fava-observe` in its own task after the session is acquired (`crates/fava-observe/src/operations.rs:312`) and reported into its slot; the transport has no HTTP client and never sees it. `nip11_fetch` attempts only plain `ws://`, so every `wss://` relay's declared limit is already `unknown()` in production — which is also why the shortfall being deleted is already unreachable there. A relay declaring less than the guaranteed 64 is out of spec; if it rejects a subscription it sends `CLOSED`, and the existing refusal path carries that.

The plan revision and ordinal stop appearing on the wire, and that costs nothing worth recovering. Nothing parses them: the only reads of an identifier's text are the two length checks being deleted. Nothing outside the process can interpret them either, because the revision's authority comes from a process-local counter (`static NEXT_PLAN_AUTHORITY: AtomicU64 = AtomicU64::new(0)`, `crates/fava-subscriptions/src/plan.rs:56`) that restarts every run — so a relay operator cannot report an identifier and have anyone map it back, and the same identifier means different things across two runs.

Alternative considered: let the caller pass a fixed-width label the session embeds, preserving today's text. Rejected — a parameter and a type on the most-used verb, plus a truncate-don't-refuse rule cutting against the convention `fava-auth`'s `Challenge` sets, all to serve a reader that does not exist.

Alternative considered: keep the planner minting and have the session refuse a duplicate. Rejected because it keeps a failure mode that need not exist.

### The planner proposes; the executor names

Session-minted identifiers meet the planner contract at one point: `SubscriptionPlan` is keyed by `SubscriptionId` throughout. `retain` and `close` survive untouched, because they name subscriptions `fava-observe` already holds and the planner already receives (`installed` is an argument to `plan`). Only `open` loses its name, because the name does not exist until the frame is sent.

That would have forced a stand-in — an index into `plan.open` for `WithdrawalReason::Regrouped` to point at, since it names a successor that has not been opened yet. It does not, because `Regrouped` is dead. `grep -rn "Regrouped"` finds it constructed only in two hand-written test plans (`crates/fava-subscriptions/tests/running.rs:81`, `:191`); both planners' `withdrawals()` emit `ConstraintChanged` or `DemandWithdrawn` and nothing else (`crates/fava-subscriptions-standard/src/diff.rs:104`, `crates/fava-subscriptions-no-grouping/src/lib.rs:307`). Those two are never branched on either — `facts.rs:95` only `Debug`-prints them into a diagnostic string.

So `WithdrawalReason` goes entirely and `close` becomes `Vec<SubscriptionId>`. The property the enum documented — open before close, so a replan never leaves demand served by neither subscription — is kept and derived instead of declared: close a withdrawn subscription once every demand it served is either gone from demand or covered by a subscription that actually opened. No plan names a subscription that does not exist, so no stand-in identifier type is needed.

Nothing that fires today stops firing. The standard planner already refuses to rewrite a running subscription: `diff.rs:84` says the last owner leaving is the only reason to close one, and a subscription that keeps an owner "is retained unchanged, even when it is now broader than what its remaining owners asked for". `admission.rs` gives the reason — rewriting costs the relay a full re-serve of the window it already served, quadratic in growth steps, and "is never taken". Both properties that matter are elsewhere and untouched: the 10ms first-arrival-anchored admission cohort that compiles simultaneous demand into one REQ, and `filter_covers`, which lets later demand attach to a live subscription rather than open a second one.

With the forward reference gone, `PlannedSubscription` can drop its `id` and absorb `EoseCompleteness`, becoming its own attribution. `SubscriptionAttribution` then covers only the retained set, where identifiers exist. `PlannedSubscription` and `AttributedSubscription` carry the same filters and demand today, cross-checked by conformance C5 and C7 precisely because they are two records of one fact; merging them deletes the check along with the duplication.

The differential testkit needs no work. `assert_planners_agree` compares `DemandId` sets only — shortfalls, per-event delivery, and EOSE settlement (`crates/fava-subscriptions-testkit/src/differential.rs:56-100`) — and never compares a wire identifier across planners. Its premise is already that two planners name subscriptions differently.

### No exclusivity rule

Subscription identifiers cannot collide, so nothing needs enforcing there. Acknowledgements fan out: two callers publishing the same signed event each get a handle and each sees the relay's `OK`. Refusing the second would be a guard against a state that is not actually invalid — the two callers want the same answer, and the relay will give it.

### Closing lives on the subscription handle, and releasing still closes

`close(id)` on the session would mean reading the identifier off the handle to hand it back to the thing that issued it. Worse, if releasing a handle only stopped routing, the relay would keep streaming into the unclaimed counter for the life of the connection — a leak that looks like a bug in the counter.

So closing is `Subscription::close()`, which sends the closure and awaits its handoff outcome, and `Drop` enqueues the same closure without awaiting. `send` already enqueues with `try_send` on a bounded channel and only then awaits a completion oneshot (`crates/fava-transport-websocket/src/session.rs:88`), so `Drop` performs the half that does not need `await`. A handle dropped after its generation advanced enqueues nothing: the relay did not carry the subscription across the connection.

This also gives `fava-observe` a better shape than it has. Today it gathers identifiers into a `Vec` and closes them by hand in two places (`crates/fava-observe/src/operations.rs:167` and `:209`); `Slot.installed` holds handles instead, and dropping a slot closes what it held.

### Routing follows the wire key, with a fixed mapping

| Relay message | Delivered to |
| --- | --- |
| `EVENT`, `EOSE`, `CLOSED`, `COUNT`, `NEG-MSG`, `NEG-ERR` | the handle holding its subscription id |
| `OK` | every handle awaiting that event id |
| `AUTH` | the challenge accessor |
| `NOTICE`, anything unclaimed, undecodable bytes | counted, delivered to nothing |

`CLOSED` reaches the subscription's handle and does not itself release the key. Release is the holder's act; the transport must not decide that a relay's refusal ends a component's interest, because the holder may want to open again.

### The delivered event is moved, not copied

With one owner per subscription and fan-out only for acknowledgements — which carry a short verdict, not an event — a decoded `EVENT` has exactly one destination and can be moved into it rather than cloned. This is strictly cheaper than today, where each consumer clones the bytes and then decodes them independently.

### `fanout.rs` becomes the router; the queue model is untouched

Each handle keeps a bounded `VecDeque`, a `dropped` counter, a `Notify`, and a detach flag. The router decides which queue an item enters. Backpressure, typed loss, per-handle ordering, and the reader's independence from any consumer are preserved by construction rather than re-argued.

## Risks / Trade-offs

**The transport learns NIP-01 and keeps learning.** → Add `crates/fava-transport/tests/architecture.rs` on the `crates/fava-nip02/tests/architecture.rs` model, pinning the dependency set exactly and asserting no filter, demand, plan, subscription-planning, or observation type appears in its manifest or source. Permitted protocol knowledge is the four verbs and the routing table above.

**OWN-02 reserves the wire-subscription plan to `fava-observe`.** → The session holds no filters beyond encoding one `REQ` frame, no retained demand, and no plan revision, and it discards every key on reconnect. Minting an identifier names one connection's wire state; the planner still decides which subscriptions exist and what each carries. `fava-observe` keeps `Slot.installed`, `Slot::advance`, and `owners()` unchanged, keyed by the identifier the session returns.

**Removing `send` blocks a third party who needs `COUNT` or negentropy.** → Accepted, and it is the change's one lasting ergonomic cost: adding a client message Fava does not send now means touching `fava-transport`. That is a small change in one crate; an escape hatch is permanent. Recorded as a non-goal so the next person adds the verb rather than reopening the hatch.

**`HandoffCorrelation` removal reaches the testkit fake, which uses it to strand in-flight frames on disconnect** (`crates/fava-transport-testkit/src/session.rs:183`, `:240`). → The fake keys stranded work on the awaiting handle instead. This is test infrastructure, not contract, and stage 1 lands before any behavior changes.

**Ten `impl Transport` sites plus the conformance suite change.** → Stage 1 is outbound-only and mechanical; stage 2 is inbound-only. Neither carries behavior.

**`Nip42Publisher` and `fava-observe`'s `RelayMessage::Auth` branch are being deleted by `own-relay-authentication`.** → This change touches neither, and its tasks name only `Nip01Publisher`. Sequence after that change or rebase onto it; do not resolve a conflict by reintroducing either.

## Migration Plan

**Stage 1 — verbs out.** `RelaySession` gains `event`, `auth`, `req`, `close`, each building and encoding what `send` was given. Each returns today's `HandoffOutcome`, minus the correlation. `send` and `HandoffCorrelation` are deleted, along with the three caller-side correlation counters. All six call sites and the ten `Transport` impls move over. Inbound is untouched — consumers still take a whole-session stream and still scan. No behavior changes.

**Stage 2 — routed messages in.** `admit_frame` decodes; `fanout.rs` becomes the router; the verbs start returning handles with their own narrow item types; `auth_challenges()` arrives; `messages()`, `RelayInbound`, and `RelayMessageStream` are deleted, and unclaimed and undecodable traffic becomes a diagnostics counter. All three consumers delete their UTF-8 and JSON handling.

**Stage 3 — deletions and documentation.** `Nip01Publisher`'s scan loop, `MAX_INBOUND_FRAMES`, and its `NOTICE`, `AUTH`, and decode-error branches go. `require_inbound_fan_out` is replaced by verb conformance. `ARCHITECTURE.md` is amended.

Rollback is per stage. No stage changes a persisted format, so there is no data migration and no schema bump.
