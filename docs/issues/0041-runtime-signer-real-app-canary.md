# 0041 — Prove runtime signer wakeup through a real downstream app

**Status:** open; salvaged design boundary from a rejected stale worktree
**Owner:** `apps/canary` for real-app evidence; existing signer/session owners remain unchanged

## Current truth

`Fava::add_signer` exists. `crates/fava/tests/runtime_signers.rs` proves that an
unsigned write accepted before login keeps the same `WriteId` and `ReceiptId`,
then signs and publishes after the matching signer attaches. The canary must
not report this API as absent.

What remains unproved is the real application path against a relay and the
causal counterexample showing that signer attachment, rather than unrelated
progress, woke the parked write.

## Focused slice

Add one downstream executable beside the existing canary roster. It must:

1. start Fava without an account;
2. accept one unsigned note and retain its exact write and receipt identities;
3. prove no relay publication occurred before the signer existed;
4. attach the matching signer at runtime;
5. require `all_acknowledged()` for that same write and observe the exact event
   through Fava from the real relay;
6. run a deliberate build with signer wakeup removed and require that run to
   fail;
7. retain bounded source, build, process, and wire evidence and tear down every
   owned process.

Do not replace the multi-flow `dx-flows` report. Do not import the stale
worktree's Croissant build system, CLI rewrite, or partial diagnostic
classifier. OPS-003 classification remains a separate complete slice covering
unsignable, unresolved-routing, and undeliverable writes together.

## Provenance

The rejected detached worktree was based on `0aea6bbb`, 133 commits behind the
reviewed main, with 44 tracked and 14 untracked changes and no commit to
cherry-pick. Its unique useful idea was the downstream runtime-signer task and
wakeup-removal mutant; this issue preserves that implementation boundary so
the stale mixed rewrite can be discarded safely.
