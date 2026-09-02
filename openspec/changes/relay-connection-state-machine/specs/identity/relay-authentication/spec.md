## MODIFIED Requirements

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

### Requirement: One owner holds every relay challenge

Exactly one component SHALL decide what to do about a relay's challenge and answer it. The challenge itself is the connection's own state; no publisher, observation owner, or application SHALL keep a copy of a challenge or a verdict of its own.

#### Scenario: Challenge state exists in exactly one place

- **WHEN** a relay issues a challenge and Fava answers it
- **THEN** the challenge and its verdict are readable only from the connection, and no component keeps a second copy

#### Scenario: A publisher does not authenticate

- **WHEN** a publication attempt meets a relay that demands authentication
- **THEN** the publisher reports that the attempt was unauthenticated and performs no handshake of its own

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

### Requirement: An observation learns why it is not being served

An observation on a relay session that demands authentication SHALL report that fact in its evidence, read from the connection it already holds rather than from a separate channel carrying a copy of the same fact.

#### Scenario: An observation reports the relay's demand

- **WHEN** a relay demands authentication for a session an observation is reading
- **THEN** the observation's evidence for that relay reports it, read from the connection itself rather than decoded from the wire or fetched from elsewhere

#### Scenario: One account's denial leaves other work running

- **WHEN** authentication is denied for one account
- **THEN** another account's observation, and public-access work on the same relay, continue unaffected

## ADDED Requirements

### Requirement: An answer names what it applies to, even when that is nothing

An answer given for a connection that has since been replaced SHALL report that it applies to nothing, rather than reporting that no such demand was ever made.

#### Scenario: A stale answer names its own irrelevance

- **WHEN** an answer is given for a connection that has since been replaced
- **THEN** it is reported that the answer applies to nothing, not that no demand exists

## REMOVED Requirements

### Requirement: Watching for challenges does not hold a session open

**Reason**: It specifies a resource leak rather than a behavior. The component that decides authentication took its own lease on the connection it watched, and this requirement bounded the damage: a poll sampling whether it was the last holder, a debounce so the poll did not act on a race, and bookkeeping so two watches did not hold each other open. None of it is about authentication. With that component attending a connection it does not own, there is nothing to release.

**Migration**: The component no longer opens or holds connections, so its holder count is always zero and no test asserts a return to a prior value.

### Requirement: An answer belongs to the connection it was shown for

**Reason**: The intent survives and moves to `transport/connection-state`, where authentication belongs to one connection and a replacement begins with none. The requirement as written specifies the mechanism that stood in for it — a connection counter mirrored into the authentication owner and compared on every read — which exists only because two components each held their own idea of which connection was current.

**Migration**: An answer for a connection that has been replaced resolves nothing, because the state it would write belongs to a connection that no longer exists. This is now a property of where the state lives rather than a comparison each caller performs.
