## 1. The two states

- [x] 1.1 Add `Connectivity`, `Authentication` and `Authority` to `fava-relay`, where work that never depends on the transport can still state what it needs, and the connection value holding both states alongside its identity to `fava-transport`; verify each state is constructible, a test names every one, and a test proves the matching rule for every pair
- [x] 1.2 Publish them on a `watch` channel per connection, replacing `Router::read_connection` and `connection_changed`; verify a component that begins watching an already-authenticated connection reads that state without waiting for a change
- [x] 1.3 Make the driver's transitions one write each, replacing the paired `end_connection` plus `connection_changed` calls and the duplicated failure formatting in both the websocket driver and the testkit session; verify the reconnect and exhaustion tests pass unchanged
- [ ] 1.4 Reset authentication when a connection is replaced, so nothing proved on the previous one carries over; verify a test authenticates, forces a reconnect, and reads no authentication on the replacement

## 2. The demand is a state

- [x] 2.1 Move the relay's `AUTH` frame from `Router::read_challenges` into an authentication state carrying the challenge; verify a challenge queued before a reconnect cannot be recorded against the connection that replaces it, which is the bug this closes
- [x] 2.2 Add the session verb that records a refusal without sending a frame; verify a test distinguishes refused from undecided and asserts no frame reached the relay
- [ ] 2.3 Delete `Router::read_challenges`, the challenges field, the `Correlation::Challenge` arm, and `RelaySessionExt::challenges`; verify a grep for each name is empty and the workspace builds
- [ ] 2.4 Confirm a repeated identical challenge wakes no watcher; verify a test pushes the same challenge three times and asserts one wake
- [ ] 2.5 Prove a coalescing channel cannot lose a challenge that matters: a relay re-challenging with a *different* nonce while the decider is between wakes must still be answered against the nonce the relay last sent; verify a test holds the decider, pushes two different nonces, and asserts the answer carries the second — this is the one risk the watch channel introduces

## 3. Matching by reachability

- [x] 3.1 Delete `RelaySessionKey` and `RelayAccess`, keying by `RelayUrl` directly — once access is a connection's state the struct is a newtype over the URL with no invariant of its own, and nothing else in the workspace wraps a URL for it to be confused with; verify the workspace builds and a grep for both names is empty
- [x] 3.2 Give work a stated requirement — no authentication, or authentication as one account — carried where access used to be; verify a write and a query each state one and a test reads it back
- [x] 3.3 Implement the matching rule: a connection serves work when it can still reach the required state, and one is opened when none can; verify tests cover an unauthenticated connection serving work that will authenticate, a connection authenticated as one account refusing work for another, and anonymous work refusing an authenticated connection
- [x] 3.4 Delete the four struct-to-map-key serde adapters and the two identical routing-to-destination zips in the memory and redb stores; verify a grep for each is empty
- [x] 3.5 Rename `Public` to `Unauthenticated` throughout; verify no occurrence of the old name remains outside archived changes

## 4. The owner decides and answers, nothing more

- [x] 4.0 Add `Transport::authentication_requests`, a broadcast of the sessions whose relay has just asked, so the one component that answers hears about a demand without holding, opening, or enumerating connections; verify a test asserts a challenge on one connection reaches a subscriber and a connection nobody challenged does not
- [x] 4.1 Replace `watch_session` and `watch_session_soon` with attending the session handed to it by that signal, and delete the lease, `open_request`, `SESSION_DEADLINE`, the frame bounds, `LAST_HOLDER_CHECK`, `LONE_CHECKS_BEFORE_RELEASE`, the `watching` set, the release loop, `WatchError` and `live_session`; verify `fava-auth` no longer depends on `Transport` and a grep for each name is empty
- [x] 4.2 Delete `Fava::watch_authenticated_relays`, `transport_for_auth`, and `BuildError::MissingAuthenticationTransport`; verify an engine with a policy and no transport builds
- [x] 4.3 Let the policy name the account it authenticates as; verify a test's policy authenticates as an account the connection was not opened for
- [ ] 4.4 Delete the deferred-demand ledger, `AuthenticationDemandId` and `PendingAuthentication`, answering by connection instead; verify the deferred-then-answered test passes and the session entry is the only record of an outstanding ask
- [x] 4.5 Delete `SessionAuthentication`'s copy of the connection counter and the stale-generation comparisons it served; verify the existing proof that a stale answer resolves nothing still passes

## 5. Waiting on the connection

- [x] 5.0 Remove `state` and `authenticated` from the authentication owner's public surface, along with the copy of connection state they read; verify a grep finds no caller and nothing outside the owner holds a second opinion about a connection
- [x] 5.1 Delete `AuthenticationOutcomes` and read the connection directly in both callers; verify `fava-observe` and `fava-publication` compile without it and a grep for the name is empty
- [x] 5.2 Stop republishing authentication from the nine completion handlers, so a relay's own words and an end-of-stored-events are no longer overwritten by an authentication state; verify a test asserts a refusal keeps the relay's message and that a completed stored window survives
- [ ] 5.3 Park a destination that meets `auth-required:` on its connection's authentication, spending no attempt and eligible for no policy; verify a test asserts the attempt count does not advance while it waits
- [ ] 5.4 Resume a parked destination when its connection satisfies the write's authority and fail it when it cannot; verify one test drives each transition and asserts the resulting receipt
- [ ] 5.5 Confirm the deciding component knows nothing of waiting work; verify a test parks a publication, drives the answer through the policy alone, and asserts the publication proceeds

## 6. What is left over

- [ ] 6.1 Delete `Router::retained`, `Unrouted` and the `unrouted` counters, `Correlation` and `correlation()`, and the duplicate identity file in the testkit; verify a grep for each name is empty
- [ ] 6.2 Replace the four liveness flags and the close notification with the connectivity state; verify the close and shutdown tests pass unchanged
- [ ] 6.3 Collapse the five hand-rolled drain loops onto the watch channel; verify each of the five sites reads one call and the tests around them pass

## 6b. What an independent review found

Eleven findings against the landed commits. Three are already assigned; the rest are here so none is lost.

- [ ] 6b.1 Take a weak reference to the session in the transport's request publisher, at both call sites rather than only inside it; verify a session with no lease holders is dropped, which today it never is, and that the two committed tests asserting a connection ends stop hanging
- [ ] 6b.2 Drop the connection's sender when the router closes, so a connection that will never reach another state says so; verify the observe listener terminates and the two hanging assertions pass
- [ ] 6b.3 Write the connectivity when a session closes; verify a closed connection reports disconnected rather than staying connected on the fake and reconnecting forever on the real one, and that it serves no work
- [ ] 6b.4 Guard the answer's write on the connection it was signed for, the way every other authentication write already is; verify a connection replaced while a signer was working is not left mid-answer for a challenge it never received
- [ ] 6b.5 Separate what the relay has accepted from how the challenge is going, per the design decision; verify a connection that could not answer still carries anonymous work, and one the relay accepted never carries it again whatever happens to a later challenge
- [ ] 6b.6 Answer each request the transport publishes without the answering loop signing inline, so a slow signer cannot block every other relay and overflow the backlog; verify a request is not lost when a subscriber falls behind, which is the failure the design chose a broadcast to avoid
- [ ] 6b.7 Give the evidence that says a relay demands authentication a producer again, scoped to the observation it concerns rather than every observation at that relay; verify an observation at a relay demanding authentication carries evidence saying so
- [ ] 6b.8 Restore the hold on a publication whose session is waiting for a person, which was removed as unreachable and was reachable; verify a write refused for want of authentication while a person is being asked stays open
- [ ] 6b.9 Retire what is kept about connections that are gone — the attempt ledger and demands from superseded connections at the same relay both grow without bound; verify neither retains an entry for a connection that has been replaced
- [ ] 6b.10 Make an exhausted reconnect budget reach the handles on the real transport as it does on the fake; verify a publisher sees the same ending either way
- [ ] 6b.11 Delete the connection-state enum the two states replaced; verify a grep finds only its definition and re-export, then neither

## 7. Verification

- [ ] 7.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --all-targets --locked`, and `cargo test --workspace --doc --locked`; verify every one passes
- [ ] 7.2 Build and test both example workspaces and `examples/crates/e2e-support`; verify each passes
- [ ] 7.3 Run the live proof against the four relays and refresh the committed evidence; verify every authentication state the app drove before is still driven
- [ ] 7.4 Falsify each new test by reverting the behavior it asserts, one at a time; verify each reversion fails only its own test
- [ ] 7.5 Report the line count removed against the ~8,200 the canvasses measured; verify the number with `wc -l` rather than estimating
- [ ] 7.6 Sign the changed and added public declarations through Symbol Gate; verify `symbol-gate verify` accepts the result
