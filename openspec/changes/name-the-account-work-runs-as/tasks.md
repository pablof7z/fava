## 1. The verb names the account

- [ ] 1.1 Add `Fava::with_account(public_key)` yielding the publication expression `by` yields today, carrying a selected account rather than an asserted author; verify a test asserts the selection reaches the relay session key's access authority
- [ ] 1.2 Resolve an authorless payload's author in the stated order — the payload's own author, then the selected account, then `Session::current_account()`; verify tests cover each rung and assert a payload that states its author keeps it under a selection naming someone else
- [ ] 1.3 Refuse before durable custody when a payload states no author and neither a selection nor a current account supplies one; verify a test asserts a typed refusal with no write identifier, no receipt identifier, and nothing handed to the publication owner
- [ ] 1.4 Delete `AuthorlessPayload` and widen `publish` to accept any payload; verify a test publishes an event authored by one account under a selection naming another and asserts the event's author and the session's account differ exactly as expected
- [ ] 1.5 Remove `Fava::by` with no alias or deprecation, and move every call site across crates, tests, and doctests; verify `cargo build --workspace --all-targets --locked` succeeds and a grep for the removed verb is empty

## 2. One door for reads

- [ ] 2.1 Make a query opened under a selection carry that account's access authority, and one opened without carry public access; verify a test asserts the session key an observation acquires under each
- [ ] 2.2 Remove `Query::with_relay_access` from the public surface and move every caller to the selection; verify a grep finds no remaining public use and the observe tests pass
- [ ] 2.3 Prove reads and writes take the same door: verify a test opens a query and publishes under one selection and asserts both reach the same relay session key

## 3. Verification

- [ ] 3.1 Update the settled `publication/author-scope` spec's rejection requirement through this change's delta, so the archived capability describes the verb that exists; verify `openspec validate` accepts the change
- [ ] 3.2 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked`, and verify every one passes
- [ ] 3.3 Re-sign the changed public declarations under Symbol Gate. (Expected to stay open until the repository owner supplies a trusted key: the tool runs, but `verify` refuses because none is named and the user-local trust store is empty.)
