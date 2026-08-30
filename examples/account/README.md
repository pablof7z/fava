# account

A focused Fava account REPL. It creates/imports/pubkey-adds accounts, switches or
clears Fava's current account, replaces/removes signers, publishes through the
current account, and keeps one `$currentPubkey` query open while selection
changes.

## Run

```sh
cargo run --manifest-path examples/account/Cargo.toml
```

Replay ordinary commands as deterministic JSONL. The complete scenario expects
an ordinary relay at `ws://127.0.0.1:18080`:

```sh
cargo run --manifest-path examples/account/Cargo.toml -- \
  --script examples/account/scenarios/account-reactivity.txt --jsonl
```

Run the same scenario against a disposable relay with independent `REQ`/`EOSE`
verification:

```sh
cargo build --manifest-path examples/account/Cargo.toml
python3 examples/account/live/harness.py \
  --app examples/account/target/debug/account \
  --relay ~/.cargo/bin/nostr-rs-relay
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
`Query::authors_current_account()` once, retains the same `Observation` handle
across switches, and uses `Observation::synchronize_current_account()` for the
`query sync` generation barrier. `diagnostics` exposes current selection,
session/selection revisions, exact signer attachment generations, and public
query/relay/write ownership. `routes` exposes active route, logical demand, and
wire subscription attribution. No app-owned
selection listener, author threading, query rebuilding, route recomputation, or
stale-generation filtering exists.

Actual terminal captures are
[`terminal-session.png`](../../docs/issues/0057/terminal-session.png) and
[`terminal-completion.png`](../../docs/issues/0057/terminal-completion.png).
