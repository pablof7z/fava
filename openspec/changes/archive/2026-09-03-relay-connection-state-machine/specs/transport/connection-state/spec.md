## Purpose

One relay connection knows where it has got to, and says so. Everything that needs a relay in a particular condition waits for that condition rather than being told about it by a third party.

## ADDED Requirements

### Requirement: A connection carries its connectivity and its authentication

A connection SHALL hold exactly two states: whether it is reachable, and how far authentication has got on it. Neither SHALL be derived, copied, or maintained anywhere else. Authentication belongs to one connection: when a connection is replaced, the replacement SHALL begin with no authentication, and nothing proved to the relay on the previous one SHALL carry over.

#### Scenario: Reconnecting forgets what was proved

- **WHEN** a connection authenticated as an account is replaced
- **THEN** the replacement reports no authentication, and the relay is free to challenge it again

#### Scenario: One place holds the fact

- **WHEN** any component asks how far authentication has got on a relay
- **THEN** it reads the connection, and no component holds a second copy to reconcile

### Requirement: A disconnection names whether a reconnect may still follow

A disconnected connection SHALL report either how many attempts its reconnect budget spent before giving up — meaning no further connection will appear — or no count at all, meaning a reconnect may still be attempted. A reconnect budget SHALL be able to give up having spent zero attempts, so a reader SHALL tell "exhausted" apart from "still trying" by whether a count is present, never by its value.

#### Scenario: A connection gives up after spending attempts

- **WHEN** a connection's reconnect budget is exhausted after spending some attempts
- **THEN** it reports disconnected naming that count, and no further connection follows

#### Scenario: A connection may still return

- **WHEN** a connection drops but a reconnect may still be attempted
- **THEN** it reports disconnected with no attempt count

#### Scenario: A budget can be exhausted having spent nothing

- **WHEN** a reconnect budget is exhausted without any attempt being spent
- **THEN** the disconnected state still names a count of zero, rather than carrying no count, so it is not mistaken for a connection that may still return

### Requirement: A change to a connection is a signal carrying the current state

A component SHALL be able to wait for a connection's state to change and read what it became, without polling and without missing the current value by arriving late. A reader that arrives after a change SHALL see the state as it now is rather than a queue of what it was.

#### Scenario: A late reader sees the present

- **WHEN** a component begins watching a connection that has already authenticated
- **THEN** it reads the authenticated state immediately, without waiting for another change

#### Scenario: Repeated states do not accumulate

- **WHEN** a relay sends the same challenge repeatedly on one connection
- **THEN** a watcher is not woken once per repetition, and no queue grows

### Requirement: Work names the connection it needs, and is served by one that can reach it

Work SHALL state what it requires of a connection rather than naming a connection. A connection SHALL serve that work when it can still reach the required state. A connection already authenticated as one account SHALL NOT serve work requiring no authentication or requiring another account, because it can no longer reach either. When no connection can serve the work, one SHALL be opened.

#### Scenario: An unauthenticated connection serves work that will authenticate

- **WHEN** work requiring authentication as an account meets a connection that is connected and unauthenticated
- **THEN** that connection serves it, and authenticates as that account when challenged

#### Scenario: A connection committed to one account does not serve another

- **WHEN** work requiring authentication as one account meets a connection authenticated as a different one
- **THEN** that connection does not serve it, and a connection is opened for it instead

#### Scenario: Anonymous work is not carried by an authenticated connection

- **WHEN** work requiring no authentication meets a connection authenticated as an account
- **THEN** that connection does not serve it, so the relay never sees the two on one connection

### Requirement: The relay's demand is a state, not a message to be delivered

A relay asking for authentication SHALL move the connection to a state carrying the challenge. It SHALL NOT be queued for delivery to a named reader, SHALL NOT be readable by a component that is not deciding it, and SHALL NOT survive the connection it arrived on.

#### Scenario: A challenge does not outlive its connection

- **WHEN** a relay challenges a connection and that connection is replaced before anything decides
- **THEN** the challenge is gone with it, and nothing records it against the replacement

### Requirement: The transport announces which session a relay has just asked

A transport SHALL publish, for each session whose relay demands authentication, that a demand now exists there — so the component that decides does not need to hold or poll connections to discover it. Each demand SHALL be announced once. A relay repeating the identical challenge on the same connection SHALL NOT be announced again. A connection that replaces another SHALL be announced afresh if its relay asks, even if the challenge text repeats what the replaced connection was asked.

#### Scenario: A challenge is announced once

- **WHEN** a relay challenges a session
- **THEN** the transport announces that session exactly once for that challenge

#### Scenario: A repeated identical challenge is not announced again

- **WHEN** a relay sends the same challenge again on the same connection
- **THEN** no further announcement is made

#### Scenario: A replacement connection is announced afresh

- **WHEN** a connection is replaced and the new connection is challenged, even with the same challenge text as before
- **THEN** the new session is announced

### Requirement: The transport answers what connections it holds

A transport SHALL be able to say which sessions it currently holds, so a component that owns no connection can still read what one is doing. Nothing SHALL poll this.

A session SHALL appear for exactly as long as something holds it: once the last holder releases a connection, neither it nor anything it knew SHALL be reported. A reader asking about a relay nothing is connected to SHALL be told that, and SHALL NOT be given the last state a closed connection reached.

#### Scenario: A listener that fell behind finds the demands it missed

- **WHEN** the component that decides authentication learns it missed announcements
- **THEN** it can ask which held connections are still waiting to be answered, without waiting for a repetition the relay has no reason to send

#### Scenario: A released connection reports nothing

- **WHEN** the last holder of a connection releases it and a reader then asks about that relay
- **THEN** no session is reported for it, and the authentication the closed connection had reached is not reported either

### Requirement: Waiting work resumes because the connection moved

Work waiting for a connection to reach a state SHALL resume when it reaches it, and SHALL fail when the connection reaches a state it can no longer be served from. Nothing SHALL tell the waiting work what happened; it SHALL observe the connection itself. The component deciding authentication SHALL NOT know what work is waiting.

#### Scenario: A held publication proceeds on authentication

- **WHEN** a publication is waiting for a connection to authenticate and the relay accepts the answer
- **THEN** the publication proceeds, and the deciding component was not told a publication existed

#### Scenario: A held publication fails on refusal

- **WHEN** a publication is waiting for a connection to authenticate and the answer is declined, rejected, or fails
- **THEN** the publication fails, naming that authentication was required and did not happen
