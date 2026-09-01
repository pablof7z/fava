## REMOVED Requirements

### Requirement: A payload that already carries an author refuses an author scope

**Reason**: The rejection was correct while the publication verb asserted authorship — offering an authored payload to an author scope named two authors, which is a contradiction. `Fava::with_account` names the account work runs as, which is the relay authority the work goes over, not who signs. Under that verb the rejection falls exactly on publishing one account's event over another account's authenticated connection, which is the case naming them separately exists to allow.

**Migration**: An authored payload offered to a selection is now accepted and keeps its own author. Callers that relied on the rejection to catch a double-named author no longer need to: there is no second author to conflict with, because the selection names a connection.

## ADDED Requirements

### Requirement: A payload that already carries an author is published as its author

An authored event body, an unsigned event, and a pre-signed event have each already settled who signs them. A selection names the account work runs as — the connection it goes over — which is a different fact, so offering such a payload to a selection SHALL be accepted and the payload's own author SHALL be kept.

#### Scenario: An authored event body is offered to a selection

- **WHEN** a caller offers an authored event body to a publication expression carrying an account selection
- **THEN** the expression accepts it, the event is authored by the payload's author, and the relay session is authenticated as the selected account

#### Scenario: An unsigned or pre-signed event is offered to a selection

- **WHEN** a caller offers an unsigned event or a pre-signed event to a publication expression carrying an account selection
- **THEN** the expression accepts it and publishes it under the author it already carries

#### Scenario: Authored payloads still publish without a selection

- **WHEN** an application publishes an authored event body, an unsigned event, or a pre-signed event through an expression with no selection
- **THEN** publication proceeds using the author the payload already carries
