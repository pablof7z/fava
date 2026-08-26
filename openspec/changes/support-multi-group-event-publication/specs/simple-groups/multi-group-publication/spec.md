## Purpose

Define how one bounded event-building expression acquires several simple-group contexts and their exact relay destinations while preserving one event identity, one publication lifecycle, and the separation between signed Nostr data and local routing intent.

## ADDED Requirements

### Requirement: Simple-group composition remains an event builder
The simple-groups capability SHALL provide an extension method that accepts a `SimpleGroup`, appends its publication contribution, and returns the same concrete `EventBuilder` type. Route-accumulation refusals SHALL return the existing `WriteIntentError` directly. The generic event-building owner SHALL expose only neutral event-tag and explicit-relay composition; it MUST NOT depend on or interpret simple groups.

#### Scenario: One group is appended
- **WHEN** an application applies `.simple_group(group)` to an `EventBuilder`
- **THEN** the result is an `EventBuilder` containing that group's exact `h` context and host contribution

#### Scenario: Builder methods remain available
- **WHEN** an application applies `.simple_group(group)` and then invokes an ordinary event-builder method
- **THEN** the ordinary method remains available on the same concrete builder type

### Requirement: One event can carry several simple-group contexts
Repeated simple-group composition SHALL produce one unsigned event containing one exact `h` tag for each distinct selected group id in first-selection order. Repeating the same group id SHALL NOT append another `h` tag, and every host supplied for that id SHALL still contribute to the publication route.

#### Scenario: Two distinct groups share one event identity
- **WHEN** an application composes group A and group B into one builder and publishes it
- **THEN** one event containing exact `h` tags for A and B is signed once and the same event id and signature are submitted for both groups

#### Scenario: The same group id contributes additional hosts
- **WHEN** two selected `SimpleGroup` values have the same id and overlapping or additional hosts
- **THEN** the event contains one `h` tag for that id and the route contains every distinct supplied host once

### Requirement: Group hosts become neutral explicit routing
Each simple-group composition SHALL add the group's complete host sequence to builder-carried publication intent without serializing those relay destinations into the Nostr event. The exact route SHALL preserve caller first-occurrence order, collapse duplicate relay identities, and obey the universal explicit-relay bound.

#### Scenario: Routing does not alter the signed event
- **WHEN** two builders have identical event fields and identical group ids but different host relay sets
- **THEN** they build the same Nostr event id while retaining different explicit publication routes

#### Scenario: Shared hosts are delivered once
- **WHEN** several selected groups include the same relay
- **THEN** publication submits the event to that relay once and the durable route records that relay once

#### Scenario: Route bound is exceeded
- **WHEN** a group contribution would increase the normalized explicit route beyond its declared bound
- **THEN** composition returns `WriteIntentError::TooManyExplicitRelays` before signing, durable custody, or relay work

### Requirement: Fava publishes a routed event builder directly
The universal Fava facade SHALL accept `EventBuilder` as a publication payload. A builder with no embedded explicit route SHALL use automatic routing; a builder with group-contributed hosts SHALL use that complete explicit route and bypass automatic routers. Successful admission SHALL return the ordinary `Write` and retain the exact route in the ordinary durable receipt lifecycle.

#### Scenario: Multi-group builder publication
- **WHEN** an application calls `fava.publish(builder)` after selecting one or more simple groups
- **THEN** Fava durably accepts one write for the built event and its complete embedded explicit route

#### Scenario: Plain builder publication
- **WHEN** an application calls `fava.publish(builder)` without adding an explicit route
- **THEN** Fava uses the configured automatic router chain

#### Scenario: Restart recovery
- **WHEN** a multi-group builder publication is durably accepted and Fava restarts before settlement
- **THEN** recovery resumes the same event identity and exact normalized relay route without reconstructing group objects

### Requirement: Publication routing has one authority
Builder-carried explicit routing and an external explicit publication scope SHALL be mutually exclusive. Fava MUST refuse any expression that supplies both before signing or durable custody, even when both routes contain the same relays.

#### Scenario: Builder route conflicts with `to`
- **WHEN** an application passes a builder carrying group-derived routing to `fava.to(relays).publish(builder)`
- **THEN** Fava returns a typed conflicting-route refusal and performs no signer, store, publication, or relay work

### Requirement: Event-only construction cannot silently discard routing
An event-only build terminal SHALL refuse a builder carrying explicit publication routing unless the caller uses an explicit operation whose contract consumes or deliberately removes that route. No successful builder operation may silently drop group-contributed relay intent.

#### Scenario: Routed builder uses event-only build
- **WHEN** an application invokes the event-only build terminal after selecting a simple group
- **THEN** the operation returns a typed refusal containing no built event

### Requirement: Pre-signed events remain immutable
A pre-signed event SHALL never acquire or lose group tags through publication composition. Simple-group validation SHALL verify the signature, require the selected exact `h` context, tolerate valid sibling group contexts, and return the exact original event; the application SHALL provide its complete explicit relay route separately.

#### Scenario: Pre-signed event contains several valid groups
- **WHEN** a valid signed event contains exact `h` tags for groups A and B and is validated for either group
- **THEN** validation returns the byte-exact original event without rejecting the valid sibling context

#### Scenario: Pre-signed event lacks the selected group
- **WHEN** a valid signed event lacks the exact `h` context for the selected group
- **THEN** validation refuses before Fava custody or relay work

### Requirement: Multi-group relay behavior is falsifiable
The capability SHALL prove through a controlled relay path that every selected host receives the same signed event and that each selected group can retrieve it by its exact `h` filter. Fava SHALL preserve actual per-relay acceptance and retrieval evidence and MUST NOT infer per-group acceptance from routing alone.

#### Scenario: Controlled two-group publication
- **WHEN** one event is published to two groups hosted on controlled relay paths
- **THEN** both exact group queries return the same event id and the evidence identifies the relays that actually accepted and served it

#### Scenario: Relay accepts only one group interpretation
- **WHEN** a destination accepts or serves the event for fewer group contexts than were selected
- **THEN** Fava reports only the observed relay outcomes and does not claim successful membership for the unproved group
