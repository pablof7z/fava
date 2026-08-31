## MODIFIED Requirements

### Requirement: A payload that already carries an author is published as its author

An authored event body, an unsigned event, and a pre-signed event have each already settled who signs them. A selection names the account work runs as — the connection it goes over — which is a different fact, so offering such a payload to a selection SHALL be accepted and the payload's own author SHALL be kept.

This replaces the rejection that existed while the publication verb asserted authorship: refusing an authored payload under a selection would refuse publishing one account's event over another account's authenticated connection, which is the point of naming them separately.

#### Scenario: An authored event body is offered to a selection

- **WHEN** a caller offers an authored event body to a publication expression carrying an account selection
- **THEN** the expression accepts it, the event is authored by the payload's author, and the relay session is authenticated as the selected account

#### Scenario: An unsigned or pre-signed event is offered to a selection

- **WHEN** a caller offers an unsigned event or a pre-signed event to a publication expression carrying an account selection
- **THEN** the expression accepts it and publishes it under the author it already carries

#### Scenario: Authored payloads still publish without a selection

- **WHEN** an application publishes an authored event body, an unsigned event, or a pre-signed event through an expression with no selection
- **THEN** publication proceeds using the author the payload already carries
