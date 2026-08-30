## 1. Current-account architecture

- [ ] 1.1 Create one focused architecture issue mapping `ID-001`–`ID-003`, `WRITE-003`, and the partial reactive-query contract to current implementation gaps; verify exact owners and counterexamples for app-owned selection, explicit author threading, and manual query rebuilding.
- [ ] 1.2 Define the minimum public vocabulary for optional current selection, current-account publication convenience, and `$currentPubkey` query binding; obtain Pablo’s explicit approval before code or `vocabulary.toml` changes.
- [ ] 1.3 Add cross-owner BDD scenarios and deliberate breaks for account switching, accepted-author stability, empty selection, automatic observation rerooting, rapid switches, and stale completions; verify each new scenario fails against current `main` for its named reason.

## 2. Session current-account owner

- [ ] 2.1 Add owner-level tests for signer-backed and pubkey-only accounts, optional current selection, bounded account count, atomic select/clear/remove, and monotonic session revision; verify current implementation lacks the behavior.
- [ ] 2.2 Implement current-account selection in the session owner without creating a second signer map or application-owned selected key; verify removing the selected account clears it while unrelated cached events, writes, and receipts remain untouched.
- [ ] 2.3 Add exact-generation tests for signer replacement, removal, cancellation, and pending invocation completion; verify a retired signer generation cannot become current while account identity remains exact.
- [ ] 2.4 Run full session/facade tests, strict Clippy, formatting, vocabulary, and deliberate-break gates; independently review, rebase, and merge the focused session slice to `main`.

## 3. Current-account write resolution

- [ ] 3.1 Add facade/publication evidence that a current-account write resolves its author before acceptance, refuses with no current account before creating a write or receipt, and remains attributed after a later switch.
- [ ] 3.2 Implement the approved current-account publication convenience by lowering to the ordinary accepted-write path; verify no parallel author field, signer path, route path, or receipt lifecycle is introduced.
- [ ] 3.3 Prove A-accepted/switch-to-B and switch-to-B/B-accepted scenarios with delayed signing and routing; verify exact event author, signer generation, write identity, and receipt evidence.
- [ ] 3.4 Run full write/publication/facade tests, strict Clippy, formatting, vocabulary, restart where applicable, and deliberate-break gates; independently review, rebase, and merge the focused write slice to `main`.

## 4. Reactive `$currentPubkey` queries

- [ ] 4.1 Add query-domain evidence that author and tag-value filters accept the current-account reactive root, empty selection matches nothing, and the declarative query remains inspectable without a concrete app-supplied key.
- [ ] 4.2 Implement the minimum current-account reactive value binding without generalizing to query-derived `ValueSet` algebra; verify literal query behavior remains unchanged.
- [ ] 4.3 Add observation evidence that one stable handle recompiles, re-evaluates cache/write-store sources, reroutes, and updates relay subscriptions when session revision changes; verify the app never closes or reopens the observation.
- [ ] 4.4 Add delayed relay, route, subscription, and local-source completions across A→B→C switches; verify only the exact current session revision and operation generation can update the current snapshot or active demand.
- [ ] 4.5 Prove removing/clearing current selection retracts account-dependent demand and yields a match-nothing snapshot without deleting cached public events or receipts.
- [ ] 4.6 Run full query/observe/routing/subscription/facade tests, strict Clippy, formatting, vocabulary, and deliberate-break gates; independently review, rebase, and merge the focused reactive-query slice to `main`.

## 5. Focused experiential account app

- [ ] 5.1 Scaffold `examples/account` as a real `e2e-support` consumer with account create/import/add-pubkey/list/select/replace/remove/clear commands, ordinary inline key data, one parser/dispatcher, deterministic replay, typed JSONL, captures, and bounded dump.
- [ ] 5.2 Add explicit-kind test-event publication through the current-account convenience plus receipt list/show; verify app code never reads the selected key merely to pass an author to Fava.
- [ ] 5.3 Add one bounded query/observation command using `$currentPubkey`, snapshot/status commands, and account switching while the handle remains open; verify app code has no session-change listener or query reconstruction path.
- [ ] 5.4 Add routes and diagnostics views that make current selection, session revision, compiled query generation, active relay demand, accepted author, and signer generation attributable without exposing private internals.
- [ ] 5.5 Compare account and simple-groups presentation code and extract only identical Reedline assembly, rendering/theme, prompt layout, bounded history, completion, and hints into `e2e-support`; verify simple-groups PTY/golden and non-TTY bytes remain unchanged.
- [ ] 5.6 Implement account-specific contextual prompt, completion, hints, narrow-terminal rendering, `NO_COLOR`, and actual-binary PTY tests without a plugin framework.
- [ ] 5.7 Audit the complete app for explicit author threading, selection propagation, query rebuilding, observation reopening, subscription mutation, route recomputation, and stale-generation filtering; fix each Fava-owned DX gap or keep the affected workflow incomplete with an executable falsifier.

## 6. Deterministic and live proof

- [ ] 6.1 Create one ordinary-command scenario that imports A and B, publishes through A, opens one `$currentPubkey` observation, switches to B, publishes through B, clears selection, switches rapidly, captures results, and dumps typed evidence.
- [ ] 6.2 Add black-box replay tests proving missing values do not consume later commands, private keys remain ordinary data, accepted writes retain authors, the observation handle stays stable, and stale generations never become current.
- [ ] 6.3 Build a bounded ordinary-relay harness from proven shared mechanics; independently require exact A/B event authors and matching `EOSE` while app evidence proves the reactive observation transitions without harness-created events.
- [ ] 6.4 Run delayed-completion controls that would fail if A/B query, route, subscription, local-source, or signer completions overwrite C; retain bounded canonical evidence with fixture versions and hashes.

## 7. Documentation and main integration

- [ ] 7.1 Write a consumer-facing README showing the minimal account workflow and the exact public calls for current-account writes and `$currentPubkey` queries; document any unresolved DX gap honestly.
- [ ] 7.2 Capture actual-binary terminal session and completion screenshots and verify they correspond to the committed UI.
- [ ] 7.3 Run full account, shared-support, simple-groups regression, session, write, query, observation, routing, subscription, live harness, strict Clippy, formatting, vocabulary, restart, and diff gates.
- [ ] 7.4 Obtain independent DX, architecture, code, concurrency, harness, and evidence review; repair every concrete blocker and rerun its falsifier.
- [ ] 7.5 Rebase reviewed slices onto current `main`, resolve conflicts to current public truth, run integrated validation, and fast-forward `main`; verify the main worktree is clean and contains the current-account owner, write convenience, reactive query support, and experiential app.
