# Fava end-to-end canary

An ordinary downstream Rust application and independent evidence lab.

## The one rule

**The canary may not work around Fava.**

If a flow is awkward, impossible, requires reaching past the public API,
requires constructing a second engine, requires a stub provider, requires
hand-feeding data the library should have fetched, or requires knowledge no
outside developer could have, the canary records it as a wall and fails. It
does not route around it. A workaround here is a suppressed bug report, and
suppressed bug reports are what users hit instead.

Consequences of the rule:

- The canary depends on the `fava` facade and on provider crates an
  application selects. Every remaining internal-crate dependency is annotated
  in `Cargo.toml` with the public-API hole that forces it.
- No stub transport, publisher, write store, or event cache. If a scenario can
  only be made deterministic by replacing the thing under test, the scenario is
  blocked instead.
- No value is written into a retained manifest unless the run measured it.
- A scenario that cannot be expressed through the public API is registered with
  `"status": "blocked"` in `scenarios.json`, its executor is deleted, and
  `canary run <id>` prints the wall and exits nonzero.

`apps/canary/src/blocked.rs` is the ledger: for every blocked scenario it names
the workaround that was removed and the wall that workaround was hiding.

## Consumer flows

`dx-flows` is the primary scenario. It drives ten things a real Nostr client
must do, through the public facade only, against a real relay:

```sh
cargo run --quiet --manifest-path apps/canary/Cargo.toml -- \
  run dx-flows --relay-url ws://127.0.0.1:7447 --seed <unique-seed>
```

`--relay-url` must name a running relay. Any relay implementation works; the
audit of 2026-08-23 used `nostr-rs-relay` 0.10.0 in Docker:

```sh
docker run -d --name fava-relay -p 7447:8080 scsibug/nostr-rs-relay:latest
```

The run writes `flows.json` with one record per flow: intent, status, severity,
the conclusion an outside developer would draw, and the measured detail. It
exits nonzero while any flow is a wall. Findings are written up in
`.planning/audit/2026-08-23/dx-walls.md`.

## Other scenarios

`canary list` prints every scenario with its milestone and status. Enabled
scenarios that start their own relay process need the pinned binary:

```sh
cargo install nostr-rs-relay --version 0.8.12 --locked
```

That pin does not currently build on Rust 1.90 (`time` fails with E0282), so
those scenarios could not be executed during the 2026-08-23 audit. Enabled
scenarios fail on an unavailable prerequisite; they never silently skip.

Bounded read-only public-relay reconnaissance needs an explicit URL:

```sh
cargo run --quiet --manifest-path apps/canary/Cargo.toml -- \
  recon --relay wss://relay.example --seed <unique-seed>
```

Evidence is preserved under `apps/canary/runs/` and excluded from Git.

## Controlled Croissant NIP-02 proof

`croissant-nip02-public-flow` starts the exact Croissant executable on a fresh
loopback port and data path. It publishes a kind-9007 group create and then the
NIP-02 baseline/edit flow through the public `Fava::to(...).publish` lifecycle.
The retained manifest correlates local observation before signing, the exact
relay echo, typed lossless decode, write/receipt/revision/event
identities, executable SHA-256, Croissant source HEAD, declared bounds, and
completed PID/port teardown.

Run it twice beneath one fresh pair root, then verify the pair:

```sh
pair_root="$(mktemp -d apps/canary/runs/phase-07.1-pair.XXXXXX)"
cargo run --quiet --manifest-path apps/canary/Cargo.toml -- \
  run croissant-nip02-public-flow \
  --relay-bin /path/to/croissant \
  --seed "$first_private_seed" --runs-dir "$pair_root"
cargo run --quiet --manifest-path apps/canary/Cargo.toml -- \
  run croissant-nip02-public-flow \
  --relay-bin /path/to/croissant \
  --seed "$second_private_seed" --runs-dir "$pair_root"
cargo run --quiet --manifest-path apps/canary/Cargo.toml -- \
  verify-croissant-pair --runs-dir "$pair_root"
```

Seeds are process-memory inputs. Never place literal seeds in shell history,
reports, or retained files. The scenario scans every pre-manifest artifact for
the raw input and retains only its SHA-256 plus public coordinates.
