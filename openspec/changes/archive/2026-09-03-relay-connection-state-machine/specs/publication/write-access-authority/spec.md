## MODIFIED Requirements

### Requirement: A write is accepted under one relay authority and keeps it

A write SHALL be accepted under exactly one relay authority — no authentication, or authentication as one exact account — and SHALL keep it for its life. The authority SHALL be a requirement the write states of any connection that carries it, not a name selecting one. A connection SHALL carry the write only while it can still satisfy that requirement.

#### Scenario: A write states its authority rather than naming a connection

- **WHEN** a write accepted under an account is routed to a relay
- **THEN** it is carried by a connection that is, or can still become, authenticated as that account

#### Scenario: A write requiring no authority is never carried by an authenticated connection

- **WHEN** a write accepted under no authority is routed to a relay whose only connection is authenticated
- **THEN** it is not carried by that connection

#### Scenario: An accepted write keeps its authority

- **WHEN** a write is accepted under one account's authority and the application then selects a different account
- **THEN** the accepted write is still delivered under the authority it was accepted under

#### Scenario: The authority is one value beside the author

- **WHEN** a write is accepted under an account whose key differs from the event's author
- **THEN** both are recorded, separately, and neither is derived from the other

### Requirement: A parked write resumes under its own authority

A write parked for want of authentication SHALL wait for the connection carrying it to satisfy its authority, and SHALL resume under that authority alone. It SHALL NOT consume a retry budget while waiting, and no policy SHALL retire it. It SHALL fail when the connection reaches a state from which its authority can no longer be satisfied.

#### Scenario: A parked write does not spend attempts

- **WHEN** a write meets a relay demanding authentication and waits
- **THEN** its attempt count does not advance and no policy gives up on it

#### Scenario: A refused authentication fails the write

- **WHEN** the connection a parked write waits on has its authentication refused, rejected, or failed
- **THEN** the write fails, naming that authentication was required and did not happen

#### Scenario: A write parked for a signer resumes authenticated

- **WHEN** a write accepted under an authenticated account is parked awaiting a signer, the process restarts, and the signer is attached
- **THEN** the write resumes and is delivered under the authority it was accepted under

#### Scenario: Restart does not default an authority to public

- **WHEN** a store containing an authenticated write is reopened by a process with no account selected
- **THEN** the write's authority is read from the store rather than defaulted
