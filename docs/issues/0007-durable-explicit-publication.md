# M5: durable explicit-route publication

**Status:** complete
**Authority:** `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, M5

## Product result

Fava durably accepts unsigned or verified signed events for an exact relay set,
returns a stable receipt, exposes the local event immediately, and retains exact
per-relay outcomes. Unsigned events never enter `EventCache`; only verified
relay echoes do. An accepted obligation resumes after process death without
application resubmission.

## Architecture

- `WriteStore` commits the obligation, current event, routing, and receipt
  identity before `publish` returns acceptance.
- The event author selects the `Signer`. `EventBuilder` remains generic and
  interprets no protocol kind.
- `Publisher` owns one attempt to one exact relay session. `DeliveryPolicy`
  alone decides whether a definite pre-handoff failure permits another attempt.
- `Publication` owns lifecycle ordering while `WriteStore`, `Signer`,
  `Publisher`, `DeliveryPolicy`, and `Transport` retain their own state.
- Receipt changes use a bounded causal broadcast. Committed transitions are
  ordered; a slow reader receives an explicit lag error rather than silent
  current-state coalescing. Removal is a distinct `(ReceiptId, None)` fact.
- Exact explicit routes bypass automatic routers.
- Explicit publication is bounded to 256 relays, receipt text to 4,096 bytes,
  and causal receipt delivery to 256 retained changes before explicit lag.
- `open_receipts` exposes every currently open obligation, including an
  unsigned event parked because its exact author signer is unavailable.
- Pre-handoff cancellation retracts the write source, records one terminal
  receipt, performs no publication handoff, and remains idempotent. Receipt
  removal is a separate operation.
- The Redb write store is the standard durable profile. The memory write store
  remains available for volatile and deterministic test assemblies.

## Exit-gate evidence

- Public-facade tests prove optimistic visibility, cache separation, exact mixed
  relay outcomes, ordered receipt transitions, explicit bounded lag,
  stalled-write inspection, cancellation, idempotence, and removal.
- The Redb process-kill corpus sends real `SIGKILL` at pre-acceptance,
  acceptance, signature, attempt/effect ambiguity, outcome, and cancellation
  boundaries and verifies the exact recovered receipt at each boundary.
- `explicit-publish-optimistic`, `mixed-relay-outcomes`,
  `cancel-pre-handoff`, and `crash-after-acceptance` exercise the public facade
  against disposable real relays. The crash canary restarts from the same Redb
  file and publishes without application resubmission.
- Canary evidence exposes receipt and relay facts, not internal attempts or
  execution lanes.

## Falsifier evidence

Skipping the Redb acceptance commit while returning the allocated receipt made
the process-kill acceptance boundary recover no receipt. Restoring the atomic
receipt and next-id commit made the same boundary pass.
