## Purpose

Defines how one publication attempt learns its own outcome on a relay connection it shares with queries and other attempts, and what may and may not end that wait.

## ADDED Requirements

### Requirement: An attempt's outcome comes only from its own acknowledgement

A publication attempt SHALL derive its outcome solely from the relay's acknowledgement of the exact event it sent, or from the transport facts of its own handoff. No message correlated to other work on the same connection SHALL determine the attempt's outcome.

#### Scenario: Another attempt's acknowledgement is not this attempt's outcome

- **WHEN** two attempts for different events share one connection and the relay acknowledges the other event
- **THEN** this attempt keeps awaiting its own acknowledgement and reports no outcome

#### Scenario: A relay notice does not end an attempt

- **WHEN** a relay sends a notice while an attempt awaits its acknowledgement
- **THEN** the attempt is unaffected and continues awaiting

#### Scenario: An unsolicited authentication challenge does not end an attempt

- **WHEN** a relay issues an authentication challenge while an attempt whose handoff was accepted awaits its acknowledgement
- **THEN** the attempt is unaffected and continues awaiting, rather than reporting that authentication was required

#### Scenario: An undecodable frame does not end an attempt

- **WHEN** the relay sends bytes that do not decode while an attempt awaits its acknowledgement
- **THEN** the attempt is unaffected and continues awaiting

### Requirement: Volume of unrelated traffic never bounds an attempt

The wait for an acknowledgement SHALL be bounded by the attempt's own deadline and by the liveness of its connection. It SHALL NOT be bounded by a count of received messages, and the outcome SHALL NOT depend on how much traffic belonging to other work crossed the connection while it waited.

#### Scenario: A busy connection does not change an outcome

- **WHEN** an attempt's acknowledgement arrives after an arbitrarily large volume of unrelated subscription traffic and within the attempt's deadline
- **THEN** the attempt reports the acknowledged or rejected outcome the relay actually sent

#### Scenario: The same publication behaves identically on a shared and an idle connection

- **WHEN** the same event is published on a connection carrying heavy unrelated traffic and on an otherwise idle connection, with the relay answering identically in both
- **THEN** both attempts report the same outcome

### Requirement: An attempt with no acknowledgement stays ambiguous

An attempt whose event was handed off but whose acknowledgement never arrived — because its deadline elapsed, its connection ended, or the reconnect budget was exhausted — SHALL report an unknown outcome naming which of those occurred. It SHALL NOT be reported as acknowledged, as rejected, or as never sent.

#### Scenario: Deadline elapses after a successful handoff

- **WHEN** an attempt's event is handed off and its deadline elapses with no acknowledgement
- **THEN** the attempt reports an unknown outcome naming the elapsed deadline

#### Scenario: The connection ends before an acknowledgement

- **WHEN** an attempt's event is handed off and the connection ends before the relay acknowledges it
- **THEN** the attempt reports an unknown outcome naming the disconnection, and never reports rejection
