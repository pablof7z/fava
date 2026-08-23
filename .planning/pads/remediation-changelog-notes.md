
- (2026-08-23 16:03) Changelog pad opened. 13 branches merged, 306->431 tests. Loop scheduled every 30m (cron 98413fcf) to keep it current.

- (2026-08-23 16:39) 15 branches merged, 476 tests passing. GSD workflow bypass recorded; awaiting Pablo on backfill vs freeze.

- (2026-08-23 16:39) Vocabulary finding: 159/280 spec type names absent from vocabulary.toml. Spec is not approval.

- (2026-08-23 17:09) 07.6 merged: crisis closed, 539 tests, 3 falsifiers green. Caught 2 merge regressions (ProviderClosed refabricated, per-filter ingest).

- (2026-08-23 17:09) Pablo ruled D5: delete ObserveError::Relay. Going through gsd-plan-phase 07.7.

- (2026-08-23 17:09) ARCHITECTURE.md:1299-1327 (local_queries/explicit_queries) is invented scaffolding over two enum values. Recommend deletion.

- (2026-08-23 17:39) Router trait: one method, open(query), refusing non-Explicit/non-CacheOnly. Reason is capability (no publish/close from a router) and the Arc cycle, not layering.

- (2026-08-23 17:39) Correction: justified the narrow trait by citing an audit finding. Wrong basis. Pablo called it.

- (2026-08-23 18:09) 07.6 verification: gaps_found, 4/7. QUERY-010 fails on release-then-reopen, proven by probe. Fixer dispatched.

- (2026-08-23 18:09) Decided: no backfilled PLAN/SUMMARY for 07.3-07.6. Markers written saying no verdict may cite them. 07.9 verifies against the corpus.

- (2026-08-23 18:09) Correction: claimed CI closed the drift gap. It does not. Green tests passed on the broken architecture for six milestones. Verifier gate is what catches drift.
