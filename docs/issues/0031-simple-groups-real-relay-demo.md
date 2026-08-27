# Runnable simple-groups real-relay demo

**Status:** implemented
**Branch:** `feat/simple-groups-real-relay-demo`
**Requested by:** Pablo, 2026-08-27
**Related:** `docs/issues/0019-simple-groups.md`

## Outcome

`examples/simple-groups` ships a literal application binary that generates
disposable Alice, Bob, and Carol keypairs and exercises the current public
simple-group surface through an assembled `Fava` engine against real WebSocket
relays. It is application code, not a test, fixture, mock, or testkit.

The group relay and the kind-10009 user relay are separate selections because a
NIP-29 group host such as Croissant intentionally rejects non-group events,
while saved group lists are user preference events. They may still be the same
URL when one relay serves both roles.

## Scope

The demo uses every current typed `fava-simple-groups` management constructor, every
`SimpleGroup` method, every `SimpleGroupMetadata` accessor, all six typed state
selectors and decoders, and the complete saved-group edit/query lifecycle. It
prints the exact acknowledgement or decoded result for every step.

The current typed metadata edit does not expose `supported_kinds`, and the
current typed invite and join-request constructors do not expose Croissant's
required invite code. The demo retains each typed constructor as the management
kind owner, then rebuilds that returned unsigned event with the additional
protocol tags. It contains no raw management kind construction.

This issue adds no production API, crate, provider contract, lifecycle owner,
compatibility path, or architectural vocabulary.

## Executable evidence

- `cargo build --manifest-path examples/simple-groups/Cargo.toml --locked`
  builds the literal app.
- A controlled run against an isolated Croissant group relay and an isolated
  `nostr-rs-relay` user relay receives acknowledgements for create, metadata,
  invite, join, member additions, leave, removal, content, all saved-list edits,
  event deletion, and group deletion.
- The same run observes the published content with relay provenance, decodes
  Croissant's relay-authored metadata/admin/member/role state, and observes each
  kind-10009 replacement from the user relay.
- The app removes the isolated Croissant process and data on every exit path.

## Validation

Green: changed-file rustfmt, exact example build, focused Clippy with warnings
denied except the unchanged `people.rs` baseline lint, 34 library/integration
tests, 32 doctests, and diff whitespace validation.

The global vocabulary gate remains red on its existing repository backlog; the
example exposes no public helper item and adds no new vocabulary diagnostic.
