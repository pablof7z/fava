# identity/account-selection Specification

## Purpose
Defines how one selection names the account work runs as — the relay-session authority a read or a write goes over — and how an event's author is resolved when the payload carries none.

## Requirements

### Requirement: One verb names the account work runs as

An application SHALL name the account through one selection that applies to reads and writes alike. There SHALL NOT be one verb for a query's relay authority and another for a publication's.

The selected account SHALL determine the relay-session access authority. Work with no selection SHALL run under the session's current account, and under public access when there is none.

#### Scenario: A selection reaches a query's relay session

- **WHEN** an application opens a query under a selected account
- **THEN** the query's relay sessions carry that account's access authority

#### Scenario: The same selection reaches a publication

- **WHEN** an application publishes under a selected account
- **THEN** the publication's relay sessions carry that same account's access authority

#### Scenario: No selection falls back in a stated order

- **WHEN** work is opened with no account selected
- **THEN** it runs under the session's current account, and under public access when no current account exists

### Requirement: Whose event it is and whose connection it goes over are separate

The selected account SHALL name the connection. A payload that states its own author SHALL keep it. Publishing one account's event over another account's connection SHALL be accepted, because a relay authenticates a connection and an event carries its own signature.

#### Scenario: An authored payload published under another account

- **WHEN** an application publishes a payload authored by one account under a selection naming a different account
- **THEN** the event is authored by the payload's author and the relay session is authenticated as the selected account

#### Scenario: An authored payload is not rejected for carrying its author

- **WHEN** an application offers a payload that already states its author to a selection
- **THEN** the selection accepts it rather than refusing it as a contradiction

### Requirement: An authorless payload resolves its author in one stated order

A payload that states no author SHALL take one: the selected account, then the session's current account. A payload with no author and neither of those SHALL be refused before any durable custody is taken.

#### Scenario: The selected account authors an authorless payload

- **WHEN** an application publishes an authorless payload under a selected account
- **THEN** the event is authored by that account

#### Scenario: The current account authors an authorless payload

- **WHEN** an application publishes an authorless payload with no selection and a current account set
- **THEN** the event is authored by the current account

#### Scenario: Nothing to author with is refused before custody

- **WHEN** an application publishes an authorless payload with no selection and no current account
- **THEN** publication is refused with a typed error, and no write identifier, receipt identifier, or durable custody is produced
