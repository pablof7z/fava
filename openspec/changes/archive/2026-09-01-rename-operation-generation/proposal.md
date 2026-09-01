## Why

`OperationGeneration` is a stale, overloaded name for the query owner's fencing token: one value bounds all currently authorized read work in a relay slot, spanning several `OperationName` calls, rather than identifying one operation or a transport connection. `Round` names that validity interval in a word nobody has to be taught: a completion from an old round is discarded. It is deliberately not `Attempt`, which publication already uses for one delivery try.

The same complaint applies to `RelaySessionGeneration`, and worse. That one means *which connection* -- it advances on every reconnect and every reacquisition -- so it can simply be called `RelayConnection`. Two types shared the word `generation` for unrelated things, and a reader had to know the prefix to tell which. Both are renamed here rather than in two passes, because the transport carries both and touching it twice costs more than doing it once.

## What Changes

- **BREAKING** Replace the public `OperationGeneration`, `OperationGenerationIssuer`, and `OperationGenerationExhausted` APIs with `Round`, `RoundIssuer`, and `RoundsExhausted`; remove every old re-export with no alias or compatibility path.
- **BREAKING** Rename `RelaySessionGeneration` to `RelayConnection`, and `RelaySessionIdentity::generation` to `connection`. It identifies one physical connection to a relay, which is what the field is for and what every doc comment already explains in longhand.
- Rename the associated fields, parameters, accessors, local state, diagnostic labels, and error text from `generation` to `round` where they carry the query-work fence, and to `connection` where they carry a relay connection.
- Preserve the identity's owner, authority-and-sequence representation, non-forgeability, checked allocation, exhaustion refusal, cancellation boundary, and exact stale-completion rejection.
- Correct query evidence and diagnostic documentation so a round is not described as a transport connection; update the focused current issue records that name either superseded API.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a vocabulary-only API break: it changes no runtime behavior, protocol interaction, persisted representation, or behavioral requirement. `skip_specs: true` is set accordingly.

## Impact

The renamed cross-crate identity originates in `fava-query` and flows through `fava-runtime` provider completions, `fava-observe` relay-work state, query evidence, diagnostics, subscriptions, and the public `fava` facade. Tests, doctests, Bazel declarations, and focused issue documentation must use the replacement vocabulary. No dependencies, wire formats, persistence formats, or lifecycle semantics change.
