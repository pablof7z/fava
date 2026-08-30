## Why

Fava needs one narrow experiential application that proves account selection is a real reactive runtime input, not application plumbing. The app’s primary purpose is to expose poor DX when writes, queries, observations, routing, or signer generations fail to follow account switches automatically and exactly.

## What Changes

- Add one focused account REPL for creating, importing, listing, selecting, replacing, and removing local accounts.
- Publish ordinary test events through the selected account and prove each accepted write permanently resolves the correct author even if the account switches later.
- Open declarative queries whose author filter uses the reactive `$currentPubkey` input and prove the same observation automatically recompiles, reroutes, and updates when the selected account changes.
- Prove no-current-account, account removal, signer replacement, rapid switching, cancellation, and late-completion behavior with exact account and generation attribution.
- Use one interactive/replay grammar, ordinary inline private-key test data, typed JSONL, captures, receipts, routes, diagnostics, and bounded independent relay proof.
- Treat any requirement for app-owned current-account state propagation, manual query rebuilding, observation reopening, subscription management, explicit author threading, or stale-completion filtering as a blocking Fava DX defect.
- Exclude profile metadata, contacts, relay lists, NIP-05/NIP-11, bookmarks, and every other account-owned protocol surface.

## Capabilities

### New Capabilities
- `experiential/account-repl`: Account lifecycle, selected-author publication, reactive `$currentPubkey` query behavior, generation isolation, deterministic replay, and live proof.

### Modified Capabilities

None.

## Impact

- Adds a small `examples/account` experiential app and only the shared shell/presentation extraction forced by a second consumer.
- Exercises and, where missing, completes the public current-account reactive input promised by `ID-002`, current-account write resolution promised by `WRITE-003`, signer attachment generations, query observation, routing, publication receipts, and diagnostics.
- Requires focused public API/vocabulary changes only if the existing facade cannot expose current-account selection and `$currentPubkey` declaratively.
- Uses ordinary relays and independent `REQ`/matching `EOSE` inspection to prove authorship and reactive query transitions.
