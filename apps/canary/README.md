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

The four M7 semantic-write canaries are deterministic, memory-backed public
Fava executions. They do not start a relay or use timing sleeps:

```sh
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run replaceable-edit-first-value --seed <unique-seed>
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run replaceable-edit-rematerialization --seed <unique-seed>
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run replaceable-edit-inverse --seed <unique-seed>
cargo run --manifest-path apps/canary/Cargo.toml -- \
  run protocol-crate-n-plus-one --seed <unique-seed>
```

Each run writes `semantic.json`, a bounded event log, a report, and a manifest
with artifact hashes. First-value evidence includes the stable write/receipt,
materialization, exact route, publication attempt, public-query result, and
cache absence. Rematerialization evidence includes the qualified source,
current and retired materializations, preserved fields, and one publication.
Inverse evidence includes both final events, all ten stable receipt IDs, empty
target counts, adjacent empty operations, and exact publication count. N+1
evidence records the independent public-only capability, raw future-kind proof,
and absence from the product dependency graph. Any missing proof exits nonzero.
