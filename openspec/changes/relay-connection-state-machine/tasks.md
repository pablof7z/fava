## 1. The two states

- [x] 1.1 Add `Connectivity`, `Authentication` and `Authority` to `fava-relay`, where work that never depends on the transport can still state what it needs, and the connection value holding both states alongside its identity to `fava-transport`; verify each state is constructible, a test names every one, and a test proves the matching rule for every pair
- [x] 1.2 Publish them on a `watch` channel per connection, replacing `Router::read_connection` and `connection_changed`; verify a component that begins watching an already-authenticated connection reads that state without waiting for a change
- [x] 1.3 Make the driver's transitions one write each, replacing the paired `end_connection` plus `connection_changed` calls and the duplicated failure formatting in both the websocket driver and the testkit session; verify the reconnect and exhaustion tests pass unchanged
- [x] 1.4 Reset authentication when a connection is replaced, so nothing proved on the previous one carries over; verify a test authenticates, forces a reconnect, and reads no authentication on the replacement

## 2. The demand is a state

- [x] 2.1 Move the relay's `AUTH` frame from `Router::read_challenges` into an authentication state carrying the challenge; verify a challenge queued before a reconnect cannot be recorded against the connection that replaces it, which is the bug this closes
- [x] 2.2 Add the session verb that records a refusal without sending a frame; verify a test distinguishes refused from undecided and asserts no frame reached the relay
- [x] 2.3 Delete `Router::read_challenges`, the challenges field, the `Correlation::Challenge` arm, and `RelaySessionExt::challenges`; verify a grep for each name is empty and the workspace builds
- [x] 2.4 Confirm a repeated identical challenge wakes no watcher; verify a test pushes the same challenge three times and asserts one wake
- [x] 2.5 Prove a coalescing channel cannot lose a challenge that matters: a relay re-challenging with a *different* nonce while the decider is between wakes must still be answered against the nonce the relay last sent; verify a test holds the decider, pushes two different nonces, and asserts the answer carries the second — this is the one risk the watch channel introduces

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
- [x] 4.4 Delete the deferred-demand ledger, `AuthenticationDemandId` and `PendingAuthentication`, answering by connection instead; verify the deferred-then-answered test passes and the session entry is the only record of an outstanding ask
- [x] 4.5 Delete `SessionAuthentication`'s copy of the connection counter and the stale-generation comparisons it served; verify the existing proof that a stale answer resolves nothing still passes

## 5. Waiting on the connection

- [x] 5.0 Remove `state` and `authenticated` from the authentication owner's public surface, along with the copy of connection state they read; verify a grep finds no caller and nothing outside the owner holds a second opinion about a connection
- [x] 5.1 Delete `AuthenticationOutcomes` and read the connection directly in both callers; verify `fava-observe` and `fava-publication` compile without it and a grep for the name is empty
- [x] 5.2 Stop republishing authentication from the nine completion handlers, so a relay's own words and an end-of-stored-events are no longer overwritten by an authentication state; verify a test asserts a refusal keeps the relay's message and that a completed stored window survives
- [x] 5.3 Park a destination that meets `auth-required:` on its connection's authentication, spending no attempt and eligible for no policy; verify a test asserts the attempt count does not advance while it waits
- [x] 5.4 Resume a parked destination when its connection satisfies the write's authority and fail it when it cannot; verify one test drives each transition and asserts the resulting receipt
- [x] 5.5 Confirm the deciding component knows nothing of waiting work; verify a test parks a publication, drives the answer through the policy alone, and asserts the publication proceeds

## 6. What is left over

- [x] 6.1 Delete `Router::retained`, `Unrouted` and the `unrouted` counters, `Correlation` and `correlation()`, and the duplicate identity file in the testkit; verify a grep for each name is empty
- [x] 6.2 (examined and left alone: the closed flags are set synchronously by the caller, so a session refuses further work the instant close is called. Reading connectivity instead defers that refusal until the driver processes the request, and a caller could hand off a frame in between. The flags are not a duplicate of the state; they are the part of it that happens immediately) Replace the four liveness flags and the close notification with the connectivity state; verify the close and shutdown tests pass unchanged
- [x] 6.3 Collapse the five hand-rolled drain loops onto the watch channel; verify each of the five sites reads one call and the tests around them pass

## 6b. What an independent review found

Eleven findings against the landed commits. Three are already assigned; the rest are here so none is lost.

- [x] 6b.1 Take a weak reference to the session in the transport's request publisher, at both call sites rather than only inside it; verify a session with no lease holders is dropped, which today it never is, and that the two committed tests asserting a connection ends stop hanging
- [x] 6b.2 Drop the connection's sender when the router closes, so a connection that will never reach another state says so; verify the observe listener terminates and the two hanging assertions pass
- [x] 6b.3 Write the connectivity when a session closes; verify a closed connection reports disconnected rather than staying connected on the fake and reconnecting forever on the real one, and that it serves no work
- [x] 6b.4 Guard the answer's write on the connection it was signed for, the way every other authentication write already is; verify a connection replaced while a signer was working is not left mid-answer for a challenge it never received
- [x] 6b.5 Separate what the relay has accepted from how the challenge is going, per the design decision; verify a connection that could not answer still carries anonymous work, and one the relay accepted never carries it again whatever happens to a later challenge
- [x] 6b.6 Answer each request the transport publishes without the answering loop signing inline, so a slow signer cannot block every other relay and overflow the backlog; verify a request is not lost when a subscriber falls behind, which is the failure the design chose a broadcast to avoid
- [x] 6b.7 Give the evidence that says a relay demands authentication a producer again, scoped to the observation it concerns rather than every observation at that relay; verify an observation at a relay demanding authentication carries evidence saying so
- [x] 6b.8 Restore the hold on a publication whose session is waiting for a person, which was removed as unreachable and was reachable; verify a write refused for want of authentication while a person is being asked stays open
- [x] 6b.9 Retire what is kept about connections that are gone — the attempt ledger and demands from superseded connections at the same relay both grow without bound; verify neither retains an entry for a connection that has been replaced
- [x] 6b.10 Make an exhausted reconnect budget reach the handles on the real transport as it does on the fake; verify a publisher sees the same ending either way
- [x] 6b.11 Delete the connection-state enum the two states replaced; verify a grep finds only its definition and re-export, then neither

- [x] 6b.12 Let a listener that fell behind recover the demands it missed. Answering no longer blocks the loop, so a lag is rare, but a burst of distinct challenges can still outrun it and a lost demand is lost for the life of that connection — the publisher only republishes a *changed* challenge. The component that answers cannot recover on its own: it holds no connections and the transport offers a count of holders, not a list. Give it a way to ask which connections are waiting to be answered, and have it ask after a lag; verify a demand dropped by a lagging listener is still answered, and that nothing polls

## 6c. A falsifier nobody has been running

- [x] 6c.1 Bring `falsifiers/external-semantic-capability` back to something that compiles, or retire it. Its relay-session fake is missing five methods the session gained when it started speaking NIP-01 — before this change — and two more this change added. It is a separate cargo workspace, so no root gate ever covered it and nobody noticed it rot. Decide whether it still proves something worth keeping; verify by `./scripts/gates` reporting it green, or by its absence from that script

  Repaired, not retired: it proves an ordinary external crate can implement Fava's contract with only public API, and this change reshaped exactly that contract. Its fake moved from the old pull model to the current push model -- it owns a real `Router` and implements `hand_off`, `mint_subscription_id`, `router`, `inbound_capacity`, `enqueue`, `sessions` and `authentication_requests`. Nothing under `crates/` moved, no dependency was added, and the fake got smaller: its inbound path now builds the wire message directly instead of round-tripping through JSON.

  The rot came from two places, and the split is the interesting part. `Fava::by` -> `with_account` predates this change (`f2679004`), and had been broken here since, unseen, because no gate reached this workspace -- which is the whole reason this task exists. `desired_destinations` becoming `BTreeSet<RelayUrl>` is ours (`dc927a2c`, task 3.1), fallout of deleting `RelaySessionKey`.

  One finding about the public surface: `fava-transport` re-exports neither `RelayMessage` nor `decode_relay`, so an external crate implementing a session has to reach past it to `nostr` for the type the router consumes.

## 7. Verification

- [x] 7.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --all-targets --locked`, and `cargo test --workspace --doc --locked`; verify every one passes
- [x] 7.2 Build and test both example workspaces and `examples/crates/e2e-support`; verify each passes
- [x] 7.3 Run the live proof against the four relays and refresh the committed evidence; verify every authentication state the app drove before is still driven

  Passed against all four relays; evidence refreshed to `examples/relay-auth/live/evidence/2026-09-03-live-nip42/`. Two things had to change first.

  The app read authentication out of `Fava::diagnostics()`, which reports only relays the *observation* owner leases — so a relay held by a publisher, including one parked waiting to be answered, was invisible, and every check returned `unknown`. `auth state` now asks the transport, which is the only owner of connections. That needed `Transport::sessions()`; `awaiting_authentication` became a default over it, so the trait gained one method and lost the special case.

  A connection's authentication dies with the connection, by design, so a check after the last holder released it is honestly `unknown`. The scenario now holds a query open across each check, which also means every state is read off the same live connection that reached it.

  Every state the app can drive is driven: `authenticated`, `unanswerable`, `declined`, `refused` twice, `requested`, `authenticating`. The two refusals share a name — the change collapsed `AcceptedButStillRefused` into `Refused` deliberately — so the harness now asserts the relay's own words instead, `error:` from `reject` and `restricted:` from `accept-refuse`, which is a stricter check than the name it replaced. `Idle` is the one state no relay here drives: all four challenge on connect.
- [x] 7.4 Falsify each new test by reverting the behavior it asserts, one at a time; verify each reversion fails only its own test

  All 31 new tests swept. Every one failed a reversion of the behavior it asserts except one, and no reversion took down a large unrelated blast radius. The sweep found three tests that proved nothing; all three are now closed.

  **Deleted.** `connectivity_is_three_states_and_says_nothing_about_authentication` contained no assertion -- a loop over three variants calling `format!` and discarding it. No change to production code can fail it, and adding a fourth variant would not either. It was a compile-surface note wearing a test's name.

  **`Authenticator::record`'s stale-identity guard was unexercised.** Deleting `if session.identity() != *identity { return false; }` outright failed *no test in `fava-auth`*. It is load-bearing: a signer can be remote, and if the connection is replaced while a person is deciding, `RelaySessionExt::answer` refuses the handoff and the resulting `Unanswerable` would be written onto the replacement -- marking a fresh connection unanswerable for a question it was never asked. Now `a_signer_finishing_after_a_reconnect_writes_nothing_on_the_replacement`, which fails on exactly that deletion and nothing else.

  **A rule was tested two crates from where it lives.** Making `fava-state`'s `exact_existing` match authority-sensitive fails nothing in `fava-state`; the only test that caught it was `fava-event-cache-memory`'s, which is aimed at `MemoryEventCache` -- a provider that does not decide this, since `EventCache::admit`'s default routes through `mutations_for_event` first. Now `one_relay_under_two_authorities_is_still_one_occurrence` asserts it where it is decided.

  **A test asserted the wrong observation.** The challenge stanza of `exact_access_keys_isolate_event_eose_and_challenge` pushed an `AUTH` at the *public* peer and then checked the *private* observation, which is unaffected either way. Removing the anonymous-slot guard in `completions.rs` passed every test in `fava-observe`. It now asserts the public observation's completed window survives a challenge it never provoked, and fails when that guard goes.

  **Recorded, not acted on.** `a_challenge_on_a_public_connection_still_reaches_the_policy` is falsifiable only through `FakeTransport::dial`: since access stopped being identity, nothing in production can tell a connection opened for public work from one opened for an account that has not authenticated yet. Every reversion in real code failed 4-5 of that file's 7 tests indiscriminately. The websocket transport's identical unconditional spawn is not covered by it.

  The two `public_api.rs` tests flagged as weak are in a file that declares itself a compile-surface proof; that is their stated job. One reported redundancy was not real: `exact_access_keys_isolate_event_eose_and_challenge` and `a_close_refusal_on_one_connection_leaves_the_other_untouched` fall to the same reversion because one bug breaks all three message kinds, but they cover `EVENT`/`EOSE` and `CLOSED` respectively.
- [x] 7.5 (measured, and the expectation was wrong) Report the line count removed against the ~8,200 the canvasses measured; verify the number with `wc -l` rather than estimating.

  Measured across the eight crates the canvasses counted, at `006b842a` and at the tip. Re-measured at `09bc441a`, once every task but Symbol Gate was done:

  ```
  production   11,338 → 12,118   +780
  tests         5,468 →  7,526  +2,058
  ```

  (Read once mid-change at 12,226 production and 7,392 tests. Production came down 108 after that, mostly from `awaiting_authentication` becoming a default over `Transport::sessions` and the ledger reads it replaced.)

  Nothing was removed on balance. The deletions were real — the lease and everything that existed to undo it, the polls, the duplicated lifecycle enum, the sideways trait, the counters nobody read — but the canvasses counted what would go without counting what takes its place. Two states on a watch channel, a matching rule, a request signal, an enumeration for catching up, and a write that waits on its connection are all new code. A dozen bugs found along the way each cost a fix and a test.

  The change was worth making for what it fixed and for having one owner per fact, and it did not make the code smaller. Both are true and the second was not expected.
- [ ] 7.6 Sign the changed and added public declarations through Symbol Gate; verify `symbol-gate verify` accepts the result

  Blocked on the repository owner. Checked on 2026-09-03, and the record four archived changes carry is wrong on one point, so here is what is actually true.

  The trust store is **not** empty. `symbol-gate trust list` reports one trusted key, `npub1l2vyh47mk2p0qlsku7hg0vn29faehy9hy34ygaclpn66ukqp3afqutajft`, at `~/Library/Application Support/symbol-gate/trusted_keys`. A bare `symbol-gate verify` refuses with `trusted_key_missing` only because it will not consult that store unless named -- running any Symbol Gate command executes this repository's own extractors as the caller, so a repository you merely scanned could have added a key to it.

  Naming it gets one step further and then stops: `symbol-gate verify --trusted-key <that path>` refuses with `policy_changed` -- no trusted key has signed `.symbol-gate/policy.toml` (digest `78ca531d…`). The policy decides which symbols are reviewable at all, so an unsigned one could remove symbols from review with nothing noticing.

  And the scale is not this change's: `symbol-gate status` reports **1497 of 1500 symbols unsigned**, across 146 files. The entire declared surface is unsigned, not the part this change touched. Signing the policy and the surface needs the owner's secret key, through `symbol-gate review` or a headless signer set with `symbol-gate signer set`. Nobody else can supply it.

  (`symbol-gate status` says so itself: it checks no signature and consults no key, and a coding agent that controls this repository can write any event it likes. Only `verify --trusted-key` means anything.)
