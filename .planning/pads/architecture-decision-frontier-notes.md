
- (2026-08-23 15:40) Session opened. 6 open decisions; D1 (planner signature) and D2 (admission window) on hold pending agent input.

- (2026-08-23 15:40) Pablo corrected the grouping model: sent subscriptions are never rewritten; grouping batches unsent demand only, behind a relay-level debounce. Source: nmp.

- (2026-08-23 15:40) nmp measured the rewrite model we shipped: 0.6% waste at 1 growth step, 90% and 1->20 concurrent subs at 20. Quadratic. .planning/audit/2026-08-23/nmp-filter-merge-comparison.md

- (2026-08-23 15:40) Content-digest wire ids violate GOALS.md:426 (QUERY-010) fresh-identity-on-reopen. Passed 45 falsifiers + clippy + vocab gate. nmp removed the same design in its #774.

- (2026-08-23 15:40) Correction (16:00): I told the query agent FROZEN-CONTRACTS governed ObserveError::Relay. It does not. Agent checked and refuted.

- (2026-08-23 15:40) 3 subagents dispatched: router acquisition boundary; WRITE-027 + ObserveError shape; adjacent sweep on Bazel/out-of-workspace corpora/cross-owner seams.

- (2026-08-23 15:41) Pad seeded: goal, constraints, model, decided, positions. 4 open annotations on positions.
