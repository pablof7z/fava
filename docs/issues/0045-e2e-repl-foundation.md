# 0045 — Private reusable E2E REPL foundation

**Status:** architecture and vocabulary approved by Pablo, 2026-08-28; first
`simple-groups` vertical implemented
**Owner:** `examples/crates/e2e-support` for shared E2E application-shell state
and ingress; each example for its domain commands and Fava workflow
**Related:** `docs/issues/0031-simple-groups-real-relay-demo.md`

## Decision

`examples/crates/e2e-support` is a private, standalone `publish = false`
package outside the root workspace. It owns one bounded `E2eSession` that
executes the same line parser for an interactive terminal and ordinary
non-PTY/script input. Its built-ins are deliberately limited to account
selection, relay aliases, capture/interpolation, dump, quit, protected secret
prompting, retained history, result rendering, and their explicit limits.

`examples/simple-groups` is the first real consumer. It owns `group create`,
`group use`, `group event publish --kind <kind> [content]`, `group delete`,
and `group list`, its selected `SimpleGroup` state, its Fava provider assembly,
and its publication acknowledgement rule. Creating a group selects it; `group
use` chooses another known group. The publish command accepts every `u16` Nostr
kind and has no content-kind policy. It sends only public Fava/provider values
through `Fava::to(...).publish`, `Fava::publish`, `Write::settled`, and
`all_acknowledged`.

The support package creates no provider profile, signer, route choice,
publication/query/observation lifecycle, relay fixture, group context, domain
registry, command registration protocol, plugin boundary, or second scenario
grammar. It has no path into `apps/canary`.

## Ownership and lifecycle

The domain application creates its signer keys and registers their Fava
signers. It supplies bounded `Account` aliases to `E2eSession`; selecting an
account changes only the unsigned event author used by the next domain command.
It does not mutate Fava's runtime signer attachments.

Relay aliases resolve a user-supplied `RelayUrl` before a domain command uses
it. `CommandResult` is an app-shell DTO, not receipt or query state. The
session retains at most the configured aliases, history, captures, last result,
and bounded scalar strings. Both modes run the identical parser,
interpolation, selection, dispatch, result, and history path. A domain command
may ask for a missing non-secret value only in interactive mode. Script replay
renders that refusal and stops with the exact error instead of consuming a
later command as the missing value.

`Secret::prompt` requires a terminal and disables echo. The type is opaque,
zeroizing, non-serializable, and only exposes a scoped conversion. Protected
prompt values have no history entry. The shell refuses nsec/PEM/common
assignment-shaped command-line input before history and never includes the
rejected text in rendered script output; secret-shaped result fields are
refused before rendering, capture, interpolation, or dump.

## Vocabulary reconciliation

This is a user-approved focused vocabulary change. The vocabulary gate now
discovers only reusable `examples/crates/**` packages, while continuing to
exclude runnable downstream examples. `docs/internals/vocabulary.toml` records
the private crate and every cross-example nominal type: `Account`,
`CommandResult`, `E2eSession`, `InputMode`, `Limits`, `OutputFormat`,
`ResultStatus`, `Secret`, and `ShellError`.

`E2eSession` is intentionally distinct from Fava `Session`: the former owns
app-shell state and no provider lifecycle; the latter owns runtime signer
attachment and no shell state. `CommandResult` is distinct from `Receipt` and
`QuerySnapshot`; it is a bounded presentation/capture DTO. `Secret` is an
ingress guard, not a signer or key store.

Counterexample: moving group grammar into generic support, allowing it to
select/replace a Fava signer, putting relay fixture/process lifecycle there, or
adding a domain-command registry would collapse owners and makes the second
application change the shared foundation for a domain-only concern.

## Executable falsifiers and evidence

The causal RED run added the support test before a library existed. It failed
with unresolved `e2e_support`; that test now proves all of the following:

- relay aliases, account selection, capture/interpolation, dump, and JSONL run
  through one `E2eSession` dispatch path;
- protected prompting refuses non-terminal script input before history; a
  secret-shaped script command is refused without echoing its value; and a
  `token` result field is refused before it can reach a renderer or capture;
- one-alias limits refuse the second relay before retention, and unknown
  interpolation creates no capture; a one-field result policy rejects a second
  result field before it becomes the next capture source;
- interactive missing-kind input prints a `kind>` prompt without adding a
  history entry, while a script missing the same required value refuses before
  reading another line;
- `examples/simple-groups/tests/shell.txt`, run as ordinary script input,
  exercises the support built-ins and a real outside-support `group list`
  command, producing one JSON object per line;
- every group mutation waits only up to 20 seconds for
  `all_acknowledged()`. It reports the Fava error/timeout as its own scoped
  command failure; it never converts unresolved work into acknowledgement.

Deliberately moving `group list` into `E2eSession`, recording protected input,
removing the second-alias check, treating result-field count as capture bytes,
rendering secret-shaped fields, or routing a group builder through
`fava.to(...)` must make the applicable focused test or public Fava admission
fail.

The performed mechanism-disable mutation changed the second-alias admission
condition to `false`. `bounds_refuse_before_retaining_external_input` then
failed because `relay add two` returned `relay-added`; the condition was
restored before the GREEN run.

## Scope and remaining proof

This slice contains only the private support package, its focused tests,
simple-groups conversion, README/command fixture, vocabulary gate/reconciliation,
and this issue. It does not claim a controlled NIP-29 create/event-publish/
delete run: that requires the external group relay fixture and independent wire
inspection already owned by the canary/lab path. It is the remaining live-proof
blocker, not an excuse to add a fixture or protocol policy to shared support.
