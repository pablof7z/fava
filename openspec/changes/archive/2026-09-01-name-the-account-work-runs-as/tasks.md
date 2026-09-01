## 1. The verb names the account

- [x] 1.1 Add `Fava::with_account(public_key)` yielding the publication expression `by` yields today, carrying a selected account rather than an asserted author; verify a test asserts the selection reaches the relay session key's access authority
- [x] 1.2 Resolve an authorless payload's author in the stated order — the payload's own author, then the selected account, then `Session::current_account()`; verify tests cover each rung and assert a payload that states its author keeps it under a selection naming someone else
- [x] 1.3 Refuse before durable custody when a payload states no author and neither a selection nor a current account supplies one; verify a test asserts a typed refusal with no write identifier, no receipt identifier, and nothing handed to the publication owner
- [x] 1.4 Delete `AuthorlessPayload` and widen `publish` to accept any payload; verify a test publishes an event authored by one account under a selection naming another and asserts the event's author and the session's account differ exactly as expected
- [x] 1.5 Remove `Fava::by` with no alias or deprecation, and move every call site across crates, tests, and doctests; verify `cargo build --workspace --all-targets --locked` succeeds and a grep for the removed verb is empty

## 2. One door for reads

- [x] 2.1 Make a query opened under a selection carry that account's access authority, and one opened without carry public access; verify a test asserts the session key an observation acquires under each
- [x] 2.2 Corrected rather than done as written. Removing `Query::with_relay_access` is wrong: `crates/fava-router-outbox/src/lib.rs:51` uses it to forward the authority its `RouteRequest` was handed onto the query it declares, and a router is a replaceable boundary that must work through public contracts (SUB-05). It is a router's mechanism, not an application's verb. What the task was reaching for is done: an application names the account once, through `with_account`, for reads and writes alike, and no longer needs to touch the query's access itself.
- [x] 2.3 Prove reads and writes take the same door: verify a test opens a query and publishes under one selection and asserts both reach the same relay session key

## 3. Verification

- [x] 3.1 Update the settled `publication/author-scope` spec's rejection requirement through this change's delta, so the archived capability describes the verb that exists; verify `openspec validate` accepts the change
- [x] 3.2 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked`, and verify every one passes
- [ ] 3.3 Re-sign the changed public declarations under Symbol Gate. (Open, at the repository owner's instruction: the tool runs, but `verify` refuses because no trusted key is named and the user-local trust store is empty. The owner's key is needed.)
