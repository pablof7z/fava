# identity/relay-authentication Specification

## Purpose
Defines who Fava authenticates as on a relay connection, when it answers a NIP-42 challenge, what happens when the relay refuses, a person owns the decision, or the connection is replaced, and how each outcome reaches query evidence.

## Requirements

### Requirement: One owner holds every relay challenge

Exactly one component SHALL decide what to do about a relay's challenge and answer it. The challenge itself is the connection's own state; no publisher, observation owner, or application SHALL keep a copy of a challenge or a verdict of its own.

#### Scenario: Challenge state exists in exactly one place

- **WHEN** a relay issues a challenge and Fava answers it
- **THEN** the challenge and its verdict are readable only from the connection, and no component keeps a second copy

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

The application SHALL decide whether to authenticate, and as which account. The decision SHALL be made synchronously and perform no effects. A decision naming an account SHALL be answered as that account; refusing SHALL leave the connection refused; declining to decide SHALL leave the connection as it is, awaiting a person, without signing or sending anything.

#### Scenario: The policy names the account

- **WHEN** a relay challenges a connection and the application's policy chooses to authenticate
- **THEN** the answer is signed as the account the policy named

#### Scenario: Deciding nothing sends nothing

- **WHEN** the application's policy neither authenticates nor refuses
- **THEN** nothing is signed, nothing is sent, and the connection stays as the relay left it

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

### Requirement: A relay's own words survive, and its challenge is refused rather than trimmed

Fava SHALL preserve the relay's own text when authentication fails. Whether the relay rejected outright or accepted and still refused the work, the failure SHALL be reported through one outcome, distinguished only by the relay's own words — no separate state SHALL exist for one machine-readable prefix over another.

A challenge longer than the accepted bound SHALL be refused with a typed error rather than truncated, and truncation SHALL NOT occur anywhere on the path from the socket to that check.

#### Scenario: Acceptance-with-refusal keeps its text

- **WHEN** a relay accepts the authentication and still refuses the work with `restricted:`
- **THEN** the failure carries the relay's own message verbatim, and no separate outcome exists to name the distinction

#### Scenario: A rejection and an acceptance-with-refusal are told apart only by their words

- **WHEN** a relay refuses authentication, whether by rejecting outright or by accepting and still refusing the work
- **THEN** both are reported through the same failure, and it is the relay's own words that distinguish them, not a separate state

#### Scenario: An oversized challenge is refused, not trimmed

- **WHEN** a relay sends a challenge longer than the accepted bound
- **THEN** nothing is signed, nothing is sent, and the failure names the bound

### Requirement: A relay cannot re-challenge without end

Attempts per connection SHALL be bounded. Once the bound is reached, Fava SHALL stop answering on that connection and record why.

#### Scenario: Endless re-challenge stops at the bound

- **WHEN** a relay challenges repeatedly on one connection
- **THEN** signing stops at the declared bound and the outcome names it

### Requirement: An observation learns why it is not being served

An observation on a relay session that demands authentication SHALL report that fact in its evidence, read from the connection it already holds rather than from a separate channel carrying a copy of the same fact.

#### Scenario: An observation reports the relay's demand

- **WHEN** a relay demands authentication for a session an observation is reading
- **THEN** the observation's evidence for that relay reports it, read from the connection itself rather than decoded from the wire or fetched from elsewhere

#### Scenario: One account's denial leaves other work running

- **WHEN** authentication is denied for one account
- **THEN** another account's observation, and public-access work on the same relay, continue unaffected

### Requirement: An answer names what it applies to, even when that is nothing

An answer given for a connection that has since been replaced SHALL report that it applies to nothing, rather than reporting that no such demand was ever made.

#### Scenario: A stale answer names its own irrelevance

- **WHEN** an answer is given for a connection that has since been replaced
- **THEN** it is reported that the answer applies to nothing, not that no demand exists
