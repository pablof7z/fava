# account live proof

The harness starts a disposable `nostr-rs-relay`, runs ordinary account REPL
lines through the real binary, and independently reads both published events by
exact id through direct `REQ` and matching `EOSE`.

```sh
cargo build --manifest-path examples/account/Cargo.toml
python3 examples/account/live/harness.py \
  --app examples/account/target/debug/account \
  --relay ~/.cargo/bin/nostr-rs-relay
```

The harness does not construct, sign, route, or publish action events. It
asserts exact Alice/Bob authorship, one stable observation id, public route and
demand attribution, the transition sequence Alice → empty Bob → Bob → Alice →
Bob → empty selection, route withdrawal after clear, and bounded process
teardown.

Canonical evidence is in
`evidence/2026-08-30-account-reactivity/`; `manifest.json` records every retained
artifact's byte count and SHA-256 digest.
