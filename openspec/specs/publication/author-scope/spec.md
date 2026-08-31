# publication/author-scope Specification

## Purpose
Defines how a publication expression supplies an author to a payload that does not carry one, and which payload kinds accept an author scope versus already having settled their identity.

## Requirements

### Requirement: An author scope is the single door for authorless payloads

Every payload that describes an event without stating who signs it SHALL receive its author from the publication expression's author scope, and from nowhere else.

#### Scenario: An authorless event body is published under an author scope

- **WHEN** an application publishes an authorless event body through an expression carrying an author scope
- **THEN** the published event is authored by that scope's public key

#### Scenario: A replaceable edit is published under an author scope

- **WHEN** an application publishes a replaceable event edit through an expression carrying an author scope
- **THEN** the edit is applied to that author's prior event, unchanged from current behavior

#### Scenario: Authorless payloads of both kinds take the same door

- **WHEN** an application compares publishing an authorless event body with publishing a replaceable edit
- **THEN** both supply their author through the same author-scope expression, with no second mechanism for either

### Requirement: Publishing an authorless payload without an author scope is refused

An authorless payload has no identity of its own. Publishing one outside an author scope SHALL be refused before any durable custody is taken.

#### Scenario: An authorless event body is published with no author scope

- **WHEN** an application publishes an authorless event body through an expression with no author scope
- **THEN** publication is refused with the missing-author refusal, and no write is accepted

#### Scenario: Refusal precedes durable custody

- **WHEN** publication is refused for a missing author
- **THEN** no write identifier or receipt identifier is produced, and nothing is handed to the publication owner

### Requirement: A payload that already carries an author refuses an author scope

An authored event body, an unsigned event, and a pre-signed event have each already settled their identity. Offering such a payload to an author scope SHALL be rejected rather than silently overriding or silently ignoring the scope.

#### Scenario: An authored event body is offered to an author scope

- **WHEN** a caller offers an authored event body to a publication expression carrying an author scope
- **THEN** the expression rejects it, on the grounds that the payload already carries its author

#### Scenario: An unsigned or pre-signed event is offered to an author scope

- **WHEN** a caller offers an unsigned event or a pre-signed event to a publication expression carrying an author scope
- **THEN** the expression rejects it, unchanged from current behavior

#### Scenario: Authored payloads publish without a scope

- **WHEN** an application publishes an authored event body, an unsigned event, or a pre-signed event through an expression with no author scope
- **THEN** publication proceeds using the author the payload already carries

### Requirement: An author scope composes with an explicit relay route

Supplying an author and narrowing the relay route are independent concerns of one publication expression, and SHALL remain composable in either order for authorless payloads.

#### Scenario: Author scope and relay scope combine in either order

- **WHEN** an application narrows a publication to an exact relay sequence and to an exact author, in either order, and publishes an authorless payload
- **THEN** the published event carries that author and is routed to exactly that relay sequence

#### Scenario: Conflicting explicit routes are still refused

- **WHEN** an authorless event body carrying its own explicit relay route is published through an expression that also narrows the route
- **THEN** publication is refused for conflicting explicit routes, unchanged from current behavior

### Requirement: Protocol event constructors state no author

A constructor that describes a protocol event for an application to publish SHALL NOT accept an author. Identity is supplied at publication.

#### Scenario: A group management constructor takes no author

- **WHEN** an application constructs a NIP-29 management event for a group
- **THEN** the constructor accepts only the arguments that carry protocol meaning, and no public key

#### Scenario: Construction and publication are separable

- **WHEN** an application constructs a protocol event once and publishes it under a chosen author
- **THEN** the same constructed value can be published under a different author without reconstructing it

#### Scenario: Constructors that reconstruct a specific event still take an author

- **WHEN** an internal caller rebuilds a specific existing event, such as applying a replaceable edit onto a prior event or answering a relay authentication challenge
- **THEN** that caller supplies the exact author required for the resulting event id to match, through the authored construction path
