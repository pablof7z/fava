## REMOVED Requirements

### Requirement: Relay challenges reach one named reader

**Reason**: A challenge is not a message addressed to a component. It is what the connection now is, and it is stated as such in `transport/connection-state`. Delivering it as a queued message let it outlive the connection it arrived on, and let a component that was not deciding it read it.

**Migration**: The component that decides authentication observes the connection's state instead of reading a queue. The exactly-one-reader property is stronger rather than weaker: the state carries the connection it belongs to, so it cannot be attributed to another.

## ADDED Requirements

### Requirement: A session can refuse a challenge without sending anything

The session SHALL offer a way to record that the application refused to authenticate, distinct from not having decided yet. Refusing SHALL send no frame.

#### Scenario: Refusal is distinguishable from silence

- **WHEN** the application refuses a challenge
- **THEN** work waiting on that connection can tell that a decision was made, and no frame reaches the relay
