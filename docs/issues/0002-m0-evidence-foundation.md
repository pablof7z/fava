# M0 evidence foundation and real-relay lab

**Status:** completed
**Milestone:** M0
**Branch:** `rewrite/0002-m0-evidence-foundation`

## Why

Every later networking, durability, and interoperability claim needs an
independent witness. Fava must not prove its own wire effects or relay
persistence through internal state.

## Behavior

- M0-LAB-001: a disposable, genuinely signed event is published and queried
  through real WebSocket frames against a third-party relay process.
- M0-LAB-002: the event remains queryable after that relay is hard-killed and
  restarted against the same data directory.
- M0-LAB-003: the canary preserves reconstructable process, wire, manifest,
  report, and JSONL evidence under one deterministic run identity.
- M0-LAB-004: relay startup or scenario failure fails the run; enabled
  scenarios never silently skip.

## Owner

The ordinary downstream `apps/canary` application owns orchestration and
evidence assembly. A transparent proxy owns the independent wire transcript.
The third-party relay owns persistence.

## First failing proof

Run `lab-real-relay-smoke` before its scenario implementation exists. The
registry must refuse to claim the enabled scenario passed.

## Mutation

Restart the relay with a fresh data directory. The post-restart query must not
find the event and the persistence assertion must fail.

## Independent witnesses

- transparent WebSocket frame transcript;
- third-party relay stdout/stderr and process identity;
- child-process kill and restart facts;
- application-visible scenario result.

## Exit gates

See M0 in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.

## Evidence

Red:

```sh
cargo test --manifest-path apps/canary/Cargo.toml \
  tests::enabled_real_relay_scenario_has_an_executor -- --exact
```

The test failed because the enabled scenario had no executor. This red run
preceded the layout correction; the uncommitted scaffold was then moved to
`apps/canary` before implementation.

Green:

```sh
cargo test --manifest-path apps/canary/Cargo.toml
cargo clippy --manifest-path apps/canary/Cargo.toml --all-targets -- -D warnings
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run lab-real-relay-smoke \
  --relay-bin /Users/pablofernandez/.cargo/bin/nostr-rs-relay \
  --seed m0-green-restored-20260820-a
```

The live scenario passed against `nostr-rs-relay 0.8.12`. Event
`932596e0026664870f28261c03163eefa34d0cb0a974cd8d43608bcc3dbc87ac`
was acknowledged, queried with EOSE, then queried with EOSE again after a
hard kill and same-directory restart. The run preserved manifest, report,
JSONL evidence, proxy frames, process facts, relay logs/config/database,
resource samples, and artifact hashes.

Mutation:

Generation two was temporarily changed to a fresh data directory. The same
command failed causally with:

```text
post-restart exact query was incomplete: event=false, eose=true
```

The mutation was removed and the live green scenario was rerun.

The canary is a separate workspace under `apps/canary`, has no Fava crate
dependency, and includes bounded read-only public-relay reconnaissance that
requires an explicit relay URL. Public-relay reconnaissance was not run; it is
evidence-only and not an M0 deterministic pass gate.
