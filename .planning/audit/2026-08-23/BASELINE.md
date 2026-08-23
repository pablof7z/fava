# Pre-remediation test baseline — 2026-08-23

`cargo test --workspace --no-fail-fast` at `b221203` + the three RED falsifiers.

- 118 test targets; 117 green, 1 red.
- 306 tests passing, 3 failing.

The 3 failures are exactly the falsifiers added by the architecture debug
session in `crates/fava/tests/explicit_live.rs`:

- `relay_establishment_does_not_delay_the_coherent_local_observation`
- `equivalent_observations_share_relay_work_until_the_last_handle_closes`
- `cancelling_observe_while_another_relay_opens_closes_provisional_work`

**Read this number correctly.** 306 green tests coexisting with a confirmed
systemic ownership inversion is not reassurance; it is the finding. The corpus
cannot distinguish the implemented architecture from the specified one. Any
remediation must be judged by new falsifiers, not by keeping this number green.
