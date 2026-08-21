# GSD Debug Knowledge Base

Resolved debug sessions. Used by `gsd-debugger` to surface known-pattern hypotheses at the start of new investigations.

---

## m8-unreachable-delivery-retry — Unreachable delivery never retried after relay recovery
- **Date:** 2026-08-21
- **Error patterns:** Elapsed(()), receipt remains Open, Unreachable, spent attempts zero, no new connection, wait_terminal timeout
- **Root cause(s):** `WaitFor` slept then redecided an unchanged Unreachable fact forever; publication used `Receipt::spent` as the monotonic generation predecessor and for give-up identity; Redb refused `Unreachable -> Attempting`. Together these prevented a fresh exact attempt while keeping offline budget at zero.
- **Fix:** Make `WaitFor` delay one store-revalidated attempt; use `Receipt::attempts` for monotonic store generation and `Receipt::spent` only for policy facts; permit Redb to begin the next generation from an Unreachable lane; add a focused provider transition regression.
- **Files changed:** crates/fava-delivery/src/lib.rs, crates/fava-publication/src/delivery.rs, crates/fava-write-store-redb/src/ops.rs, crates/fava-write-store-redb/BUILD.bazel, crates/fava-write-store-redb/tests/delivery_lifecycle.rs, crates/fava/tests/delivery_bounds.rs
- **Why not caught:** No existing committed integration gate combined unreachable-then-reachable retry, zero offline budget spend, monotonic attempt generation, and Memory/Redb provider parity.
- **Recurrence guard:** Regression tests `crates/fava/tests/delivery_bounds.rs:offline_time_spends_no_attempt_budget_and_the_write_stays_open` and `crates/fava-write-store-redb/tests/delivery_lifecycle.rs:unreachable_generation_can_retry_without_spending_attempt_budget` both pass and kill the deliberate three-seam break.
---
