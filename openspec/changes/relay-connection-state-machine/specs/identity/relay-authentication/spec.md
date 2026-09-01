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

## REMOVED Requirements

### Requirement: Watching for challenges does not hold a session open

**Reason**: It specifies a resource leak rather than a behavior. The component that decides authentication took its own lease on the connection it watched, and this requirement bounded the damage: a poll sampling whether it was the last holder, a debounce so the poll did not act on a race, and bookkeeping so two watches did not hold each other open. None of it is about authentication. With that component attending a connection it does not own, there is nothing to release.

**Migration**: The component no longer opens or holds connections, so its holder count is always zero and no test asserts a return to a prior value.

### Requirement: An answer belongs to the connection it was shown for

**Reason**: The intent survives and moves to `transport/connection-state`, where authentication belongs to one connection and a replacement begins with none. The requirement as written specifies the mechanism that stood in for it — a connection counter mirrored into the authentication owner and compared on every read — which exists only because two components each held their own idea of which connection was current.

**Migration**: An answer for a connection that has been replaced resolves nothing, because the state it would write belongs to a connection that no longer exists. This is now a property of where the state lives rather than a comparison each caller performs.
