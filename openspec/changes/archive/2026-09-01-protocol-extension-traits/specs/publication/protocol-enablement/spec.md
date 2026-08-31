## Purpose

Defines how an application turns on a protocol crate's write semantics at assembly, and what the application is allowed to see of the mechanism that makes it work.

## ADDED Requirements

### Requirement: A protocol is enabled by name, not by handler

An application SHALL enable a protocol crate's write semantics by naming the protocol, not by obtaining and passing that crate's edit handler.

#### Scenario: Enabling simple groups

- **WHEN** an application assembles the facade and enables simple groups
- **THEN** it does so with a single call named for the protocol, and never names an edit applier, an event kind, or a handler type

#### Scenario: Enabling several protocols

- **WHEN** an application enables more than one protocol
- **THEN** each is a separate call of the same shape, and their order does not change the resulting facade

#### Scenario: Enabling nothing

- **WHEN** an application enables no protocol and publishes an edit
- **THEN** publication is refused because no handler claims that kind, unchanged from current behavior

### Requirement: The handler is not part of a protocol crate's public surface

A protocol crate SHALL NOT expose its edit handler, nor a factory returning one. The only way out of the crate is the enabling call.

#### Scenario: A consumer looks for the handler

- **WHEN** the public surface of a protocol crate is inspected
- **THEN** it contains no function returning an edit applier, and no edit-applier type

#### Scenario: The enabling call is the whole seam

- **WHEN** an application enables a protocol
- **THEN** the value passed across the crate boundary carries no operation the application can use for anything else

### Requirement: Enabling is written against a neutral sink

The enabling call SHALL be expressed against a neutral acceptor owned by the crate that owns the edit-applier contract, so that enabling a protocol does not require the protocol crate to depend on the facade.

#### Scenario: Protocol crate dependencies are unchanged

- **WHEN** a protocol crate's dependency set is inspected after gaining its enabling call
- **THEN** it is unchanged from before, and does not include the universal facade

#### Scenario: The facade accepts what protocol crates offer

- **WHEN** the facade's builder is offered a protocol crate's handler through the neutral acceptor
- **THEN** it indexes it exactly as it indexes one supplied directly by an application

### Requirement: Applications defining their own kinds still register directly

An application that owns the edit semantics for its own kind SHALL still be able to register its handler directly. That caller holds a real handler and is not hidden from the contract.

#### Scenario: An application registers its own handler

- **WHEN** an application implements the edit-applier contract for a kind it defines and registers it at assembly
- **THEN** the handler is indexed and edits of that kind can be published and recovered

#### Scenario: Both routes share one index

- **WHEN** an application enables a protocol crate and also registers a handler of its own
- **THEN** both land in the same index under the same duplicate-kind and count refusals, with neither taking precedence

### Requirement: A forgotten enabling call fails, and fails where it can

Forgetting to enable a protocol SHALL NOT silently produce wrong behavior.

#### Scenario: Outstanding writes fail at assembly

- **WHEN** a facade is assembled with an outstanding stored write whose kind no enabled protocol claims
- **THEN** assembly fails, because recovery runs during assembly and cannot apply that edit

#### Scenario: A fresh application fails at first publish

- **WHEN** an application with no outstanding writes forgets to enable a protocol and then publishes one of its edits
- **THEN** publication is refused naming the unclaimed kind, and no write is accepted
