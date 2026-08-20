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
