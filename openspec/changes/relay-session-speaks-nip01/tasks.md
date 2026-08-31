## 1. Stage 1 — the session owns the envelope

- [x] 1.1 Add `fava-wire` to `fava-transport`'s manifest; verify `cargo build --workspace --all-targets --locked` succeeds and `cargo tree -p fava-transport` shows no cycle
- [x] 1.2 Add `RelaySession::req`, `event`, and `auth`, each taking protocol values, building the `ClientMessage`, encoding it, and enforcing `max_frame_bytes` before anything reaches the socket; verify unit tests assert the exact frame each verb produces for a known input and that an over-bound request is refused with a typed error and writes nothing
- [x] 1.3 Mint the subscription identifier inside `req` at a fixed declared width within NIP-01's guaranteed 64 characters, opaque and carrying no plan information, returning it alongside the handoff outcome and exposing the width on the session; verify tests assert two simultaneously live subscriptions never share an identifier and that the returned identifier is the one the frame carried
- [x] 1.4 Remove `HandoffCorrelation` and the `correlation` field from all three `HandoffOutcome` variants; verify the three variants stay distinct in a test and that `HandoffOutcome::identity()` still names the generation the attempt was made against
- [x] 1.5 Remove `RelaySession::send`; verify a grep for `encode_client` and `ClientMessage` outside `fava-transport*` and `fava-subscriptions-*` returns nothing
- [x] 1.6 Delete `WithdrawalReason` and `WithdrawnSubscription`, collapsing `SubscriptionPlan::close` to `Vec<SubscriptionId>`, and delete `PlanConformanceError::UnknownSuccessor` with the C5c rule at `crates/fava-subscriptions/src/conformance.rs:184`; verify a grep for `Regrouped` returns nothing outside historical records and both planners still compile with their `withdrawals()` returning bare identifiers
- [x] 1.7 Close a withdrawn subscription in `fava-observe` once every demand it served is gone from demand or covered by a subscription that actually opened, replacing the successor guard at `crates/fava-observe/src/operations.rs:160`; verify a test asserts a predecessor stays live when the subscription meant to take over its demand fails to open, and closes when it succeeds
- [x] 1.8 Retire the two hand-built `Regrouped` plans at `crates/fava-subscriptions/tests/running.rs:81` and `:191`, replacing what they proved with the coverage-derived rule from 1.7; verify the replacements fail when 1.7's rule is inverted
- [x] 1.9 Drop `id` from `PlannedSubscription` and absorb `EoseCompleteness` into it, narrowing `SubscriptionAttribution` to the retained set; verify the observe attribution and completeness tests pass and a planned subscription is its own attribution
- [x] 1.10 Delete `SubscriptionPlanError::DuplicateSubscription` and the C2 `OverlappingBuckets` and C3 `ReopenedInstalled` conformance rules, and delete C7 `FilterAttributionMismatch` with the opened half of C5; verify a grep for each removed variant is empty and the remaining conformance rules still fail on the plans they are meant to reject
- [x] 1.11 Delete `wire::mint` from `fava-subscriptions-standard` and its counterpart in `fava-subscriptions-no-grouping`, and delete `ShortfallReason::SubscriptionIdTooLong` with the two checks that raise it and the conformance case at `crates/fava-subscriptions/src/conformance.rs:278`; verify a grep for the removed variant is empty
- [x] 1.12 Measure a `REQ`'s encoded length from the session's declared identifier width in both planners, replacing the probe mint at `crates/fava-subscriptions-standard/src/lib.rs:145`; verify a test asserts the measured length equals the exact length of the frame `req` produces for the same filters
- [x] 1.13 Move `fava-observe`'s `REQ` and `CLOSE` sends to the verbs, keying `Slot.installed` by the identifier `req` returned, and delete its `correlations = 1_u64..` counter; verify the existing subscription install and close tests pass apart from the identifier text
- [x] 1.14 Move `fava-auth`'s `AUTH` send to the verb and delete its generation-derived correlation; verify the existing NIP-42 tests pass with the same wire transcript
- [x] 1.15 Move `fava-publisher-nip01`'s `EVENT` and `AUTH` sends in both publishers to the verbs; verify `crates/fava-publisher-nip01/tests/nip42.rs` passes with the same wire transcript
- [ ] 1.16 Confirm the 10ms first-arrival-anchored admission cohort and `filter_covers` attachment are untouched; verify a test asserts two equal demands arriving inside one window produce one REQ, and that demand arriving after the freeze which an incumbent covers opens no second REQ
- [x] 1.17 Rework the `fava-transport-testkit` fake and the nine other `impl Transport` sites to answer verbs, keying stranded-on-disconnect work on the awaiting caller rather than on a correlation; verify `cargo test --workspace --locked` passes

## 2. Stage 2 — handles, and their own types

- [ ] 2.1 Decode in `driver::admit_frame` right after the `max_frame_bytes` check, keeping the frame's byte length available; verify a test asserts one decode per frame across a session carrying several live subscriptions and a pending publication
- [ ] 2.2 Add the subscription handle and its item type — event, end of stored events, closure, exact loss, generation ended — and return it from `req`; verify a test opens two subscriptions and asserts each handle receives only its own event, end-of-stored-events, and closure
- [ ] 2.3 Add the acknowledgement handle and its settlement type — accepted, rejected, generation ended — and return it from `event` and `auth`; verify a test publishes two different events and asserts each handle settles on its own verdict only
- [ ] 2.4 Make the generation ending name whether the connection dropped or the reconnect budget was exhausted; verify a test drives both and asserts they are distinguishable without inspecting text
- [ ] 2.5 Fan an acknowledgement out to every live handle awaiting that event, accepting rather than refusing a second publication of the same event; verify a test publishes one event from two callers and asserts both settle on the relay's verdict
- [ ] 2.6 Add `RelaySession::auth_challenges()` delivering only challenges; verify a test asserts an unsolicited challenge reaches it with nothing having been sent, and that a live subscription and a pending publication observe nothing
- [ ] 2.7 Turn `fanout.rs` into a router that chooses which handle's queue an item enters, leaving each queue, `dropped` counter, `Notify`, and detach flag as they are, and moving a decoded event into its single destination rather than cloning; verify the existing bounded-loss and detach tests pass unchanged
- [ ] 2.8 Count unclaimed messages and undecodable bytes through `fava-diagnostics` with a bounded reason, delivering neither to any handle and keeping the session open; verify tests assert a notice leaves a pending publication awaiting, that undecodable bytes leave a live subscription unaffected, and that the two counts are separate
- [ ] 2.9 Delete `RelaySession::messages()`, `RelayInbound`, and `RelayMessageStream`; verify a grep for each is empty outside historical records
- [ ] 2.10 Preserve relay ordering per handle and prove no handle can park the reader; verify with the existing adversarial testkit assertions plus a case where the unread handle owns a subscription and the read one does not

## 3. Stage 2 — closing, and generation lifecycle

- [ ] 3.1 Add `Subscription::close()` sending the closure and reporting its handoff outcome; verify a test asserts the caller learns whether the closure was handed off, refused, or left unprovable
- [ ] 3.2 Make `Drop` enqueue the same closure without awaiting, using the non-blocking enqueue `send` already performs at `crates/fava-transport-websocket/src/session.rs:88`; verify a test releases a handle without closing it and asserts the relay received the closure
- [ ] 3.3 Send nothing when a handle whose generation has ended is released; verify a test reconnects, releases the stale handle, and asserts no closure was written
- [ ] 3.4 End every live handle when the generation advances, before anything from the new generation is delivered; verify a test asserts an event naming the old identifier on the new generation reaches no old handle, and that an outstanding acknowledgement reports the ending distinctly from a rejection
- [ ] 3.5 Assert the router retains nothing across a generation; verify a test reconnects under load and asserts the retained key count returns to zero before the new generation's first message

## 4. Stage 2 — consumers stop decoding

- [ ] 4.1 Delete the UTF-8 and `decode_relay` handling from `fava_observe::ingest::accept`, taking decoded values as input while leaving attribution against `InstalledSubscriptions` untouched; verify the existing observe attribution tests pass unchanged and a grep for `decode_relay` in `fava-observe` is empty
- [ ] 4.2 Hold subscription handles in `Slot.installed` in place of identifiers and the per-session listen task, so dropping a slot closes what it held; verify a test asserts an installed subscription's events reach their observation, a closed one's do not, and dropping the slot sends the closures
- [ ] 4.3 Keep `Slot::advance` and `owners()` attributing a wire subscription to the observations that own it; verify the existing attribution and completeness tests pass unchanged
- [ ] 4.4 Delete the UTF-8 and `decode_relay` handling from `fava_auth::authenticator::session_watch::admit` and read challenges from `auth_challenges()`; verify the existing NIP-42 watch tests pass unchanged and a grep for `decode_relay` in `fava-auth` is empty

## 5. Stage 3 — the publication attempt awaits its own acknowledgement

- [ ] 5.1 Publish through the `event` verb in `Nip01Publisher` and await its settlement; verify a test asserts the verdict is reported when it arrives after a large volume of unrelated subscription traffic
- [ ] 5.2 Delete `MAX_INBOUND_FRAMES` and the frame-scanning loop from `Nip01Publisher`; verify a grep for `matching OK absent` in `fava-publisher-nip01` returns only the `Nip42Publisher` occurrence, and none if `own-relay-authentication` has already deleted that publisher
- [ ] 5.3 Delete `Nip01Publisher`'s `NOTICE`, `AUTH`, and decode-error branches; verify tests assert an attempt is unaffected by a notice, by an unsolicited challenge, and by an undecodable frame, and reports the relay's actual verdict in each case
- [ ] 5.4 Bound the wait by the attempt's own deadline and by session liveness alone; verify tests assert an elapsed deadline, a disconnection, and an exhausted reconnect budget each report a distinct unknown outcome, and that none is reported as acknowledged, rejected, or never sent
- [ ] 5.5 Prove the same publication behaves identically on a busy and an idle connection; verify with a test publishing the same event under both against a relay answering identically and asserting equal outcomes

## 6. Conformance and boundary pinning

- [ ] 6.1 Replace `require_inbound_fan_out` in `fava-transport-testkit` with verb conformance — the exact frame each verb produces, per-handle delivery, acknowledgement fan-out, the generation ending every handle, release sending the closure, and unclaimed traffic reaching no handle; verify `fava-transport-websocket`'s conformance test passes against the new suite
- [ ] 6.2 Add `crates/fava-transport/tests/architecture.rs` on the `fava-nip02` model, pinning the dependency set exactly and asserting no filter, demand, plan, subscription-planning, or observation type appears in its manifest or source; verify the test fails when an extra dependency is added
- [ ] 6.3 Assert no crate outside the transport reaches a relay without a verb; verify with a test that greps the workspace for client-message construction, encoding, and relay-message decoding, allowing only `fava-transport*` and the two byte-length measurements in `fava-subscriptions-*`

## 7. Documentation

- [ ] 7.1 Amend `docs/spec/ARCHITECTURE.md` at the transport responsibility line (`:76`), the `fava-wire` relationship (`:408`), the `fava-transport` contract, owned state, and `HandoffCorrelation` paragraph (`:1696-1757`), the `fava-transport-websocket` framing-diagnostics restriction (`:1777`), the inbound flow diagram (`:2946-2948`), and the replaceable-boundary table rows (`:3353`, `:3850`); verify each amended line names protocol verbs and per-handle delivery rather than bytes, and that no remaining line asserts the transport moves bytes only
- [ ] 7.2 Record in `ARCHITECTURE.md` that the session owns envelope construction and mints subscription identifiers while the planner still decides which subscriptions exist, with no filters, demand, plan revision, or observation identity retained; verify the statement names OWN-02 and matches what `crates/fava-transport/tests/architecture.rs` enforces
