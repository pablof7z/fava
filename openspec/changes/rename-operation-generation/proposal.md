## Why

`OperationGeneration` is a stale, overloaded name for the query owner's fencing token: one value bounds all currently authorized read work in a relay slot, spanning several `OperationName` calls, rather than identifying one operation or a transport connection. `WorkEpoch` names that validity interval and keeps it distinct from an observation, a subscription-plan revision, and `RelaySessionGeneration`.

## What Changes

- **BREAKING** Replace the public `OperationGeneration`, `OperationGenerationIssuer`, and `OperationGenerationExhausted` APIs with `WorkEpoch`, `WorkEpochIssuer`, and `WorkEpochExhausted`; remove every old re-export with no alias or compatibility path.
- Rename the associated fields, parameters, accessors, local state, diagnostic labels, and error text from `generation` to `epoch` where they carry this query-work fence.
- Preserve the identity's owner, authority-and-sequence representation, non-forgeability, checked allocation, exhaustion refusal, cancellation boundary, and exact stale-completion rejection.
- Correct query evidence and diagnostic documentation so this epoch is not described as a transport connection generation; update the focused current issue records that name the superseded API.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a vocabulary-only API break: it changes no runtime behavior, protocol interaction, persisted representation, or behavioral requirement. `skip_specs: true` is set accordingly.

## Impact

The renamed cross-crate identity originates in `fava-query` and flows through `fava-runtime` provider completions, `fava-observe` relay-work state, query evidence, diagnostics, subscriptions, and the public `fava` facade. Tests, doctests, Bazel declarations, and focused issue documentation must use the replacement vocabulary. No dependencies, wire formats, persistence formats, or lifecycle semantics change.
