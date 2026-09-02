## Why

A relay connection has one thing to say about itself: where it has got to. Twenty-two places in the code hold a piece of that, and no two of them agree on what to call it.

The cost is not abstraction. It is bugs, and they are live on `main`:

- A challenge queued on one connection survives the reconnect and is recorded against the next one, resetting the attempt budget and storing a nonce the relay never sent. `Router::end_connection` drains subscriptions and acknowledgements and never touches the challenge queue.
- `Mailbox::offer` at capacity discards the *incoming* item, so a full queue keeps the oldest state and drops the newest — backwards for a state machine. Nothing calls `take_dropped` on a connection or challenge queue, so that loss is silent.
- `publish_authentication` fires immediately after `subscription_refused` and `stored_complete`, and `record_state` replaces rather than merges, so the relay's own words and a just-published end-of-stored-events are overwritten by an authentication state — for every observation at that relay, not just the affected subscription.
- `AuthenticationOutcomes::state` returns nothing for a session nobody separately asked the authenticator to watch, so the write path's authentication branch cannot be reached by any real assembly. No test has ever run it.

Underneath all four: authentication is a fact about a connection, and it is being carried between components instead of being held by the connection.

The scale is the argument. Of roughly 713 mentions of relay session identity and access, five read the access to decide anything, and none of them is a router. Every timing constant in this area except the re-challenge bound compensates for a signal that was never sent — including a 250ms poll whose own comment names the missing signal.

## What Changes

- **BREAKING** A connection carries two states, because they are two facts: its connectivity (`Disconnected`, `Connecting`, `Connected`) and its authentication (`None`, `Requested`, `Authenticating`, `Authenticated`, `Declined`, `Failed`). Authentication belongs to the connection, so reconnecting resets it. Together they replace `ConnectionState`, half of `SessionEnded`, the challenge queue, four separate liveness flags, and five copies of the connection counter.
- **BREAKING** Access stops being identity. `RelaySessionKey { relay, access }` becomes the relay alone. Work states what it needs of a connection, and a connection serves it when it can still *reach* that state: an unauthenticated one can still become Bob's, one already authenticated as Alice cannot. When nothing matches, a connection is opened. A relay may therefore hold more than one, but by what work needs rather than by a name in a key.
- **BREAKING** Rename `Public` to `Unauthenticated`. A connection nobody has authenticated is not a public one.
- The application's policy names the account a challenge is answered as, rather than it being read off the session key.
- Connection state is read as a `watch` channel carrying the current value, not a queue of past values. Five hand-rolled drain loops collapse to one call.
- **BREAKING** `fava-auth` never opens a connection. It attends one someone else opened, decides, and writes the answer into it. Its lease, its transport dependency, its own four transport deadlines and bounds, its last-holder poll, its release heuristic and its watch bookkeeping all go.
- **BREAKING** Delete `AuthenticationOutcomes`. Both callers already hold the connection that now carries the state.
- **BREAKING** Delete the deferred-demand ledger. `challenged` already stores the challenge and `resolved` never clears it, so the session entry holds what the ledger holds. `AuthenticationDemandId` and `PendingAuthentication` go with it; an answer names the connection it belongs to.
- Waiting work waits on connection state. A write that meets `auth-required:` waits for its connection to leave `AuthenticationRequested`: reaching `Authenticated` resumes it, reaching a refusal fails it. No attempt ceiling, no backoff, no timer.
- `fava-auth` learns nothing about who is waiting, and tells nobody.

## Capabilities

### New Capabilities

- `transport/connection-state`: one connection per relay, the states it moves through, and how anything else learns of a change.

### Modified Capabilities

- `transport/session-protocol`: challenges stop being a named reader and become a state; the session's verbs gain the one that declines.
- `transport/message-routing`: routing carries messages that belong to a request, not facts about the connection.
- `identity/relay-authentication`: the owner decides and answers; it does not hold connections, a ledger of demands, or a copy of the connection's state.
- `publication/write-access-authority`: a write states the authority it requires of a connection rather than being routed to a connection that has it.
- `publication/attempt-acknowledgement`: an attempt that meets a demand for authentication waits for the connection rather than consuming a retry budget.

## Impact

`fava-transport`, `fava-transport-websocket`, `fava-transport-testkit`, `fava-auth`, `fava-relay`, `fava-observe`, `fava-publication`, `fava-publisher-nip01`, `fava-routing`, `fava-query`, `fava-write`, all three write stores, `fava-event-cache-persistent`, `fava-diagnostics`, `fava`, and both example workspaces.

Roughly 8,200 lines across the eight most affected crates today. The transport crates alone shed about 390 lines against 90 added; `fava-auth` sheds about 240 of its 1,082. Three whole files go, one of which is a copy of another.

Persisted shapes change. The write store and the event cache stay at schema version 1 and do not read anything written before.

Two questions the current design made unaskable, both now settled:

- **Who is a challenge answered as?** The application's policy names the account. It already decides whether to authenticate; it now decides as whom.
- **Anonymity.** A write requiring no authority needs an unauthenticated connection, and a connection authenticated as Alice can never satisfy that. Bob's anonymous write does not ride Alice's socket — not by policy, but because it does not match. The property the two-socket design had by accident becomes one the matching rule states.
