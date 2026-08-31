# write/edit-vocabulary Specification

## Purpose
Fixes the names the write-edit surface presents to anyone reading or calling it: what an opaque protocol change is called, what the thing that consumes one is called, and what identifies each resulting event revision.

## Requirements

### Requirement: An opaque protocol change is an event edit

The value a protocol crate constructs to express a change to a replaceable event SHALL be named for what it is — an event edit — without restating that the subsystem only handles replaceable kinds.

#### Scenario: A protocol crate constructs an edit

- **WHEN** a caller invokes a typed edit constructor such as following a public key, bookmarking an event, or saving a group
- **THEN** the returned value is an event edit, named without a replaceability qualifier

#### Scenario: The edit remains opaque to the facade

- **WHEN** the facade accepts an event edit for publication
- **THEN** it carries the same kind, optional discriminator, and opaque change bytes as before, and the facade still cannot interpret the change

### Requirement: The thing that consumes an edit applies it

The contract that owns a kind's write semantics SHALL be named for the action it performs. It applies an edit to the event that currently exists and returns the event that should exist next.

#### Scenario: An implementation declares its kind and applies an edit

- **WHEN** an implementation of the contract is asked which kind it owns, whether it supports a given edit, and to apply that edit against a current event, an author, and a timestamp
- **THEN** each operation is named for what it does, with the applying operation named `apply`

#### Scenario: Application against empty state

- **WHEN** an edit is applied for a coordinate that has no current event
- **THEN** the implementation applies it against protocol-defined empty state, unchanged from current behavior

#### Scenario: Refusals name the same terms

- **WHEN** publication refuses an edit because no implementation claims its kind, or because the claiming implementation does not support that edit
- **THEN** the refusal message uses the same vocabulary the contract uses, with no reference to materialization

### Requirement: Each application of an edit produces an identified revision

The generation counter identifying one immutable rebuild of a write's event SHALL be named for the revision it identifies.

#### Scenario: The first revision of an accepted write

- **WHEN** a write is accepted and its edit applied for the first time
- **THEN** the resulting event carries the first revision identity

#### Scenario: Re-application advances the revision

- **WHEN** a write's edit is applied again against a changed current event
- **THEN** the resulting event carries the next revision identity, and recovery compares against the revision it expects

#### Scenario: Persisted values are unchanged

- **WHEN** a write store reads back a revision identity written before this change
- **THEN** the value is unchanged, because only the type name differs and the serialized form is the same nonzero integer

### Requirement: Registration is named for what is registered

The facade's registration methods SHALL name the thing being registered rather than who defines it.

#### Scenario: An assembly registers an implementation for a kind

- **WHEN** an assembly registers one implementation, or several, that own the write semantics for their kinds
- **THEN** the methods are named for edit appliers rather than for materializers

#### Scenario: Duplicate and overflow refusals are unchanged

- **WHEN** two registered implementations claim the same kind, or the number registered exceeds the declared bound
- **THEN** assembly is refused exactly as before, with the refusal text using the new vocabulary

### Requirement: Behavior is unchanged

This change SHALL alter names only. No observable behavior, wire format, event id, persisted value, or refusal condition changes.

#### Scenario: Every event is byte-identical

- **WHEN** any edit is applied after this change with the same inputs as before it
- **THEN** the resulting unsigned event and its deterministic id are byte-identical

#### Scenario: No signature changes beyond names

- **WHEN** the renamed declarations are compared against their originals
- **THEN** each has the same parameters, argument order, return type, and error type, differing only in the identifiers used
