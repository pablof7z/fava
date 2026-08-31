## 1. Rename the owned identity contract

- [x] 1.1 Replace `OperationGeneration`, `OperationGenerationIssuer`, and `OperationGenerationExhausted` with `WorkEpoch`, `WorkEpochIssuer`, and `WorkEpochExhausted` in `fava-query::identity`, preserving the opaque authority/sequence representation, checked allocation, and exact error cases; verify `cargo test -p fava-query` passes.
- [x] 1.2 Rename the identity doctests and query-evidence carriers from `generation` to `epoch`, including the no-construction/no-default compile-fail proof and the superseded-epoch comparison evidence; verify `cargo test -p fava-query --doc` and `cargo test -p fava-query --test query_evidence` pass, then temporarily reintroduce `Default` or an always-current comparison and observe the named proof fail before restoring it.

## 2. Propagate the epoch through its consumers

- [x] 2.1 Rename the `fava-runtime` re-export, module, `ProviderCompletion` field/accessor, call boundary, tests, and diagnostics from generation to epoch without changing deadline, panic, cancellation, or refusal behavior; verify `cargo test -p fava-runtime --test provider_isolation` passes and the stale-completion test observes `epoch()` on both superseded and current completions.
- [x] 2.2 Rename the `fava-observe` issuer, slot, reports, operation helpers, completion checks, evidence publication, and exact-current guards to `WorkEpoch`/`epoch`; verify `cargo test -p fava-observe --test access_work_isolation` and `cargo test -p fava-observe --test shared_work` pass, then temporarily defeat the exact epoch comparison and observe stale relay work reach the named falsifier before restoring it.
- [x] 2.3 Rename query evidence, diagnostics, subscription and facade re-exports, integration fixtures, and user-visible error text so work epochs and `RelaySessionGeneration` are unambiguously distinct; verify `cargo test -p fava-diagnostics --test ownership_graph`, `cargo test -p fava-query --test query_evidence`, and the public `fava` crate tests that import the re-export pass.

## 3. Update vocabulary records and review the public break

- [x] 3.1 Update the query-work ownership row in `docs/spec/ARCHITECTURE.md` and current focused issue records `0028`, `0039`, `0040`, and `0054` to use `WorkEpoch`/`epoch`, preserving the owner and lifecycle distinctions without retaining an old-name alias or migration narrative; verify a scoped `rg` finds no old public spelling in live source, active specifications, or those issue records.
- [ ] 3.2 Re-scan the changed public declaration surface and obtain fresh Symbol Gate review/approval for `WorkEpoch`, `WorkEpochIssuer`, and `WorkEpochExhausted` under the existing observation/query-work vocabulary; verify `symbol-gate status` and `symbol-gate verify` report the replacement surface approved and no former declaration remains signed.

## 4. Validate the focused breaking rename

- [x] 4.1 Run `cargo fmt --check`, strict Clippy for `fava-query`, `fava-runtime`, `fava-observe`, `fava-diagnostics`, `fava-subscriptions`, and `fava`, and `cargo check --workspace --all-targets`; verify all pass with no compatibility aliases or behavior changes.
- [ ] 4.2 Run `bazel test //crates/fava-query:query_evidence //crates/fava-runtime:provider_isolation //crates/fava-diagnostics:ownership_graph //crates/fava-observe:access_work_isolation` and the corresponding Cargo focused tests; verify the Bazel and Cargo paths both preserve independent authorities, exhaustion refusal, and stale-work isolation.
- [x] 4.3 Perform a final repository search excluding approved historical evidence and this change's planning artifacts; verify every live declaration, import, re-export, field, accessor, test, diagnostic, and current documentation reference uses `WorkEpoch`/`epoch`, while `RelaySessionGeneration` retains its transport-owned `generation` vocabulary.
