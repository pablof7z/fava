# transport/session-protocol Specification

## Purpose
Defines what a relay session accepts from the components using it — protocol values rather than encoded envelopes — what each request yields back, and how a subscription ends.

## Requirements

### Requirement: A relay session accepts protocol values, not envelopes

A relay session SHALL accept the client messages Fava sends as protocol values: filters to subscribe with, a signed event to publish, and a signed event answering an authentication challenge. The session SHALL own envelope construction, encoding, and the outbound frame bound.

No component outside the transport SHALL construct or encode a client message in order to reach a relay. A relay session SHALL NOT offer a byte-level send.

#### Scenario: Publishing names an event, not a frame

- **WHEN** a component publishes a signed event to a relay session
- **THEN** it supplies the event itself, and the exact bytes on the wire are chosen by the session

#### Scenario: No component builds an envelope

- **WHEN** the workspace is searched for client-message construction or client-message encoding outside the transport, excluding code that measures an encoded length without sending
- **THEN** no occurrence is found

#### Scenario: The outbound frame bound applies to what the session encodes

- **WHEN** a request would encode to more than the session's declared maximum frame size
- **THEN** the request is refused with a typed error naming the bound, and nothing is written to the socket

### Requirement: Each request reports its own outcome without a caller token

Each request SHALL report the handoff outcome of the frame it produced: that the bytes definitely did not leave Fava, that the session accepted the complete frame, or that whether the relay received it cannot be proven. Those three SHALL remain distinct.

A caller SHALL NOT supply a correlation token to identify its own request, and no outcome SHALL carry one. Awaiting the request is the correlation.

#### Scenario: The three handoff outcomes stay distinct

- **WHEN** a request is refused locally, accepted, and left unprovable in three separate attempts
- **THEN** each reports a distinct outcome, and a local refusal is never reported as unprovable

#### Scenario: Two concurrent requests do not need tokens to be told apart

- **WHEN** two requests are in flight on one session and one is refused locally while the other is accepted
- **THEN** each caller receives its own outcome, with no token supplied by either

### Requirement: The session names the subscription

A relay session SHALL choose the identifier for each subscription it opens. A caller SHALL supply the filters and SHALL NOT supply an identifier. The chosen identifier SHALL be readable from the handle the request yields.

Identifiers SHALL be opaque — carrying no plan, filter, or observation information — and SHALL be unique among the subscriptions live on a session, by construction rather than by detecting a collision. They SHALL be of a fixed width the session declares, within the length every relay is obliged to accept, so that the exact encoded length of a subscription request is derivable before the identifier exists.

#### Scenario: Opening a subscription supplies filters only

- **WHEN** a component opens a subscription
- **THEN** it supplies the filters, and the identifier on the wire is chosen by the session and readable from the handle

#### Scenario: Two subscriptions never share an identifier

- **WHEN** many subscriptions are opened and closed on one session over its lifetime
- **THEN** no two simultaneously live subscriptions carry the same identifier

#### Scenario: Encoded length is derivable before an identifier exists

- **WHEN** a component measures what a subscription request will encode to, before opening it
- **THEN** the declared identifier width makes the measurement exact, matching the frame the session later produces

### Requirement: A request yields the replies belonging to it

Opening a subscription SHALL yield a handle delivering exactly the relay messages correlated to that subscription's identifier. Publishing an event, and answering an authentication challenge, SHALL each yield a handle delivering exactly the relay's acknowledgement of that event.

Each handle SHALL deliver its own narrow item type, exposing only what can actually arrive for it. A subscription SHALL deliver an event, the end of stored events, the relay's closure of it, exact bounded loss, or the end of its generation. An acknowledgement SHALL settle as accepted, as rejected, or as the end of its generation.

The end of a generation SHALL name whether the connection dropped or the reconnect budget was exhausted.

#### Scenario: A subscription's messages arrive on its own handle

- **WHEN** two subscriptions are open on one session and the relay sends an event, an end-of-stored-events, and a closure for one of them
- **THEN** all three arrive on that subscription's handle and none on the other's

#### Scenario: An acknowledgement arrives on the request that sent the event

- **WHEN** two different events are published on one session and the relay acknowledges one of them
- **THEN** the acknowledgement arrives on the handle for that event alone

#### Scenario: A handle exposes only what can reach it

- **WHEN** a caller matches exhaustively on what a subscription handle delivers
- **THEN** no arm exists for an acknowledgement, a challenge, or a message belonging to another subscription

#### Scenario: An ended generation names its cause

- **WHEN** one handle ends because the connection dropped and another because the reconnect budget was exhausted
- **THEN** the two endings are distinguishable without inspecting text

### Requirement: Acknowledgements fan out rather than exclude

A request naming an event another handle is already awaiting the acknowledgement of SHALL be accepted, not refused. Every live handle awaiting an event's acknowledgement SHALL receive it.

#### Scenario: Two callers publishing one event both learn its outcome

- **WHEN** two components publish the same signed event on one session and the relay acknowledges it
- **THEN** both handles report the relay's verdict

### Requirement: A subscription cannot stop being read while the relay sends it

Closing a subscription SHALL send the relay's closure and SHALL report that frame's handoff outcome. Releasing a subscription handle without closing it SHALL enqueue the same closure without waiting for its outcome, so that no subscription is left streaming to a component that has stopped reading.

Releasing a handle whose generation has already ended SHALL send nothing, because the relay did not carry that subscription across the connection.

#### Scenario: Closing reports whether the closure left

- **WHEN** a component closes a subscription
- **THEN** it learns whether the closure frame was handed off, refused, or left unprovable

#### Scenario: Releasing a handle still tells the relay

- **WHEN** a subscription handle is released without being closed
- **THEN** the relay receives the closure and stops sending that subscription

#### Scenario: Releasing after a reconnect sends nothing

- **WHEN** a subscription handle whose generation has ended is released
- **THEN** no closure is sent

### Requirement: Relay challenges reach one named reader

A relay session SHALL expose the relay's authentication challenges through their own accessor. A challenge SHALL NOT be delivered mixed with other messages for a component to filter out, and SHALL NOT require a component to have sent anything to receive it.

#### Scenario: A challenge arrives without a prior request

- **WHEN** a relay issues an unsolicited authentication challenge and one component is reading challenges
- **THEN** that component receives it, having sent nothing to correlate it to

#### Scenario: A challenge does not reach a subscription or an acknowledgement

- **WHEN** a relay issues a challenge while a subscription and a publication are live on the same session
- **THEN** neither handle observes it
