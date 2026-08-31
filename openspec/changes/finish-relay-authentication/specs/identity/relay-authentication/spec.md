## Purpose

Defines who Fava authenticates as on a relay connection, when it answers a NIP-42 challenge, what happens when the relay refuses, a person owns the decision, or the connection is replaced, and how each outcome reaches query evidence.

## ADDED Requirements

### Requirement: One owner holds every relay challenge

Exactly one component SHALL hold NIP-42 challenge state, keyed by relay session identity and connection. No publisher, observation owner, transport, or application SHALL retain a challenge, an authentication verdict, or a derived "already authenticated" flag of its own.

#### Scenario: Challenge state exists in exactly one place

- **WHEN** a relay issues a challenge and Fava answers it
- **THEN** the challenge and its verdict are readable only from the authentication owner, and no other component retains either

#### Scenario: A publisher does not authenticate

- **WHEN** a publication attempt meets a relay that demands authentication
- **THEN** the publisher reports that the attempt was unauthenticated and performs no handshake of its own

### Requirement: An application can build an engine that authenticates

An assembled engine SHALL accept one authentication policy, supplied once at assembly, and SHALL apply it to every challenge on every session. An engine built without one SHALL authenticate to nothing rather than authenticate silently.

#### Scenario: A policy supplied at assembly answers a challenge

- **WHEN** an application builds an engine with an authentication policy and a relay challenges an authenticated session
- **THEN** the challenge is answered according to that policy, through the engine's public API alone

#### Scenario: No policy authenticates nothing

- **WHEN** an application builds an engine without an authentication policy and a relay issues a challenge
- **THEN** nothing is signed and nothing is sent

### Requirement: Authentication serves reads and writes on one connection

An authenticated relay session SHALL serve every query and every publication running under that same relay session identity, for the life of that connection. Fava SHALL NOT perform a second handshake for work joining an already authenticated session.

#### Scenario: A query authenticates and a publication reuses it

- **WHEN** a query under an authenticated identity answers a challenge, and a publication under the same identity and relay follows
- **THEN** the publication proceeds without a further challenge exchange

#### Scenario: A publication authenticates and a query reuses it

- **WHEN** a publication under an authenticated identity answers a challenge, and a query under the same identity and relay follows
- **THEN** the query proceeds without a further challenge exchange

### Requirement: The application decides whether to authenticate

Fava SHALL consult the application's policy for each challenge, carrying the relay, the account identity, and the challenge. Fava SHALL NOT authenticate to a relay the policy declines.

#### Scenario: Policy declines an unapproved relay

- **WHEN** a relay Fava has not been told to authenticate to issues a challenge
- **THEN** no signing occurs, no `AUTH` is sent, and the work reports that the policy declined

#### Scenario: Policy approves

- **WHEN** the policy approves a challenge and a signer is attached for the account
- **THEN** Fava signs the challenge response and sends it as that account

#### Scenario: No signer attached

- **WHEN** the policy approves a challenge and no signer is attached for the account
- **THEN** the work reports authentication was not satisfied, without blocking or retrying indefinitely

### Requirement: A person may own the answer

A policy SHALL be able to defer a challenge to a person without blocking, signing, or sending. A deferred demand SHALL be retained with a stable identity, SHALL be enumerable by the application, and SHALL raise a signal when the set changes. Work needing that session SHALL wait under its existing identity rather than fail, and SHALL resume when the demand is answered.

#### Scenario: Deferred challenge signs nothing

- **WHEN** a policy defers a challenge
- **THEN** nothing is signed, no `AUTH` frame is sent, and the demand is enumerable with a stable identity

#### Scenario: An answer resumes waiting work

- **WHEN** a person approves a deferred demand
- **THEN** Fava signs and sends the response, and work that was waiting on that session proceeds

#### Scenario: A publication waits rather than failing

- **WHEN** a publication attempt meets a session whose challenge is deferred
- **THEN** the attempt stays open while the demand is outstanding, rather than being recorded as denied

### Requirement: An answer belongs to the connection it was shown for

A deferred demand SHALL be invalidated when its connection is replaced, and an answer arriving for a replaced connection SHALL resolve nothing. A new connection SHALL begin unauthenticated with a refilled attempt budget.

#### Scenario: A reconnect voids an outstanding demand

- **WHEN** a challenge is deferred and the connection is replaced before a person answers
- **THEN** the demand is dropped, the change is signalled, and a later answer authenticates nothing

#### Scenario: A new connection is challenged afresh

- **WHEN** a session authenticates and then reconnects
- **THEN** the new connection is unauthenticated until the relay challenges it again

### Requirement: A relay's own words survive, and its challenge is refused rather than trimmed

Fava SHALL preserve the relay's own text when it accepts, rejects, or accepts-but-still-refuses. An acceptance that still refuses SHALL be distinguishable from a plain rejection.

A challenge longer than the accepted bound SHALL be refused with a typed error rather than truncated, and truncation SHALL NOT occur anywhere on the path from the socket to that check.

#### Scenario: Acceptance-with-refusal keeps its text

- **WHEN** a relay accepts the authentication and still refuses the work with `restricted:`
- **THEN** the outcome names acceptance-with-refusal and carries the relay's own message

#### Scenario: An oversized challenge is refused, not trimmed

- **WHEN** a relay sends a challenge longer than the accepted bound
- **THEN** nothing is signed, nothing is sent, and the failure names the bound

### Requirement: A relay cannot re-challenge without end

Attempts per connection SHALL be bounded. Once the bound is reached, Fava SHALL stop answering on that connection and record why.

#### Scenario: Endless re-challenge stops at the bound

- **WHEN** a relay challenges repeatedly on one connection
- **THEN** signing stops at the declared bound and the outcome names it

### Requirement: An observation learns why it is not being served

An observation on a relay session that demands authentication SHALL report that fact in its evidence, sourced from the authentication owner's outcome for that session identity. The observation owner SHALL NOT derive it from the wire.

Every authentication outcome SHALL be reachable this way.

#### Scenario: An observation reports the relay's demand

- **WHEN** a relay demands authentication for a session an observation is reading
- **THEN** the observation's evidence for that relay reports it, without the observation owner decoding a challenge

#### Scenario: One account's denial leaves other work running

- **WHEN** authentication is denied for one account
- **THEN** another account's observation, and public-access work on the same relay, continue unaffected

### Requirement: Watching for challenges does not hold a session open

The authentication owner SHALL release its lease on a relay session when no authenticated work remains for that relay.

#### Scenario: The lease is released when the last authenticated work ends

- **WHEN** the last observation and publication under an authenticated identity for one relay end
- **THEN** the session's holder count returns to what it was before authentication began
