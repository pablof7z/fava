## 0. Consolidate the lifecycle vocabulary

- [x] 0.1 Move `BoundedText` from `fava-query` to `fava-relay` and delete `fava_transport::BoundedReason`, updating every user of both; verify `cargo build --workspace --all-targets --locked` succeeds and a grep for the deleted type is empty
- [x] 0.2 Move `AuthenticationState` from `fava-query` to `fava-relay`, keeping `RelaySourceState::AuthenticationRequired` working; verify the existing query-evidence tests pass unchanged
- [x] 0.3 Extend `AuthenticationState` with `Accepted`, `Failed { reason }`, and `AwaitingAnswer`, and give `AcceptedButStillRefused` the relay's own message; verify a test asserts every variant is constructible and carries its text within the bound
- [x] 0.4 Assert one lifecycle enum exists: no second authentication state, outcome, or verdict type in any crate; verify with a test that greps the workspace for a competing definition

## 1. Create the `fava-auth` crate

- [x] 1.1 Scaffold `crates/fava-auth` with a workspace members entry and a `[workspace.dependencies]` path entry; verify `cargo build --workspace --all-targets --locked` succeeds with the empty crate present
- [x] 1.2 Add `Challenge` with an explicit byte bound, refusing empty and over-bound text through a typed error rather than truncating; verify unit tests cover empty, at-bound, and over-bound input
- [x] 1.3 Add `AuthenticationDemand` carrying `fava_transport::RelaySessionIdentity` and a `Challenge`, plus `AuthenticationDecision`; verify a test asserts the demand names the exact session key and generation the challenge arrived on
- [x] 1.4 Add `auth_event`, building the unsigned kind-22242 response through `fava_write::EventBuilder` with the relay and challenge tags; verify a test asserts kind, both tag rows, and their exact values
- [x] 1.5 Add `SessionAuthentication` tracking one `RelaySessionKey` across generations, with `challenged`, `resolved`, `reconnected`, `state`, `authenticated`, and `attempts`, and an explicit attempt bound; verify unit tests cover replacing a challenge, exhausting the bound, and a reconnect clearing an earlier verdict
- [x] 1.6 Add the `AuthenticationPolicy` trait with a synchronous `decide`, matching `DeliveryPolicy`, plus a blanket implementation over `Fn(&AuthenticationDemand) -> AuthenticationDecision`; verify a test supplies a bare closure and a stateful type through the same builder method with no adapter between them
- [x] 1.7 Add `AuthenticationDecision::Defer`, `AuthenticationDemandId`, and `PendingAuthentication`; verify a test asserts a deferred demand carries a stable identity and the session generation it arrived on
- [x] 1.8 Add `crates/fava-auth/tests/architecture.rs` on the `fava-nip02` model, asserting the crate's declared dependencies exactly match its allowed set and that banned owners appear in neither the manifest nor its source; verify the test fails when an extra dependency is added

## 2. Drive the handshake

- [x] 2.1 Acquire a lease per `RelayAccess::Authenticated` session key and watch its inbound stream for `RelayMessage::Auth`, running the watch under `Runtime::spawn_cancellable`; verify a test with a fake transport observes a challenge delivered to the watcher while no publication is in flight
- [x] 2.2 Consult the policy, sign through `Session::invoke_signer` for the key's account, and send `ClientMessage::Auth`, running each await under `Runtime::call_provider` with a Fava-owned deadline; verify a test asserts the wire transcript carries exactly one client `AUTH` frame per approved challenge
- [x] 2.3 Classify the relay's reply into `AuthenticationState` variants, reading `auth-required:` and `restricted:` through `nostr::message::MachineReadablePrefix::parse`; verify tests cover acceptance, rejection, acceptance-with-refusal, and a policy decline
- [x] 2.4 Report no signing and no `AUTH` frame when the policy declines or no signer is attached; verify a test asserts the transcript contains no `AUTH` frame in both cases
- [x] 2.5 Answer a fresh challenge on a new generation after reconnect and discard a verdict arriving from a replaced generation; verify tests cover both
- [x] 2.6 Stop answering once the attempt bound is reached for a generation; verify a test drives a relay that re-challenges continuously and asserts signing stops at the bound
- [ ] 2.7 Release the lease when no authenticated work remains for that key; verify a test asserts the transport holder count returns to its pre-authentication value
- [x] 2.8 Retain a deferred demand without signing or sending, and expose `Fava::authentication()` with `pending`, `subscribe`, and `answer`; verify a test defers a challenge, observes it in `pending`, sees the signal fire, and asserts no `AUTH` frame was sent
- [x] 2.9 Authenticate on an approving answer and decline on a refusing one; verify tests assert exactly one `AUTH` frame in the first case and none in the second
- [x] 2.10 Drop a deferred demand when its generation is replaced, signal the change, and make a stale answer resolve nothing; verify tests cover both
- [ ] 2.11 Add `FavaBuilder::authentication_policy` and assemble `fava-auth` into the engine; verify a test builds a `Fava` with a policy and asserts a challenge on an authenticated session is answered

## 3. Reach the read path

- [ ] 3.1 Delete `Accepted::AuthenticationRequired` and the `RelayMessage::Auth` branch at `crates/fava-observe/src/ingest.rs:114`, so `fava-observe` no longer decodes authentication from the wire; verify a grep finds no authentication decoding left in `fava-observe`
- [ ] 3.2 Report `RelaySourceState::AuthenticationRequired` from the owner's published outcome keyed by session identity, so every `AuthenticationState` variant becomes reachable; verify a test drives each variant through a real observation and asserts the evidence value
- [ ] 3.3 Prove an authenticated query completes end to end: challenge, policy approval, signed response, relay acceptance, results delivered; verify through one assembled `Fava` via its public API
- [ ] 3.4 Prove one authenticated account's denial leaves another account's observation and public-access work running; verify with the `auth_denied_for_one_access_context_leaves_another_running` falsifier named in OWN-07
- [ ] 3.5 Prove no component outside `fava-auth` retains challenge state or an authentication flag; verify with the `nip42_challenge_state_lives_only_in_fava_auth` falsifier named in OWN-07

## 4. Unify account selection

- [ ] 4.1 Replace `Fava::by(author)` with `Fava::with_account(public_key)`, keeping `Session::current_account()` as the default and public access as the fallback; verify the existing publication tests pass against the new verb
- [ ] 4.2 Make `with_account` select the relay access authority for observations as well as publications; verify a test asserts an observation opened under an account uses that account's session key
- [ ] 4.3 Remove `Query::with_relay_access` from the public surface and update every caller to select through `with_account`; verify a grep finds no remaining public use and the observe tests pass
- [ ] 4.4 Delete `AuthorlessPayload` and let `with_account(...).publish(...)` accept an authored payload; verify a test publishes an event authored by one account under a selection naming another and asserts the event's author and the session's account differ as expected
- [ ] 4.5 Refuse before acceptance when an authorless payload has no author to resolve; verify a test asserts an immediate typed refusal with no current account and no selection
- [ ] 4.6 Update every call site of the removed verb across crates, tests, examples, and `apps/canary`; verify `cargo build --workspace --all-targets --locked` and the canary manifest build both succeed

## 5. Make the selected account durable

- [ ] 5.1 Carry the accepted `RelayAccess` on `WriteIntent` and `Receipt`, not a second public key beside the existing author; verify a test asserts the access authority round-trips through acceptance and readback and that no second author field was introduced
- [ ] 5.2 Reshape `RouteRequest::Write` to carry the access authority and make `RouteRequest::access()` return it instead of `RelayAccess::Public`; verify a test asserts an automatically routed write under an account selects destinations under that account's access
- [ ] 5.3 Update every `RouteRequest::Write` construction and destructuring site in `fava-publication`, the router crates, and their tests; verify `cargo build --workspace --all-targets --locked` succeeds
- [ ] 5.4 Bump `SCHEMA_VERSION` from 4 to 5 in `crates/fava-write-store-redb/src/schema.rs`; verify `redb_schema_mismatch_refuses_without_fallback` still refuses a store stamped with a different version
- [ ] 5.5 Extend the four row-mutation recovery tests with a tampered access field — absent, malformed, and an authority contradicting the routed destinations — asserting reconstruction refuses each; verify the new cases fail when the check is removed
- [ ] 5.6 Rename those four tests to drop the `schema_v4_` prefix, which wrongly implies they test an earlier schema version; verify the renamed tests still run and pass
- [ ] 5.7 Prove a write parked awaiting a signer resumes after a real process restart under its accepted access authority and not under public access; verify with a process-kill test in `fava-write-store-redb`
- [ ] 5.8 Prove a selection change, signer replacement, or account removal after acceptance does not retarget accepted work; verify a test asserts author and access authority are unchanged

## 6. Retire `Nip42Publisher`

- [ ] 6.1 Move the `apps/canary` phase-F gate onto `fava-auth`, keeping its assertions on the `AUTH` challenge and kind-22242 response wire frames against `communities-relay`; verify the gate passes against the real relay before anything is deleted
- [ ] 6.2 Delete `Nip42Publisher` and its `build_auth_event` from `fava-publisher-nip01`, and update its crate doc line which still claims a NIP-42 variant; verify `cargo build --workspace --all-targets --locked` succeeds and no reference remains
- [ ] 6.3 Confirm `Nip01Publisher` still reports `PublishOutcome::AuthenticationRequired` for a relay that returns `auth-required:` in an ordinary `OK`; verify the existing publisher test still passes
- [ ] 6.4 Hold a publication attempt whose session has a deferred demand instead of recording `AuthenticationDenied` at `crates/fava-publication/src/delivery.rs:196`, waking it on the answer through the path a parked write already uses; verify a test asserts the receipt stays open while the demand is outstanding and completes after approval, alongside the existing WRITE-008 parked-write proof
- [ ] 6.5 Prove an authenticated publication completes end to end through one assembled `Fava` on a relay that demands `AUTH` for writes; verify through the public API, not direct provider calls

## 7. Close the gates

- [ ] 7.1 Run the full configured validation set — `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --all-targets --locked`, `cargo test --workspace --doc --locked`, both external falsifiers, and the canary suite — and verify every one passes
- [ ] 7.2 Review every file this change touches against the 500-line soft and 800-line hard limits, splitting on real ownership boundaries; verify no touched file exceeds the hard limit
- [ ] 7.3 Confirm the coherence findings hold: one lifecycle enum, one bounded-text type, one verb selecting an account, one component decoding authentication, and no `RelayAccess` duplicated as a second author; verify each with a grep or test named in the task that introduced it
- [ ] 7.4 Sign the changed and added public declarations through Symbol Gate; verify `symbol-gate verify` accepts the result
