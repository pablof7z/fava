# 0036 — Symbol Gate reviews generic owners, not binder names

**Status:** approved and complete, 2026-08-27
**Owner:** repository Symbol Gate policy

## Decision

`lifetime_parameter` and `type_parameter` are excluded as independent Symbol
Gate items. Their owning declarations remain included, with complete generic
bounds, parameter and return types, and binder uses in the structural shape.

The closest existing concept is the function, method, trait, type alias, or
nominal type that declares the binder. A standalone `T`, `F`, `'a`, or `'de`
has no separate lifecycle or observable behavior and is not a Fava vocabulary
concept. Reviewing it separately duplicates its owner's approval without
covering another public contract.

## Counterexample and falsifier

Scanning the same current SCIP index with the old policy yields 4,652 items;
the approved policy yields 4,580. The exact delta is 72 binder nodes: 23
lifetime parameters and 49 type parameters. No struct, enum, trait, type alias,
function, method, field, variant, module, provider contract, or other nominal or
callable symbol disappears.

`symbol-gate scan` must continue to expose each owning declaration and its full
generic constraints. A policy or scanner change that removes an owner or drops
its constraints fails this decision.
