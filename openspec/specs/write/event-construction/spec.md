# write/event-construction Specification

## Purpose
Defines how an unsigned Nostr event body is assembled from application-level fields, at which point the event's author becomes part of that body, and which construction states are permitted to produce a deterministic event id.

## Requirements

### Requirement: Event construction begins without an author

Beginning a new event body SHALL require only its kind. The resulting builder describes a body — timestamp, content, tags, and local publication route — and states no identity.

#### Scenario: A new builder is begun from a kind alone

- **WHEN** a caller begins a new event body for a given kind
- **THEN** construction succeeds without the caller supplying any public key

#### Scenario: Body fields accumulate on an authorless builder

- **WHEN** a caller sets timestamp, content, tags, and an explicit relay route on an authorless builder, in any order
- **THEN** each field is retained exactly as supplied, tags in their input order, and no identity is required at any step

### Requirement: An authorless body cannot produce an event id

A deterministic event id is derived from the complete event including its author. A body that has not been given an author SHALL NOT be able to produce an unsigned event or an event id.

#### Scenario: Finalization is unavailable without an author

- **WHEN** a caller holds an authorless event body
- **THEN** no operation is available that yields an unsigned event or a deterministic event id

#### Scenario: The absent-author state is not a runtime refusal

- **WHEN** a caller attempts to finalize an authorless event body
- **THEN** the attempt is rejected at compile time, not reported as a runtime error at finalization

### Requirement: An author converts a body into a finalizable event

Supplying an author to an authorless event body SHALL yield an authored body that retains every field already set and can produce an unsigned event with its deterministic id.

#### Scenario: Supplying an author preserves the accumulated body

- **WHEN** a caller supplies an author to a body that already carries a timestamp, content, tags, and a relay route
- **THEN** the authored body carries the same kind, timestamp, content, tag sequence, and route, and additionally the supplied author

#### Scenario: The authored body yields the same id as before this change

- **WHEN** an authored body is finalized
- **THEN** the resulting unsigned event and its deterministic id are byte-identical to what construction with an up-front author produced previously

#### Scenario: Bounds are still checked once, at finalization

- **WHEN** an authored body exceeding the declared tag-count or serialized-byte bound is finalized
- **THEN** finalization is refused with the corresponding bound refusal, reporting the actual and maximum values

### Requirement: Reconstructing an existing event yields an authored body

Construction from exact raw event parts, and reopening a finalized unsigned event, both carry an author by definition. These SHALL yield an authored body directly, without an intervening authorless state.

#### Scenario: Construction from exact raw parts is authored

- **WHEN** a caller constructs a body from an exact author, kind, timestamp, tag sequence, and content
- **THEN** the result is an authored body that can be finalized immediately

#### Scenario: Reopening an unsigned event preserves its author

- **WHEN** a caller reopens a finalized unsigned event for further construction
- **THEN** the result is an authored body carrying that event's author, kind, timestamp, tag order, and content, with the derived id discarded and routing reset to automatic

#### Scenario: A reconstructed body re-derives the original id

- **WHEN** a caller reopens an unsigned event and finalizes it without modifying any body field
- **THEN** the re-derived event id equals the original event's id

### Requirement: Explicit routing remains incompatible with event-only finalization

An explicit relay route is local to a publication expression and is not part of the signed event. Finalizing to an event alone SHALL continue to refuse when an explicit route would be discarded.

#### Scenario: Event-only finalization refuses to discard an explicit route

- **WHEN** an authored body carrying an explicit relay route is finalized to an event alone
- **THEN** finalization is refused because the explicit route would be silently discarded
