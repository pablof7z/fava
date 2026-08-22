# Fava end-to-end canary

An ordinary downstream Rust application and independent evidence lab. It must
not depend on Fava internal crates or use Fava diagnostics as the sole witness
for external effects.

The first enabled scenario is `lab-real-relay-smoke`, using the pinned
`nostr-rs-relay` 0.8.12 binary as a real third-party process on macOS. Install
it with:

```sh
cargo install nostr-rs-relay --version 0.8.12 --locked
```

Scenario status is recorded in `scenarios.json`. Enabled scenarios fail on an
unavailable prerequisite; they never silently skip.

Run the deterministic local scenario:

```sh
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run lab-real-relay-smoke --seed <unique-seed>
```

Run bounded read-only public-relay reconnaissance only with an explicit URL:

```sh
cargo run --manifest-path apps/canary/Cargo.toml -- \
  recon --relay wss://relay.example --seed <unique-seed>
```

Evidence is preserved under `apps/canary/runs/` and excluded from Git.

## Controlled Croissant NIP-02 proof

`croissant-nip02-public-flow` starts the exact Croissant executable on a fresh
loopback port and data path. It publishes a kind-9007 group create and then the
README NIP-02 baseline/edit flow through the same public `Fava::to(...).publish`
lifecycle. The retained manifest correlates local observation before signing,
the exact relay echo, typed lossless decode, write/receipt/materialization/event
identities, executable SHA-256, Croissant source HEAD, declared bounds, and
completed PID/port teardown.

Run it twice beneath one fresh pair root, then verify the pair:

```sh
pair_root="$(mktemp -d apps/canary/runs/phase-07.1-pair.XXXXXX)"
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run croissant-nip02-public-flow \
  --relay-bin /Users/pablofernandez/Work/croissant/croissant \
  --seed "$first_private_seed" --runs-dir "$pair_root"
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run croissant-nip02-public-flow \
  --relay-bin /Users/pablofernandez/Work/croissant/croissant \
  --seed "$second_private_seed" --runs-dir "$pair_root"
cargo run --manifest-path apps/canary/Cargo.toml -- \
  verify-croissant-pair --runs-dir "$pair_root"
```

Seeds are process-memory inputs. Never place literal seeds in shell history,
reports, or retained files. The scenario scans every pre-manifest artifact for
the raw input and retains only its SHA-256 plus public coordinates. The pair
verifier requires exactly two manifests, distinct seed/group/event/write/receipt
identities, no cross-run group data, complete artifact hashes and bounds, exact
foreign kind-3 tags/content, and closed child processes and ports.

The four M7 semantic-write canaries are deterministic, memory-backed public
Fava executions. They do not start a relay or use timing sleeps:

```sh
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run replaceable-edit-first-value --seed <unique-seed>
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run replaceable-edit-rematerialization --seed <unique-seed>
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run replaceable-edit-opposing-operations --seed <unique-seed>
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run protocol-crate-n-plus-one --seed <unique-seed>
```

Each successful run writes `semantic.json`, a bounded event log, a report, and
a manifest with artifact hashes. Every publication record correlates its exact
write, receipt, materialization, event, engine-owned timestamp, relay session,
and attempt number. Semantic generations assert exact timestamp agreement with
their accepted materialization and strict monotonicity across rematerialization.
Rematerialization starts from sources that both lack the followed target, adds
one unrelated source participant, and proves the final event contains each
exactly once. It holds a real generation-one delivery, installs generation two,
releases the retired completion, and accepts the generation-two attempt only as
the causal processing acknowledgement. The exact expected receipt transition
proves the retired outcome cannot contaminate current event, route, attempt, or
delivery evidence. Inverse evidence includes both final events and all ten
correlated attempts. N+1 evidence records canonical-package normal-edge
Cargo reachability, Bazel product reachability, owned-child reaping, and the raw
future event's exact caller-owned `created_at = 42`, tags, content, and identity.
A failed run retains bounded `failure.json`, `replay.json`, report, event log,
and hashed manifest evidence; the replay record names the working directory,
redacts the caller seed while retaining its hash, and selects a fresh output
directory. Any missing proof exits nonzero.
