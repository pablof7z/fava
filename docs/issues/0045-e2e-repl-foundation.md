# 0045 — Reusable bounded E2E REPL foundation

**Status:** implemented
**Owner:** `examples/crates/e2e-support` owns shared terminal/script mechanics
and local account commands; each runnable example owns its domain grammar and
public Fava workflows

## Current model

`e2e-support` is a private `publish = false` package outside the root
workspace. One bounded `E2eSession` runs the identical parser, interpolation,
history, dispatch, result, and JSONL path for interactive input and ordinary
non-PTY replay. Its only command families are local accounts, relay aliases,
captures, dump, and quit.

`account new` creates a disposable local keypair; `account import` accepts an
nsec or hex key only from a no-echo terminal prompt; `account remove` detaches
the matching signer. The package invokes `Fava::add_signer` and
`Fava::remove_signer` only through the public facade. Fava remains the signer
attachment owner: support does not retain a secret key as Account state, read
session internals, attach a provider privately, publish, query, observe, route,
or create a relay fixture.

The package bounds retained aliases, captures, history, command bytes, expanded
command bytes, arguments, result fields, and result scalar bytes. It rejects
secret-shaped ordinary input before history or dispatch and refuses secret-shaped
result field names or values before output, capture, interpolation, or dump. A
command that promises to expose explicit event content checks the result-scalar
bound before it can accept its write.

`CommandResult` is the one shell presentation DTO. It has deterministic field
order and exactly one JSON object per JSONL line. It may project exact public
Fava facts such as author, event id, write id, receipt id, kind, group, and
caller-supplied content; it is not a receipt, query snapshot, diagnostic, or
secret container.

## Boundary

The foundation deliberately has no group grammar, domain-command registry,
plugin protocol, provider profile, relay process lifecycle, or app-specific
state. A different example can replace simple-groups commands without changing
the shared parser or result safety rules. A group builder remains self-routed;
support never supplies a second Fava route.

## Evidence

`examples/crates/e2e-support/tests/foundation.rs` proves shared account and
relay commands, secret non-disclosure, result/capture bounds, and one dispatcher
for non-PTY and interactive input. `examples/simple-groups/tests/repl.rs`
proves an ordinary command-file replay and that a missing domain value stops a
script before the following line can be consumed.
