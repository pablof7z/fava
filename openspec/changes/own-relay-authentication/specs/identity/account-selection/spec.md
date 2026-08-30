## Purpose

Defines how one account selection reaches both the relay session identity and event authorship across queries and publications, what happens when nothing is selected, and how a selection survives a restart.

## ADDED Requirements

### Requirement: One verb selects the account for any work

Fava SHALL expose exactly one way to name the account a query or publication runs as. That selection SHALL supply the relay access authority for the work, and the author of a payload that carries none. Fava SHALL NOT expose a separate verb for authorship and another for relay identity.

#### Scenario: Selection reaches a query

- **WHEN** an observation is opened under a named account
- **THEN** its relay work runs under that account's access authority

#### Scenario: Selection reaches a publication

- **WHEN** an authorless payload is published under a named account
- **THEN** the event is authored by that account and its relay work runs under that account's access authority

#### Scenario: An authored payload keeps its author

- **WHEN** a payload that already carries an author is published under a different named account
- **THEN** the event keeps its own author and only the relay access authority comes from the selection

### Requirement: Work defaults to the current account

Work with no explicit selection SHALL run as the currently selected account. Work with no explicit selection and no current account SHALL run under public relay access, and SHALL refuse before acceptance when an authorless payload has no author to resolve.

#### Scenario: Query follows the current account

- **WHEN** an account is selected and an observation is opened without naming one
- **THEN** the observation runs under that account's access authority

#### Scenario: Publication follows the current account

- **WHEN** an account is selected and an authorless payload is published without naming one
- **THEN** the event is authored by that account

#### Scenario: No account selected

- **WHEN** no account is selected and an observation is opened without naming one
- **THEN** the observation runs under public relay access

#### Scenario: No author to resolve

- **WHEN** no account is selected and an authorless payload is published without naming one
- **THEN** Fava returns an immediate typed refusal before accepting the work

### Requirement: An accepted write keeps the account that accepted it

The account a write was accepted under SHALL be durable with that write. A later selection change, signer replacement, account removal, process exit, or restart SHALL NOT retarget accepted work to a different account, and SHALL NOT silently downgrade it to public relay access.

#### Scenario: Parked write resumes after restart under its own account

- **WHEN** a write accepted under an account parks awaiting a signer, the process exits, and the store is reopened
- **THEN** the resumed write publishes under that same account's access authority

#### Scenario: Switching accounts does not retarget accepted work

- **WHEN** a write is accepted under one account and a different account is selected before delivery settles
- **THEN** the write's author and access authority remain those of the accepting account

#### Scenario: A store from an earlier format refuses to open

- **WHEN** a write store written under an earlier persisted format is opened
- **THEN** it is refused with an exact reason rather than partially loaded

### Requirement: Automatic routing carries the selected access authority

Route selection for a write SHALL carry the access authority the write was accepted under. Routing SHALL NOT assume public access for writes.

#### Scenario: Write routes to an authenticated destination

- **WHEN** a write accepted under an account is routed automatically
- **THEN** its selected destinations execute under that account's access authority

#### Scenario: Write with no account routes publicly

- **WHEN** a write accepted with no account selection is routed automatically
- **THEN** its selected destinations execute under public access
