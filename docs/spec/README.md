# Rewrite specification

These documents are the authoritative inputs for the clean-room rewrite:

1. `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` defines required behavior.
2. `ARCHITECTURE.md` assigns responsibilities, state, lifecycles, and replaceable contracts.
3. `FAVA_TDD_BDD_TESTING_GUIDE.md` defines the required TDD, BDD, mutation, and evidence discipline.
4. `FAVA_REWRITE_IMPLEMENTATION_PLAN.md` defines delivery sequencing and milestone exit gates.
5. `partial-spec-api-semantics.md` refines the Rust reactive-query surface and relay-source semantics where it does not conflict with the complete authorities.

They were imported from the documents supplied by Pablo on 2026-08-20.
Implementation status belongs in focused issues, the canary registry, and
feature evidence, not in these source documents.

Architectural concepts, public Rust symbols, and crate names used by these
documents are defined where they are implemented. Symbol Gate signs the public
declarations themselves; see `.symbol-gate/`.
