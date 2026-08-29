## Purpose

Defines what a protocol capability is — an opaque claim on one event kind's write semantics — how a protocol crate declares one, and what a consumer of that crate can observe about it.

## ADDED Requirements

### Requirement: A capability is an opaque claim on one kind

A capability SHALL wrap exactly one implementation of the edit-applying contract and expose nothing about it. A consumer holding a capability learns neither which kind it claims nor how the claimed kind's edits work.

#### Scenario: A consumer inspects a capability

- **WHEN** an application obtains a capability from a protocol crate
- **THEN** the value offers no operation that reveals a kind number, the applier contract, or the encoding of that kind's edits

#### Scenario: The claim is still readable by the owner of the index

- **WHEN** the write-edit subsystem indexes a capability
- **THEN** it can read the claimed kind from within the capability, because the index is what the claim exists for

#### Scenario: A capability carries no mutable state

- **WHEN** the same capability is indexed by two independent assemblies in one process
- **THEN** neither assembly observes state from the other

### Requirement: An application can wrap its own applier as a capability

An application defining edit semantics for its own kind SHALL be able to produce a capability from its implementation, so that self-registered and application-defined claims are the same kind of value.

#### Scenario: An application wraps its implementation

- **WHEN** an application constructs a capability from its own edit applier
- **THEN** the resulting value is indistinguishable in type and handling from one a protocol crate declares

#### Scenario: There is one registration vocabulary

- **WHEN** an application compares registering its own semantics against enabling a protocol crate's
- **THEN** both are expressed as capabilities, with no separate shipped-versus-application path

### Requirement: A protocol crate declares its capability once

A protocol crate SHALL declare its claim with a single declaration, without exposing an applier factory, a kind constant, or any other implementation term in its public surface.

#### Scenario: A protocol crate declares a claim

- **WHEN** a protocol crate declares the capability for the kind it owns
- **THEN** the declaration is the crate's only statement about registration, and its public surface gains no applier factory

#### Scenario: The claimed kind stays private to the crate

- **WHEN** a consumer inspects the protocol crate's public surface
- **THEN** the kind number the crate claims is not exposed there, unchanged from the crate's existing encapsulation of its kind constants

#### Scenario: Declaration does not depend on the facade

- **WHEN** a protocol crate declares its capability
- **THEN** it does so through the crate that owns the edit-applying contract, and does not take a dependency on the universal facade

### Requirement: A declared capability is collected at link time

Declaring a capability SHALL make it available to any assembly in a program that links the declaring crate, with no statement required at the assembly site.

#### Scenario: Linking a protocol crate enables its kind

- **WHEN** a program depends on a protocol crate that declares a capability, and assembles the facade naming nothing
- **THEN** edits of that crate's kind can be published and recovered

#### Scenario: Not linking the crate leaves the kind unclaimed

- **WHEN** a program does not depend on a protocol crate
- **THEN** that crate's kind is unclaimed, and publishing an edit of it is refused as unclaimed rather than silently accepted

#### Scenario: Collection happens before recovery

- **WHEN** an assembly completes and recovers outstanding writes
- **THEN** every capability declared by a linked crate is already indexed, so recovery of an outstanding edit of that kind finds its applier
