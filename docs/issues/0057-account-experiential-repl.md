# 0057: focused account experiential repl

## decision

`examples/account` is the ordinary downstream consumer for bounded account
lifecycle, selected-author publication, and reactive `$currentPubkey` queries.
It uses only public Fava APIs. Application aliases are input presentation; Fava
remains the selected-public-key authority.

## behavior

The shell supports account create, secret import, pubkey-only add, list, switch,
signer replace, remove, and clear. It publishes explicit event kinds through the
current account, lists receipts, and retains named query observations. One
`query open <name> $currentPubkey <kind> <relay>...` call follows subsequent
selection changes without application reopening, rerouting, or stale-result
filtering.

Interactive and replay modes use one grammar and dispatcher. Interactive
omissions prompt without entering history. Replay omissions return one typed
refusal without consuming the next command. Noninteractive output is bounded,
deterministic JSONL; private keys and relay URLs are ordinary bounded input.

The Reedline terminal shows compact account, relay, and query context, bounded
history, completion, hints, value prompts, typed human results, narrow-width
elision, and `NO_COLOR`/`--no-color` behavior. Actual binary captures are in
`docs/issues/0057/terminal-session.png` and
`docs/issues/0057/terminal-completion.png`.

## public surface exercised

- `Fava::{add_account,accounts,select_account,clear_current_account,remove_account}`
- `Fava::{add_signer,replace_signer,remove_signer,signer_status,current_account}`
- `Fava::to(...).publish(EventBuilder)` without an explicit author
- `Fava::{open_receipts,receipt,diagnostics}`
- `Query::authors_current_account` and one stable `Observation`
- `Observation::{id,current,changed,wait_until,synchronize_current_account,close}`

## evidence

`examples/account/tests/repl.rs` executes the real binary for account lifecycle,
signer-generation attribution, active route/demand ownership, missing-value
refusal, stable observation identity, terminal rendering, color, and
`$currentPubkey` completion. Shared-support and simple-groups regressions
prove extraction did not fork their grammar or bytes.

`examples/account/live/harness.py` starts a disposable `nostr-rs-relay`, runs the
ordinary scenario lines, and independently reads Alice and Bob events by exact
id through direct `REQ` and matching `EOSE`. The retained run proves one public
observation id across Alice → empty Bob → Bob → Alice → Bob → cleared. The
harness never constructs, signs, routes, or publishes the tested events.
Canonical bounded evidence and SHA-256 artifact hashes are in
`examples/account/live/evidence/2026-08-30-account-reactivity/`.

Core owner race tests deliberately block source opening, selection activation,
owner-side synchronization, diagnostic commit, and close. Mutations prove selection locking,
selection-specific revision, provisional-demand suppression, and atomic
diagnostic publication are necessary.

## remaining gap

Symbol Gate's repository policy is unsigned and reports repository-wide unsigned
surface, including pre-existing declarations. Owner signature approval remains required before
claiming the repository's public API approval gate complete; it does not change
the app's runtime evidence.
