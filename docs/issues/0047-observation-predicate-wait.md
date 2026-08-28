# 0047 — Bounded observation predicate wait

**Status:** implemented; pending parent amend
**Owner:** `fava-observe` owns the installed observation handle and its delivery lifecycle

## Decision

`Observation::wait_until` is the one bounded predicate wait for an already
installed observation:

```rust
async fn wait_until(
    &mut self,
    timeout: Duration,
    predicate: impl FnMut(&QuerySnapshot) -> bool,
) -> Result<Option<Arc<QuerySnapshot>>, ObservationClosed>;
```

`Some(snapshot)` is the first observed snapshot that matches. `None` is expiry
of the whole caller-supplied timeout. `Err(ObservationClosed)` preserves the
existing observation closure result; no second wait error or observation
lifecycle exists.

## Lifecycle and boundedness

The call tests the installed `current()` snapshot before awaiting. It then uses
the snapshot returned by `changed()` for every later predicate test, so it does
not clone `current()` after delivery. `FnMut` may run once for the initial
snapshot and once for each later snapshot the call observes.

One timeout bounds the entire call, not individual delivery attempts. Timeout
and caller cancellation leave the observation, its demand, and later delivery
open for the same handle. A completed `changed()` alone advances delivery;
cancelled waiting cannot claim a stale completion or affect another wait.

## Falsifier

`bounded_predicate_wait_preserves_timeout_closure_and_later_delivery` proves
an initial synchronous match under a zero duration, one failed initial
predicate call, `Ok(None)` timeout, delivery through the same handle after that
timeout, and `Err(ObservationClosed)` after explicit closure. Removing the
initial test, consuming or closing the handle at timeout, mapping closure to
`None`, or retesting the unchanged initial snapshot makes this evidence fail.

## Validation

- `cargo fmt --check`
- `cargo test -p fava-observe --all-targets`
- `cargo test -p fava --all-targets -- --skip vocabulary_gate_requires_all_terms_approved --skip vocabulary_terminal_names_match_term_names`
- `cargo check` in `apps/canary` and `examples/simple-groups`
- `python3 tools/crate_readme_api.py check fava-observe fava`
- `python3 -m unittest tools.tests.test_vocabulary_structure`

The repository-wide vocabulary gate and strict Clippy remain inherited-red on
unrelated existing findings.
