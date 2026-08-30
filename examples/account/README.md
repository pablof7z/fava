# account

A focused Fava account REPL. It creates/imports/pubkey-adds accounts, switches or
clears Fava's current account, replaces/removes signers, publishes through the
current account, and keeps one `$currentPubkey` query open while selection
changes.

## Run

```sh
cargo run --manifest-path examples/account/Cargo.toml
```

Replay ordinary commands as deterministic JSONL:

```sh
cargo run --manifest-path examples/account/Cargo.toml -- \
  --script examples/account/scenarios/account-reactivity.txt --jsonl
```

Minimal live flow:

```text
relay add primary wss://relay.example
account import alice <nsec-or-hex-secret>
query open mine $currentPubkey 1 primary
publish 1 "alice event" primary
query wait mine 1
account import bob <nsec-or-hex-secret>
query snapshot mine
publish 1 "bob event" primary
query wait mine 1
diagnostics
```

Interactive and replay input use the same parser and dispatcher. Interactive
omissions may prompt; replay omissions refuse without consuming the next line.
Private keys are ordinary bounded test data.

## Public Fava surface

The account shell delegates account state to `Fava::add_account`,
`add_signer`, `replace_signer`, `remove_account`, `select_account`, and
`clear_current_account`. Publication calls
`fava.to(relays)?.publish(EventBuilder::new(kind).content(content))`; it never
reads or passes the selected author. Query open calls
`Query::authors_current_account()` once and retains the same `Observation`
handle across switches. `diagnostics` exposes the atomic current-account/session
revision plus public query, relay, and write diagnostics.
