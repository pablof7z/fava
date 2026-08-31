## Purpose

Defines which relay authority a write is accepted under, how that survives durable custody and a process restart, and what happens when a stored write's authority is missing or contradicts where it was routed.

## ADDED Requirements

### Requirement: A write is accepted under one relay authority and keeps it

A write SHALL record the relay access authority it was accepted under, and SHALL be delivered under that authority for its whole life. A later change of selection, signer, or current account SHALL NOT retarget work already accepted.

#### Scenario: An accepted write keeps its authority

- **WHEN** a write is accepted under one account's authority and the application then selects a different account
- **THEN** the accepted write is still delivered under the authority it was accepted under

#### Scenario: The authority is one value beside the author

- **WHEN** a write is accepted under an account whose key differs from the event's author
- **THEN** both are recorded, separately, and neither is derived from the other

### Requirement: Routing a write requests the authority it was accepted under

A write's route request SHALL carry its accepted authority, and routing SHALL select destinations under it. A write accepted under an authenticated account SHALL NOT be routed as public work.

#### Scenario: An authenticated write routes to an authenticated session

- **WHEN** an automatically routed write was accepted under an authenticated account
- **THEN** its destinations are relay sessions under that account's authority

#### Scenario: A public write routes as public

- **WHEN** a write was accepted with no account selected
- **THEN** its destinations are public relay sessions, unchanged from current behaviour

### Requirement: A parked write resumes under its own authority

A write parked awaiting a signer or a route SHALL resume under the authority recorded at acceptance, including after a process restart. It SHALL NOT resume as public work because the process that resumed it had no selection.

#### Scenario: A write parked for a signer resumes authenticated

- **WHEN** a write accepted under an authenticated account is parked awaiting a signer, the process restarts, and the signer is attached
- **THEN** the write resumes and is delivered under the authority it was accepted under

#### Scenario: Restart does not default an authority to public

- **WHEN** a store containing an authenticated write is reopened by a process with no account selected
- **THEN** the write's authority is read from the store rather than defaulted

### Requirement: A stored authority that cannot be trusted is refused, not defaulted

Reconstructing a stored write SHALL refuse when its access authority is absent, malformed, or contradicts the destinations the write was routed to. It SHALL NOT fall back to public access.

#### Scenario: An absent authority is refused

- **WHEN** a stored row carries no access authority
- **THEN** reconstruction refuses with a named error and the write is not delivered

#### Scenario: A malformed authority is refused

- **WHEN** a stored row's access authority cannot be decoded
- **THEN** reconstruction refuses with a named error

#### Scenario: An authority contradicting its destinations is refused

- **WHEN** a stored row's access authority names one account and its routed destinations are sessions under another
- **THEN** reconstruction refuses rather than choosing one of the two

### Requirement: A store written by an earlier build refuses to open

Carrying the authority changes the persisted row shape. A store stamped with an earlier schema version SHALL refuse to open with a named error rather than partially deserializing.

#### Scenario: An earlier store refuses with a named error

- **WHEN** a store written under the previous schema version is opened
- **THEN** opening fails with an error naming the version mismatch, and no row is read
