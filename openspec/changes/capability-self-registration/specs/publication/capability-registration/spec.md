## Purpose

Defines how declared and explicitly named capabilities reach the facade's kind index at assembly, and how the facade refuses conflicts and absences without ever interpreting a kind.

## ADDED Requirements

### Requirement: The facade claims no kind of its own

The universal facade SHALL ship no edit applier and SHALL NOT depend on any protocol crate. Every claim in its index comes from a capability supplied to it.

#### Scenario: The facade names no protocol

- **WHEN** the facade's dependencies are inspected
- **THEN** no protocol crate appears among them, and no kind number is written in the facade

#### Scenario: All claim slots are available

- **WHEN** an assembly registers capabilities
- **THEN** the full declared bound of claims is available to it, with none consumed before it begins

#### Scenario: An assembly with no capabilities publishes no edits

- **WHEN** a program links no capability-declaring crate and names none at assembly, then publishes an edit
- **THEN** publication is refused because no implementation claims that kind

### Requirement: Declared and named capabilities land in one index

Capabilities collected from linked crates and capabilities named at the assembly site SHALL be indexed together under identical rules.

#### Scenario: A named capability joins the collected ones

- **WHEN** an assembly names a capability for its own kind while linked crates declare capabilities for theirs
- **THEN** all of them are indexed, and edits of every claimed kind can be published

#### Scenario: Order of arrival does not matter

- **WHEN** capabilities arrive from linking and from the assembly site in any order
- **THEN** the resulting index is the same

#### Scenario: The index maps kind to implementation only

- **WHEN** the facade routes an edit for publication or recovery
- **THEN** it looks the edit's kind up in the index and invokes the claimed implementation, without interpreting the kind or the edit's change bytes

### Requirement: A kind is claimed once

Two capabilities claiming the same kind SHALL refuse the assembly. No capability overrides another, regardless of whether it was declared by a linked crate or named at the assembly site.

#### Scenario: Two linked crates claim one kind

- **WHEN** a program links two crates that both declare a capability for the same kind
- **THEN** assembly is refused as a duplicate claim, naming the kind

#### Scenario: A named capability collides with a declared one

- **WHEN** an assembly names a capability for a kind a linked crate already declares
- **THEN** assembly is refused as a duplicate claim, and the named one does not take precedence

#### Scenario: Exceeding the claim bound

- **WHEN** the number of distinct claims exceeds the declared bound
- **THEN** assembly is refused, reporting the actual count against the bound

### Requirement: Refusals are assembly-time or publish-time, never silent

A conflict SHALL be refused when the facade is assembled. An unclaimed kind SHALL be refused when an edit of it is published or recovered. Neither is ever resolved by guessing.

#### Scenario: Conflicts surface before any write is accepted

- **WHEN** an assembly carries conflicting claims
- **THEN** it fails to assemble, and no write store, publication owner, or recovery has run

#### Scenario: An unclaimed kind is refused at publish

- **WHEN** an application publishes an edit whose kind no capability claims
- **THEN** publication is refused as unclaimed, and no write is accepted

#### Scenario: An outstanding edit of an unclaimed kind is refused at recovery

- **WHEN** an assembly recovers a write outstanding from a previous run whose kind no longer has a claim, because the declaring crate is no longer linked
- **THEN** recovery refuses rather than dropping or silently completing the write

### Requirement: An application publishes typed edits without naming a capability twice

Enabling a protocol is a dependency decision, not a per-call one. Publishing SHALL require no capability argument, no kind, and no applier.

#### Scenario: Publishing a typed edit

- **WHEN** an application constructs a typed edit from a protocol crate and publishes it under an author
- **THEN** the facade routes it by its kind alone, with the application naming nothing about capabilities at the call site

#### Scenario: The application never names the kind

- **WHEN** an application enables a protocol and publishes its edits
- **THEN** at no point does the application write the kind number, mirroring the protocol crate's own encapsulation of it
