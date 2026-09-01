## 1. Carry the authority

- [x] 1.1 Add the accepted `RelayAccess` to `WriteIntent` and `Receipt` as one value beside the existing author, not a second public key; verify a test asserts both are recorded, that neither is derived from the other, and that the author is unchanged
- [x] 1.2 Reshape `RouteRequest::Write` to carry the authority and make `RouteRequest::access()` return it rather than `RelayAccess::Public`; verify a test asserts an automatically routed write accepted under an account selects destinations under that account's authority
- [x] 1.3 Route a write accepted with no selection as public, unchanged; verify the existing automatic-publication tests pass with no assertion text changed
- [x] 1.4 Update every `RouteRequest::Write` construction and destructuring site in `fava-publication`, the router crates, and their tests; verify `cargo build --workspace --all-targets --locked` succeeds
- [ ] 1.5 Prove a selection change, signer replacement, or account removal after acceptance does not retarget accepted work; verify a test asserts the author and the authority are both unchanged after each

## 2. Persist it

- [x] 2.1 Bump `SCHEMA_VERSION` from 4 to 5 in `crates/fava-write-store-redb/src/schema.rs`; verify `redb_schema_mismatch_refuses_without_fallback` still refuses a store stamped with a different version
- [ ] 2.2 Refuse reconstruction of a stored row whose access authority is absent or malformed, rather than defaulting it to public; verify tests cover each and fail when the check is removed
- [ ] 2.3 Refuse reconstruction of a stored row whose authority contradicts its routed destinations; verify a test tampers with one and asserts reconstruction refuses rather than choosing either
- [ ] 2.4 Extend the four row-mutation recovery tests with a tampered access field; verify each new case fails when its check is removed
- [x] 2.5 Rename those four tests to drop the `schema_v4_` prefix, which reads as though they test an earlier schema version when they mutate rows in the current one; verify the renamed tests still run and pass

## 3. Prove it end to end

- [ ] 3.1 Prove a write parked awaiting a signer resumes after a real process restart under its accepted authority and not under public access; verify with a process-kill test in `fava-write-store-redb`
- [ ] 3.2 Prove a store reopened by a process with no account selected reads the authority from the store rather than defaulting it; verify a test asserts the reconstructed write's authority
- [ ] 3.3 Prove an authenticated publication completes end to end through one assembled `Fava` against a relay that demands `AUTH` for writes; verify through the public API, not direct provider calls

## 4. Verification

- [ ] 4.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked`, and verify every one passes
- [ ] 4.2 Re-sign the changed public declarations under Symbol Gate. (Expected to stay open until the repository owner supplies a trusted key.)
