## Context

See proposal.md — Why. The measurements there come from five independent canvasses of the affected crates; the counts are theirs.

## Goals / Non-Goals

**Goals:**
- One place holds where a connection has got to.
- Anything that needs a relay in a condition waits for that condition.
- The component that decides authentication learns nothing about who is waiting.

**Non-Goals:**
- Delivering every intermediate state. A watcher sees the state as it is now, which is what a status question wants and what an audit log does not.
- Observing the very first dial. `acquire_session` awaits establishment before a session object exists, so a first-attempt failure stays a returned error rather than a state. Changing that means handing back a handle to a socket that does not exist yet; it is a separate decision.
- Preserving subscriptions across an authentication change. If a connection can no longer serve a subscription, closing it is acceptable.

## Decisions

**Two states, not one enum.** Connectivity and authentication are independent questions with independent answers, and most waiters care about one of them. A single flat enum forces every reader to match arms it does not care about, and forces the two facts to be written together even when only one moved. Authentication is scoped to the connection rather than the relay, so a replacement starts with none — that scoping is what makes a stale answer impossible without a counter to compare.

*Alternative.* One enum with authentication states threaded between `Connected` and `Ended` reads well on a diagram and badly in code: `Ended` and `Authenticated` are not alternatives to each other, and expressing "connected, whatever its authentication" needs a match over half the variants.

**Matching by reachability, not by name.** Work states its requirement; a connection serves it if it can still reach that state. Unauthenticated can still become Bob's; authenticated as Alice cannot become Bob's or become anonymous. So a relay may hold more than one connection, but because two pieces of work genuinely cannot share one — not because a key said so.

This replaces access-as-identity and is strictly more expressive: the old keying could not represent "either of these will do", which is what an unauthenticated connection is to work that will authenticate. It also makes the anonymity property explicit. Under the old design an anonymous write avoided an authenticated socket because a different key named a different socket; under this one it avoids it because an authenticated connection cannot satisfy the requirement.

**The relay's demand is a state.** Queuing it as a message is what let a challenge outlive the connection it arrived on: the queue is not drained when a connection ends, and the component reading it asks the session which connection is current — after the driver has advanced it. A state carrying its own connection cannot be misattributed. Coalescing is the second reason: a repeated identical challenge is not a change, so the bookkeeping that exists to stop a re-challenging relay asking a person twice becomes a property of the channel.

**A watch channel, not a queue.** A queue of past states is the wrong shape for a current fact, and this one drops the newest item when full rather than the oldest, so a full queue keeps stale states and discards fresh ones — silently, because nothing reads its loss counter. Five components hand-roll the same drain loop to reconstruct "the current state" from it. `Mailbox` survives for subscription items and settlements, which are events.

**The policy names the account.** The account was read off the session key, which only worked because the key named one. Asking the application is not a workaround for losing that: deciding whether to authenticate and deciding as whom are one decision, and the application is already making half of it.

**Nothing carries authentication facts sideways.** `AuthenticationOutcomes` exists so two readers can learn a conclusion without depending on the crate that reached it. Both readers already hold the connection, so the trait's job disappears rather than moving. Its removal also ends the arrangement where an observation's authentication state is republished from nine unrelated completion handlers and overwrites the fact just published beside it.

## Risks / Trade-offs

- **More connections than the current design opens** → only where the current design could not have shared one either. Two pieces of work wanting different accounts always needed two sockets; the difference is that the second is opened because nothing could serve the work, not because a key spelled it.
- **A waiter waits forever on a policy that never decides** → intended. Not deciding is a state, and a person is expected to. Every caller already imposes its own bound; none is imposed here.
- **The removed requirements are load-bearing for something unseen** → both are argued in the spec delta with their reason and migration, and both describe mechanisms rather than behavior. The behavior each protected is restated where the state now lives.

## Migration Plan

`finish-relay-authentication` remains open at 19/21. Its task 3.4 is the behavior this change makes expressible, and its task 5.3 is unrelated. Archive it before this lands or fold its two open tasks in; do not implement 3.4 against the design being replaced.

Persisted shapes change in the write store and the event cache. Both stay at schema version 1 and refuse anything written earlier.
