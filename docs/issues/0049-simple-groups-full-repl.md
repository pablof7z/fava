# 0049 — Full reusable experiential simple-groups REPL

**Status:** implemented locally; all controlled checks except Croissant's
non-queryable kind-9008 deletion record are proven
**Owner:** `examples/simple-groups` owns NIP-29 and kind-10009 grammar, selected
group state, public Fava workflows, and command DTO selection; `e2e-support`
owns only shared bounded shell mechanics
**Related:** [0045 reusable E2E foundation](0045-e2e-repl-foundation.md),
[0031 simple-groups relay demo](0031-simple-groups-real-relay-demo.md)

## Decision

The application is one real command grammar, consumed by an interactive terminal
or an ordinary command file. Required ordinary arguments prompt only
interactively; replay returns a typed refusal before reading a later line.
`account import` is stricter: it accepts key material solely from a protected
no-echo terminal prompt and is unavailable to replay. InputMode refuses replay
before account-import omission handling can prompt.

The app keeps selected account and selected `SimpleGroup` independent. Account
commands alter only selected local author/signer attachment. Group create/open/
switch/delete alter only selected group state. A group id opened with another
relay set is refused rather than silently replacing its application-local route.

All NIP-29 management writes use public typed constructors and their self-routed
builders through `Fava::publish`. Arbitrary explicit-kind content uses public
`EventBuilder` plus `SimpleGroupEventBuilder`. No command writes a raw NIP-29
kind or reconstructs protocol tags. Kind-10009 changes are the one deliberate
different route shape: the app uses the public `Fava::to(...).by(...).publish`
edit door because a saved list has no group-host route.

Every ordinary mutation waits at most 20 seconds for `all_acknowledged()`. It
never calls `all_terminal()` as acknowledgement proof. Its complete terminal
receipt remains a typed failing result for rejected or timed-out writes. The
explicit `group event expect-rejection --kind <kind> [content]` path instead
waits for terminality and succeeds only if every desired destination rejected.
Every bounded read supplies a
whole-query limit (1–64), opens a public observation, and uses
`Observation::wait_until` for EOSE from every selected relay. State reads reverse
the generic kind with `SimpleGroupStateEventKind::try_from` and run all six
public decoders without canonicalizing relay state.

## Command and result contract

The README is the complete grammar for shared account/relay commands, group
create/open/list/switch/edit/invite/join/member add/member remove/leave/delete,
arbitrary-kind event publish/delete, bounded events/state reads, saved-list
show/group add/rename/remove/relay add/remove, status/routes/receipt
list/show/diagnostics/dump.

Every command renders one deterministic `CommandResult`; JSONL is one object per
line and interactive JSONL is refused before a prompt can share stdout. Fields
are typed public text, integers, booleans, or bounded scalar arrays; only scalar fields
are capture-safe. Every successful publish or delete, ordinary terminal write
failure, and expected rejection projects `author`, `event_id`, `write_id`,
`receipt_id`, `kind`, aggregate outcome/counts, and parallel bounded destination
relay/outcome/reason arrays; it additionally projects `group` and
publication relay where applicable. Explicit content publish projects its exact
supplied `content` only after result-size admission. This gives an independent
relay harness enough public evidence to assert exact author, id, kind, content,
and tags without inferring identity from a private key or app cache.

Secrets never appear in command history, results, captures, dump, replay input,
or logs. Credential-bearing relay URLs, nsec material, and raw 64-hex key
material outside a public-key/event-id grammar position refuse before history
or rendering. The opaque secret API consumes the protected input inside signer
attachment and cannot return secret bytes. The app does not render relay event
content/tags or diagnostic detail. It has no plugin framework, private Fava
workaround, provider profile, or canary dependency.

## Evidence and falsifiers

- The support test suite proves protected account ingress, account/relay command
  lifecycle, credential and contextual raw-hex refusal before retention, typed
  DTO/capture boundaries, JSONL output, and replay/interactive dispatch parity.
- The app unit test constructs all six state kinds and proves the reverse kind
  conversion selects every public decoder.
- The app black-box replay tests run `tests/shell.txt` without a PTY and probe
  every domain command family for one schema-valid JSON result without claiming
  live publication success. Its missing-required and missing-kind scenarios
  prove a replay cannot consume `account new alice` as a prompted group id or
  consume `quit` as an omitted explicit event kind.
- `scenarios/full-repl.txt` is the complete controlled two-relay walkthrough,
  including metadata `supported_kinds`, membership, saved lists, event deletion,
  and group deletion.
- The explicit isolated-PTY account-import proof creates one disposable valid
  nsec only in memory, fails on echo, scans retained artifacts with that exact
  input, and proves its returned public key is the direct-relay author of
  Fava-created kind 9007 and arbitrary kind 12345 events. Its canonical record
  contains public captures and EOSE evidence only.

Deleting `SimpleGroupEventBuilder` from arbitrary event publication, accepting
reversed publish tokens, routing a
group builder through `Fava::to`, replacing `wait_until` with an unbounded
observation stream, making a script prompt consume another line, retaining a
protected key, omitting an event's public author/id/write/kind facts, or using a
raw NIP-29 management kind must fail focused code, scenario, or live-harness
assertions.

## Remaining live fixture gap only

The controlled run used the ordinary full command file and exact direct
`REQ`/`EOSE` inspection against explicit Croissant and `nostr-rs-relay 0.8.12`.
It proved the named authorization, routing, state, deletion, and absence claims,
including 39000/39001/39002/39003 only. The acknowledged kind-9008 id receives
an empty direct `REQ`/`EOSE` after deletion because Croissant hides the deleted
group. The full contract remains correctly failing there until the fixture
exposes that retained deletion record publicly.
