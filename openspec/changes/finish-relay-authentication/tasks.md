## 1. Stage 1 — an application can build an engine that authenticates

- [x] 1.1 Add `FavaBuilder::authentication_policy`, taking one policy for the engine and matching how `DeliveryPolicy` is supplied; verify a test supplies a bare closure and a stateful type through the same method with no adapter between them
- [x] 1.2 Assemble `fava-auth` into the engine so it takes its own lease per authenticated session key and reads that session's challenges; verify a test builds a `Fava` with a policy and asserts an unsolicited challenge is answered with no query or publication attached
- [x] 1.3 Authenticate nothing when no policy was supplied; verify a test builds a `Fava` without one, drives a challenge, and asserts the wire transcript carries no `AUTH` frame
- [x] 1.4 Expose `Fava::authentication()` with `pending`, `subscribe`, and `answer`; verify a test defers a challenge, observes it in `pending`, sees the signal fire, answers it, and asserts exactly one `AUTH` frame follows
- [x] 1.5 Prove an authenticated query completes end to end — challenge, policy approval, signed response, relay acceptance, results delivered — through one assembled `Fava` via its public API, not direct provider calls

## 2. Stage 2 — the outcome reaches an observation

- [x] 2.1 Consume authentication outcomes in `fava-observe` by session identity, keyed the same way connection state already is, and report `RelaySourceState::AuthenticationRequired` from them; verify a grep confirms `fava-observe` decodes no challenge and derives nothing from the wire
- [x] 2.2 Drive every `AuthenticationState` variant through a real observation and assert the evidence value for each; verify the test fails when any variant loses its producer
- [x] 2.3 Prove one authenticated account's denial leaves another account's observation and public-access work on the same relay running; verify with the `auth_denied_for_one_access_context_leaves_another_running` falsifier named in OWN-07
- [x] 2.4 Prove no component outside `fava-auth` retains challenge state or an authentication flag; verify with the `nip42_challenge_state_lives_only_in_fava_auth` falsifier named in OWN-07
- [x] 2.5 Release the authentication lease when no authenticated work remains for a relay; verify a test asserts the transport holder count returns to its pre-authentication value after the last authenticated observation and publication end

## 3. Stage 3 — remove the duplicate handshake

- [ ] 3.1 (open, and now the only evidence gap) Establish a real-relay NIP-42 proof owned by `fava-auth`, asserting the relay's `AUTH` challenge and Fava's kind-22242 response as wire frames against a relay that demands authentication; verify the proof passes against a real relay and names which relay it ran against, since no such harness exists anywhere in the repository today
- [x] 3.2 Delete `Nip42Publisher` and its `build_auth_event` from `fava-publisher-nip01`, and the crate doc line claiming a NIP-42 variant; verify `cargo build --workspace --all-targets --locked` succeeds and a grep for both names is empty
- [x] 3.3 Confirm `Nip01Publisher` still reports `PublishOutcome::AuthenticationRequired` when a relay answers an ordinary `OK` with `auth-required:`; verify the existing publisher test passes unchanged
- [x] 3.4 Hold a publication attempt whose session has a deferred demand instead of recording `AuthenticationDenied` at `crates/fava-publication/src/delivery.rs:196`, waking it on the answer through the path a parked write already uses; verify a test asserts the receipt stays open while the demand is outstanding and completes after approval, alongside the existing WRITE-008 parked-write proof
- [ ] 3.5 (blocked on 3.1) Prove an authenticated publication completes end to end through one assembled `Fava` against a relay that demands `AUTH` for writes; verify through the public API

## 4. Retire the superseded change

- [x] 4.1 Delete `openspec/changes/own-relay-authentication`, whose design describes the replaced transport and whose remaining account-selection and write-access tasks are unbundled; verify nothing else references it
- [x] 4.2 Propose the account-selection change — `Fava::with_account` replacing `by`, `AuthorlessPayload` deleted, `Query::with_relay_access` removed — so the unbundled work is recorded rather than lost; verify the change exists and validates
- [x] 4.3 Propose the write-access-authority change — `RelayAccess` through `WriteIntent`, `Receipt`, and `RouteRequest::Write`, `RouteRequest::access()` no longer hardcoding `RelayAccess::Public`, and the redb schema bump from 4 to 5; verify the change exists and validates

## 5. Verification

- [x] 5.1 Run the full configured validation set — `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --all-targets --locked`, `cargo test --workspace --doc --locked`, both named falsifiers, and the real-relay proof from 3.1 — and verify every one passes
- [x] 5.2 Confirm the coherence findings hold: one component decoding authentication, one lifecycle enum with every variant reachable through an observation, and no second challenge holder; verify each with the grep or test named in the task that introduced it
- [ ] 5.3 Sign the changed and added public declarations through Symbol Gate; verify `symbol-gate verify` accepts the result
