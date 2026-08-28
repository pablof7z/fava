# Simple-groups E2E REPL

`simple-groups` is a reusable experiential NIP-29 application, not a relay
fixture. It composes only public Fava and `fava-simple-groups` APIs: Fava owns
the signer attachment, write, receipt, query, observation, and diagnostics
lifecycles; this app owns the NIP-29 command grammar and selected
`SimpleGroup`; `e2e-support` owns the bounded common shell.

Run it interactively against a controlled group relay and a separate user
relay for kind 10009:

```sh
cargo run --manifest-path examples/simple-groups/Cargo.toml
```

Replay the identical grammar without a PTY:

```sh
cargo run --manifest-path examples/simple-groups/Cargo.toml -- --jsonl \
  --script examples/simple-groups/scenarios/full-repl.txt
```

Every command emits one deterministic typed result. `--jsonl` emits exactly
one JSON object per line with `status`, `kind`, `summary`, and stable typed
`fields` (text, nonnegative integer, boolean, or bounded scalar arrays). `capture
<name> <last-result-field>` accepts only a scalar field for `${name}`
interpolation. JSONL is refused for interactive input so prompts cannot corrupt
the stream. Results omit relay-received event content,
tags, relay responses, and diagnostics text. An explicit event publish returns
only its caller-supplied content, after protected-secret and result-size checks,
so external input cannot turn a result into a secret-bearing transcript.

## Shared shell grammar

```text
account new <alias>
account import <alias>
account list
account switch <alias>
account remove <alias>

relay add <alias> <ws-url>
relay list
relay remove <alias>

capture <alias> <last-result-field>
dump
quit
```

`account new` creates an ephemeral local Nostr keypair and selects it.
`account import` accepts an nsec or hex secret only from a no-echo terminal
prompt, then attaches a `LocalSigner` through `Fava::add_signer`. It cannot run
from a script, argv, capture, result, history, or environment. `account
remove` calls `Fava::remove_signer` before it drops the bounded shell alias.

Relay aliases are public endpoint names: credential-bearing URL authorities and
credential query parameters are refused. The shell also rejects nsec material
and raw 64-hex key material before history, interpolation, output, or domain
dispatch unless that exact grammar position is a public key or event id. It bounds
accounts, relays, captures, history, command and expanded-command size,
arguments, aliases, result fields, and every retained scalar value.

## Simple-group grammar

```text
group create <id> <relay-alias> [relay-alias ...]
group open <id> <relay-alias> [relay-alias ...]
group list
group switch <id>
group edit [--name <text>] [--about <text>] [--picture <url>]
           [--private|--public] [--closed|--open]
           [--supported-kinds <kind> ...]
group invite <code>
group join [code] [reason]
group member add <public-key> [role ...]
group member remove <public-key>
group leave
group delete [id]
group event publish --kind <kind> [content]
group event expect-rejection --kind <kind> [content]
group event delete <event-id>
group events [limit]
group state [limit]
```

`create` and every typed management builder use their self-selected group
routes and publish through `Fava::publish`; this app never adds a conflicting
`Fava::to(...)` route. `group event publish` accepts every `u16` Nostr kind,
adds the group's exact `h` context through `SimpleGroupEventBuilder`, and has
no content-kind policy. Supplied publish tokens are exact: `--kind` precedes
the kind and optional content. `group edit --supported-kinds` can be empty and
keeps all supplied kinds in order, including repetitions, through `MetadataEdit`.
`expect-rejection` waits for terminal delivery, succeeds only when every
desired relay rejects, and exposes the attempted event id for an independent
absence check.

The selected account and selected group are independent. Creating or opening a
group changes only the active group; switching/removing an account never
changes it. A known group id cannot silently be reopened with different relays.
At most eight known groups are retained; create/open refuses before a ninth
group could be published or retained.

`group events` and `group state` apply a caller-visible result limit of 1–64
(default 16), open public Fava observations, and wait with
`Observation::wait_until` for EOSE from every selected relay. `state` converts
the generic kind back with `SimpleGroupStateEventKind::try_from` and invokes all
six public decoders: metadata, admins, members, roles, LiveKit participants,
and pins. Result fields report bounded structural facts and never invent a
canonical state record.

## Saved lists and inspection

```text
saved-list show <relay-alias> [limit]
saved-list group add <publication-relay-alias> [display-name]
saved-list group rename <publication-relay-alias> <display-name>
saved-list group remove <publication-relay-alias>
saved-list relay add <publication-relay-alias> <saved-relay-alias>
saved-list relay remove <publication-relay-alias> <saved-relay-alias>

status
routes [limit]
receipt list
receipt show <receipt-id>
diagnostics
dump
```

The first saved-list relay is the explicit publication/query relay for the
account's kind-10009 list; it is deliberately distinct from the active group's
NIP-29 host. Saved-list changes use the semantic materializer through
`Fava::to(...).by(...).publish(edit)`, then require bounded
`all_acknowledged()` evidence. `receipt list` reports Fava's current open
obligations; `receipt show` reads the exact retained receipt by id. `routes`
is an inert read-route preview. `diagnostics` reports bounded category counts,
not untrusted diagnostic prose.

Every publish, deletion, expected rejection, terminal rejection, and timeout
result carries typed `author`, `event_id`, `write_id`, `receipt_id`, `kind`,
aggregate outcome/counts, and parallel bounded destination relay/outcome/reason
arrays. Ordinary terminal failures emit this record then stop replay;
only `expect-rejection` is a successful, capturable negative assertion.

## Prompts and proof boundary

An interactive omission of a required ordinary value prompts for that value
without adding it to history. In a script the same omission returns one typed
refusal and exits before consuming the next line. Protected account import
uses its own no-echo prompt and is always unavailable to a script.

`tests/shell.txt` proves a no-PTY shared-shell replay. `tests/missing-kind.txt`
and `tests/missing-required.txt` prove that replay cannot consume a later line
as a prompt response. `scenarios/full-repl.txt` is the controlled live-relay
walkthrough, including outsider and post-deletion rejected writes whose event
ids support independent absence reads. It still requires a supervised group
relay, a user relay, and independent bounded wire inspection before it can
claim real-relay proof. Croissant currently authors only state kinds
39000–39003; the all-six decoder test is synthetic coverage, not a live
39004/39005 claim.
