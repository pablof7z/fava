# 0040 — Transport owns physical session generations

**Status:** approved for implementation, 2026-08-27
**Owner:** `fava-transport`

## Defect

`RelaySessionIdentity` reused the query-owned `Round`. Both
standard and fake transports also restarted each newly registered session at
generation 1 and advanced reconnects with wrapping atomics. Releasing the last
lease and reacquiring the same relay-access key could therefore recreate an
old physical identity, while exhaustion could silently reuse generation 0.

Giving `OpenRelaySession` an `initial_generation` would move the defect across
the boundary: callers do not own socket identity and cannot coordinate all
sessions and reconnects of a replaceable transport.

## Decision

`RelayConnection` identifies one physical connection to a relay. Each
transport instance mints one monotonic sequence across initial connections,
reacquisitions, and reconnects. Registry removal does not reset it.
`OpenRelaySession` remains generation-free.

The transport reserves a reconnect successor before opening network work. A
successful socket installs that reserved identity. Failed work may leave a gap;
an identity is never repeated. Exhaustion refuses an initial acquisition with
`TransportError::GenerationExhausted`, or terminally reports
`RelayInbound::ReconnectExhausted` with zero attempts and
`TransportFailure::GenerationExhausted` before opening reconnect work.

`HandoffCorrelation` remains caller-owned. It is an opaque token echoed by one
handoff outcome, not physical session identity. Callers use checked or
structurally bounded construction and never saturate into duplicate tokens.

## Falsifiers

- Release and reacquire one key; the physical generation must change.
- Force initial generation exhaustion; no dial or registry entry may appear.
- Force reconnect generation exhaustion; no reconnect dial or `Reconnected`
  item may appear, and the terminal item must name the last live identity.
- Minting after the maximum generation must refuse rather than wrap or repeat.
- The same handoff correlation remains valid under distinct exact session
  identities, while each outcome echoes its caller's token unchanged.
