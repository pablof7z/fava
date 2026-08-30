## Purpose

Defines who Fava authenticates as on a relay connection, when it answers a NIP-42 challenge, what it does when the relay refuses or the connection is replaced, and how each outcome reaches query and publication evidence.

## ADDED Requirements

### Requirement: One owner holds every relay challenge

Exactly one owner SHALL hold NIP-42 challenge state, keyed by relay session identity and transport session generation. No publisher, query owner, transport, or application SHALL retain a challenge, an authentication verdict, or a derived "already authenticated" flag of its own.

#### Scenario: Challenge state exists in exactly one place

- **WHEN** a relay issues a challenge and Fava answers it
- **THEN** the challenge and its verdict are readable only from the authentication owner, and no other component retains either

#### Scenario: A publisher does not authenticate

- **WHEN** a publisher's attempt meets a relay that demands authentication
- **THEN** the publisher reports that the attempt was unauthenticated and performs no handshake of its own

### Requirement: Authentication serves reads and writes on one connection

An authenticated relay session SHALL serve every query and every publication that runs under that same relay session identity, for the life of that connection. Fava SHALL NOT perform a second handshake for work that joins an already authenticated session.

#### Scenario: A query authenticates and a publication reuses it

- **WHEN** a query under an authenticated identity answers a challenge, and a publication under the same identity and relay follows
- **THEN** the publication proceeds without a further challenge exchange

#### Scenario: A publication authenticates and a query reuses it

- **WHEN** a publication under an authenticated identity answers a challenge, and a query under the same identity and relay follows
- **THEN** the query proceeds without a further challenge exchange

### Requirement: The application decides whether to authenticate

Fava SHALL consult one application-supplied policy for each challenge, carrying the relay, the account identity, and the challenge. Fava SHALL NOT authenticate to a relay the policy declines. A policy is supplied once for the engine and SHALL apply to queries and publications alike.

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

A policy SHALL be able to defer a challenge to a person without blocking, signing, or sending. A deferred demand SHALL be retained with a stable identity, SHALL be enumerable by the application, and SHALL raise a signal when the set of deferred demands changes. Work needing that session SHALL park under its existing identity rather than fail, and SHALL resume when the demand is answered.

#### Scenario: Deferred challenge signs nothing

- **WHEN** a policy defers a challenge
- **THEN** no signing occurs and no `AUTH` frame is sent, and the demand is listed as awaiting an answer

#### Scenario: A publication waits for the person, then completes

- **WHEN** a publication meets a relay demanding authentication, the policy defers, and the person later approves
- **THEN** the publication does not fail while the demand is outstanding, and it completes after the session authenticates

#### Scenario: Refusing after deferral

- **WHEN** a deferred demand is answered with a refusal
- **THEN** no signing occurs and the work reports that authentication was declined

#### Scenario: Query evidence distinguishes who is being waited on

- **WHEN** a challenge is deferred to a person
- **THEN** query evidence reports it as awaiting an answer, distinct from awaiting a relay verdict

### Requirement: A deferred demand does not outlive its connection

A deferred demand SHALL be scoped to the session generation it arrived on. When that generation is replaced the demand SHALL be dropped and the change signalled. An answer to a dropped demand SHALL resolve nothing and SHALL NOT authenticate any session.

#### Scenario: Reconnect drops an outstanding demand

- **WHEN** a demand is deferred and the connection is replaced before it is answered
- **THEN** the demand no longer appears as pending and the application is signalled

#### Scenario: A stale answer authenticates nothing

- **WHEN** an answer arrives for a demand whose generation was replaced
- **THEN** no signing occurs, no session is marked authenticated, and the answer is reported as no longer applicable

### Requirement: Every authentication outcome is exact and attributable

Each attempt SHALL resolve to exactly one terminal outcome: accepted, declined, rejected by the relay, accepted but still refused for that account, or failed. A rejection or refusal message SHALL be retained verbatim within a bounded size. Every outcome SHALL be visible to the query and publication work that ran under that session.

#### Scenario: Relay refuses an authenticated account

- **WHEN** the relay accepts the challenge response but still refuses the request for that account
- **THEN** the work reports acceptance-with-refusal as distinct from rejection, carrying the relay's own message

#### Scenario: Query evidence reports how far authentication reached

- **WHEN** a query runs against a relay that demanded authentication
- **THEN** its evidence reports which stage authentication reached, and stages beyond a received challenge are reachable

### Requirement: Denial is scoped to one access context

A declined, rejected, or failed authentication SHALL terminate only the work running under that exact relay session identity. Work under a different account, or under public access, SHALL continue unaffected.

#### Scenario: One account is denied while another proceeds

- **WHEN** two observations run against the same relay under two different authenticated accounts and one is refused
- **THEN** the refused observation reports its outcome and the other continues delivering results

#### Scenario: Public work is unaffected

- **WHEN** authenticated work against a relay is refused
- **THEN** public-access work against the same relay continues

### Requirement: A replaced connection is not authenticated

A new transport session generation SHALL begin unauthenticated. Fava SHALL treat authentication established on an earlier generation as spent, and SHALL answer a fresh challenge on the new generation under the same policy.

#### Scenario: Reconnect requires authenticating again

- **WHEN** an authenticated session is lost and replaced by a new generation
- **THEN** work on the new generation is unauthenticated until a fresh challenge is answered

#### Scenario: A stale verdict cannot authenticate new work

- **WHEN** an authentication outcome from a previous generation arrives after the connection was replaced
- **THEN** it is discarded and does not mark the current generation authenticated

### Requirement: Challenges and re-authentication are bounded

A relay-supplied challenge SHALL be accepted only within an explicit size bound and refused, never truncated, when it exceeds it or is empty. Authentication attempts SHALL be bounded per session generation, so a relay that re-challenges without end cannot cause unbounded signing.

#### Scenario: Oversized challenge is refused

- **WHEN** a relay sends a challenge larger than the accepted bound
- **THEN** Fava refuses it with a typed outcome and does not sign a response

#### Scenario: Repeated challenges stop being answered

- **WHEN** a relay issues challenges repeatedly on one connection beyond the attempt bound
- **THEN** Fava stops answering and reports that the bound was reached

### Requirement: Authentication identity is not authorship

The account Fava authenticates as SHALL be a distinct value from the author of any event it publishes. A payload that already carries its own author SHALL keep it.

#### Scenario: Publishing one account's event over another's session

- **WHEN** an event authored by one account is published under a selection naming a different account
- **THEN** the event remains authored and signed by the first account, and the relay session is authenticated as the second
