## 1. Architecture Approval and Current-Model Documentation

- [x] 1.1 Open one focused local architecture issue covering the closest existing concepts, multi-group distinction and counterexample, owners/lifecycles, forcing requirement, insufficient current state, relay limitation, and executable falsifier; verify Pablo's explicit approval is recorded before implementation begins.
- [ ] 1.2 Update the authoritative goals, architecture, implementation plan, and testing guide to the approved builder-carried route model; verify an empty search for superseded unsigned `SimpleGroup::prepare` and single-group-only publication claims outside retained history.
- [ ] 1.3 Update `docs/internals/vocabulary.toml` for every added, removed, or changed public concept and symbol; verify `python3 tools/check_vocabulary.py` and all `tools/tests/test_vocabulary_*.py` tests pass.

## 2. Failing Behavioral Evidence

- [ ] 2.1 Add `fava-write` tests proving neutral builder route accumulation, first-occurrence relay order, duplicate collapse, the 256-relay refusal, unchanged event identity across host-only changes, and event-only build refusal; verify each new test fails against the pre-change implementation.
- [ ] 2.2 Add `fava-simple-groups` public API and architecture tests proving `.simple_group(...)` returns concrete `EventBuilder`, propagates `WriteIntentError` without translation, ordinary builder chaining remains available, distinct ids append ordered exact `h` tags, repeated ids remain idempotent, and sibling tags stay scoped; verify each new test fails before implementation.
- [ ] 2.3 Add facade tests proving `fava.publish(builder)` selects automatic or embedded explicit routing, a dual-explicit expression refuses before signer/custody work, and one exact event is delivered once per unique relay; verify each new test fails before implementation.
- [ ] 2.4 Add restart and deliberate-break tests proving the accepted event and explicit route survive restart, dropping the route is detected, changing tag order changes event identity, and weakening dual-route refusal is detected; verify the named deliberate breaks fail for their intended reasons.
- [ ] 2.5 Add signed-event tests proving valid sibling `h` contexts and malformed unrelated siblings cannot erase the selected exact group, missing selected context refuses, and successful validation is byte-exact; verify the tests fail under the current duplicate-context policy.

## 3. Neutral Event-Builder Routing

- [ ] 3.1 Add neutral `WriteRouting` state to `EventBuilder`, defaulting to automatic, and implement atomic bounded explicit-relay accumulation without protocol imports; verify the focused `fava-write` route-order and builder tests pass.
- [ ] 3.2 Add the neutral consuming event-and-routing operation and typed event-only build refusal for routed builders; verify no successful public path can discard an attached route and the event id excludes route data.
- [ ] 3.3 Update `fava-write` Rustdocs, README, Cargo/Bazel source mappings, and public API inventory to the new current surface; verify `cargo test -p fava-write` and `bazel test //crates/fava-write:event_builder //crates/fava-write:routing_order` pass.

## 4. Simple-Groups Builder Composition

- [ ] 4.1 Implement the approved `SimpleGroupEventBuilder` extension trait for `EventBuilder`, staging route validation before appending the owned exact `h` tag, returning `EventBuilder`, and propagating `WriteIntentError` without translation; verify the public compile-shaped tests and unit tests pass.
- [ ] 4.2 Make repeated group composition preserve first group/tag/relay occurrence, add hosts for repeated ids, and leave foreign or malformed sibling tags untouched; verify duplicate-id, extra-host, disjoint-id, extra-cell, and malformed-sibling cases pass.
- [ ] 4.3 Change signed-event validation to require the selected exact context while tolerating sibling contexts, remove the unsigned `prepare` path and its duplicate-context restriction, and leave no alias or shim; verify an empty public-surface search for the removed unsigned path and all signed tests pass.
- [ ] 4.4 Replace simple-groups README examples, Rustdocs, Cargo/Bazel mappings, and public API inventory with the fluent multi-group expression; verify `cargo test -p fava-simple-groups` and `bazel test //crates/fava-simple-groups:unit_tests //crates/fava-simple-groups:public_api //crates/fava-simple-groups:architecture` pass.

## 5. Universal Publication Integration

- [ ] 5.1 Admit `EventBuilder` through the existing Fava publication payload path and resolve automatic, facade-explicit, builder-explicit, and dual-explicit cases exactly as designed; verify publication-door and publication-scope tests pass without adding another public publisher.
- [ ] 5.2 Lower the built unsigned event and resolved route through `WriteIntent::event` before the existing signer and custody owners; verify signer-call counters and store/publication counters remain zero on every build, bound, and conflict refusal.
- [ ] 5.3 Prove receipt and restart behavior retains the normalized explicit route without persisting `SimpleGroup` values; verify focused restart, settlement, explicit-publication, simple-groups, and multi-relay tests pass.
- [ ] 5.4 Update the Fava facade README, Rustdocs, Cargo/Bazel mappings, and public API inventory to show `fava.publish(builder)` as the grouped unsigned terminal and `fava.to(...).publish(event)` as the pre-signed explicit route; verify `bazel test //crates/fava:publication_door //crates/fava:publication_scopes //crates/fava:explicit_publication //crates/fava:simple_groups //crates/fava:multi_relay` passes.

## 6. Relay Interoperability Evidence

- [ ] 6.1 Add a controlled relay canary that publishes one signed event with two exact `h` contexts through the derived host union and queries each context independently; verify both queries return the same event id and signature with actual serving-relay evidence.
- [ ] 6.2 Add a negative relay case that accepts or serves fewer contexts than selected; verify Fava reports only observed relay/event outcomes and makes no inferred per-group success claim.
- [ ] 6.3 Record the exact relay implementation/revision, commands, event id, routes, query filters, acknowledgements, and retrieval evidence in the focused issue; verify retained evidence is bounded and rerunnable.

## 7. Final Validation

- [ ] 7.1 Run `cargo fmt --check`, focused Cargo tests for `fava-write`, `fava-simple-groups`, and `fava`, and the mapped focused Bazel targets; verify every command is green at the implementation tip.
- [ ] 7.2 Run `python3 tools/check_vocabulary.py` and all vocabulary unit tests, check changed code files against the 500/800-line limits, and verify generated inventories match the actual public surface.
- [ ] 7.3 Run the repository-required broader validation and compare failures to the exact pre-change baseline; verify no new unexplained failures, skipped acceptance gates, compatibility paths, or unrelated worktree changes remain.
