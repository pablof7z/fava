# Edit author at the taker, not the edit

**Status:** proposed (awaiting Pablo approval — vocabulary + spec amendment)
**Authority:** `AGENTS.md` (focused local issue before implementation; vocabulary change),
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` WRITE-003 / ID-002 / ID-003,
`docs/spec/ARCHITECTURE.md` :167-178, :714-716, :900, :1997, :2080, :2583, and
:726-730 (illustrative)
**Companion:** `docs/issues/0014-publish-door-ergonomics.md` owns the door the
application knocks on (`publish` ergonomics, the `by` / `to` scopes, auto
default, `WriteIntent` demotion, `NonEmptyVec`). Neither lands alone.

## Product result

A `ReplaceableEventEdit` is a durable change to a replaceable event that carries
no author. The author is supplied to the *taker* — `WriteIntent::edit` resolves
the active signer; `WriteIntent::edit_as` takes an explicit pubkey — and is
resolved once at acceptance, persisted with the edit, and never re-resolved.
`fava_nip02::follow(target)` is one argument, engine-free, with no
`fava-signer` dependency and no coordinate construction. A restart after an
account switch never rematerializes Alice's follow as Bob's.

## The spec amendment — applied

Twelve sites said the edit carries its actor. All twelve are amended; `grep -rn
"actor" docs/spec/` now returns nothing.

**`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` (authority #1):**

- WRITE-003 (`:720`) — "the edit carries its actor … an event with that actor as
  `pubkey`" becomes "the accepted write carries its resolved author … an event
  with that author as `pubkey`".
- WRITE-003 (`:722`) — account resolution now happens "before the write is
  accepted, and the resolved author is committed with it", rather than "before
  producing the accepted event or edit".
- WRITE-006 (`:759`) — "retain the edit and actor" becomes "retain the edit and
  its resolved author".
- ID-003 (`:1170`) — "no explicit actor/pubkey" becomes "no explicit author
  public key". The requirement itself is unchanged and remains permanent.
- Summary bullet (`:1617`) — authorship comes from the event `pubkey` or "the
  author resolved when a replaceable-event edit was accepted".

**`ARCHITECTURE.md` (authority #2):**

- `:167-178` — the prose and the illustrative struct. `ReplaceableEventEdit`
  loses `actor` and the author half of its coordinate, keeping `kind` and
  `identifier`; materialization
  now produces an event whose `pubkey` is "the accepted write's resolved author".
- `:714-716` — prose, not an illustrative signature, so AGENTS.md's
  "preserve the behavior and ownership rule" escape does not cover it. "It
  identifies the actor and event coordinate" becomes "It identifies the
  coordinate it changes apart from the author — the replaceable kind, and the
  identifier when that coordinate is addressable … It carries no author."
- `:726-730` — illustrative signatures: `follow(actor, bob)` → `follow(bob)`,
  `fava_bookmarks::add(actor, target)` → `add(target)`.
- `:900` — owned state: "actor for replaceable-event edits" becomes "resolved
  author for replaceable-event edits".
- `:1997` — "The edit contains its actor, coordinate, durable protocol-owned
  change, and format version" becomes the coordinate apart from the author, the
  change, and the format, with the author carried by the accepted write.
- `:2080` — "a `ReplaceableEventEdit` whose `actor` is the resolved account"
  becomes "a `ReplaceableEventEdit`, whose accepted write records the resolved
  account as its author".
- `:2583` — the acceptance walkthrough drops `actor=alice` from the call and
  gains an explicit author-resolution step at acceptance.

The spirit survives exactly: resolved before acceptance, carried by accepted
state, never re-resolved. The Alice/Bob acceptance test is unchanged and still
passes.

`python3 tools/check_vocabulary.py` passes after the amendment — `EventCoordinate`
remains referenced elsewhere in the spec (`ARCHITECTURE.md:368`), so removing it
from the edit struct does not break registry closure.

## Architecture

- **The edit.** `ReplaceableEventEdit { kind, identifier, format, change }`.
  The `actor` goes; the coordinate stays, minus its author. Only the author half
  of the coordinate is redundant once the accepted write resolves it — `kind` and
  `identifier` are not. Dropping `identifier` too would bake in a limitation the
  spec does not have: `GOALS:528` and `ARCHITECTURE.md:411` both put
  "replaceable-event coordinates, including addressable coordinates" in scope,
  and the `identifier: None` requirement in `fava-nip02`
  (`fava-nip02/src/lib.rs:151-162`) is correct *for kind 3*, which is not
  addressable — it is not a global refusal of addressable coordinates. An edit to
  a kind-30023 article must be able to say which article. Both redundant author
  checks vanish (`edit.rs:106`, `fava-nip02/src/lib.rs:157`) because there is no
  second author field to disagree with.
- **nip02.** `follow(target)` — one argument, engine-free, no `fava-signer`
  dependency, no coordinate construction.
- **Where the author enters.** At the door, per `0014`: `fava.publish(edit)?`
  resolves the active signer, `fava.by(carol).publish(edit)?` names one. The
  application never constructs the intent. Internally that is still two
  constructors rather than an `Option` param, because Rust has no named or
  optional arguments:
  - `WriteIntent::edit(edit, routing)` — author = active signer.
  - `WriteIntent::edit_as(edit, pubkey, routing)` — explicit.
  `by` names a pubkey; signers are already indexed by pubkey
  (`fava-publication/src/lib.rs:62`), so naming the pubkey names the signer.
  The `fava-write → fava-signer` dependency cycle dissolves: `fava-write` names
  no `Signer`.
- **Resolve once, persist, never re-resolve.** `publish` resolves the active
  signer at acceptance; the write store persists the resolved pubkey alongside
  the edit; recovery (`recover_materialized_edits` → `fava-publication/src/run.rs:179`)
  reads the stored pubkey and never re-consults the session. A restart after an
  account switch must not rematerialize Alice's follow as Bob's — the WRITE-003
  acceptance test.
- **Before `fava-session` exists.** The no-`as:` branch refuses with a typed
  error. Not a placeholder: ID-003 (`GOALS:1170`) mandates it permanently —
  *"If a convenience publication operation requires a current account and none
  exists, and no explicit actor/pubkey is supplied, the operation MUST fail
  before creating a write or receipt."* The API shape is final on day one; when
  `fava-session` lands, only that branch changes from "always refuse" to
  "resolve or refuse."
- **Materializer contract gains the author.**
  `materialize(&self, edit, author, source, created_at)`. The author must be
  known before materialize regardless: source selection queries by it
  (`fava-publication/src/materialization.rs:367`) and qualification rejects a
  mismatched source (`fava-nip02/src/lib.rs:202`).

## Open: is the inverse stored or derived?

Not settled by this issue and deliberately not decided by the amendment. The spec
requires edits to have inverses — `ARCHITECTURE.md:3065`, `:3330`,
`GOALS:1223`, `FAVA_REWRITE_IMPLEMENTATION_PLAN.md:726` and its
`replaceable-edit-inverse` scenario (`:752`) — but has never put an `inverse`
field on the struct, and the illustrative struct here does not add one.

The m7 implementation stores it, and its own validation argues against that:
`decode_edit` (`fava-nip02/src/lib.rs:141-148`) *derives* the inverse via the
pure `Operation::inverse()` (`:80-85`) in order to check the stored one. If it can
be derived to validate it, it can be derived instead of stored. The
format-stability argument does not rescue it either, because `decode_edit`
refuses on `edit.format() != FORMAT` (`:141`) before it reads the inverse at all,
so an edit written under an older format is never decoded.

The one case where an inverse genuinely is not derivable — an edit like "set the
title to X", whose undo needs the previous title — is also the case where a
stored inverse cannot be correct, because the edit is constructed before any
source event is read (WRITE-006's offline edit) and is rematerialized against
newer sources afterwards.

That points at removing the field, which contradicts whiteboard decision D2 and
changes shipped M7 code. It needs its own focused issue and Pablo's call; it does
not block the author amendment either way.

## Blast radius — 13 non-test sites, 6 crates, mechanical

| crate | sites |
|---|---|
| `fava-write` | `edit.rs` struct/ctor/serde; `WriteIntent::edit`/`edit_as`; `materialization.rs` trait |
| `fava-nip02` | `:157` delete, `:184`, `:202`, `:280` |
| `fava-bookmarks` | `:269` delete, `:323`, `:341`, `:467` |
| `fava-publication` | `materialization.rs:367`, `:402`; author threading through `prepare_semantic` |
| `fava-write-store-{memory,redb}` | `semantic.rs:352`/`:297`, `redb/validation.rs:185`, redb `schema.rs:25` persist the author |
| `fava` | `publish`, and `preview_write_routes` (`:210-236`) — preview materializes, so it needs the same resolution |

## Two things not to miss

- **`preview_write_routes` is on this path** — it materializes to have an event
  to route (`fava/src/lib.rs:210-236`), so it needs the same author resolution.
  Easy to forget.
- **The silent no-signer hole becomes load-bearing.** `fava-publication/src/run.rs:306-308`
  returns without recording when no signer matches the author, leaving an
  accepted receipt that never progresses. With an active-signer default that is
  now a reachable normal state, so it becomes a typed refusal at `publish`
  rather than a silent stall.

## Vocabulary delta

`docs/internals/vocabulary.toml` `ReplaceableEventEdit` term (`:411-420`): the
persisted entity (no `actor`; the coordinate keeps `kind` and `identifier`) and the
provider contract (`ReplaceableEventMaterializer::materialize` gains `author`)
both change. `WriteIntent` gains `edit`/`edit_as`. Run
`python3 tools/check_vocabulary.py` (and its unit tests) after the edit.

## Exit-gate evidence

- Spec amendment applied to all twelve sites (see above); `grep -rn "actor"
  docs/spec/` returns nothing.
- `vocabulary.toml` updated; `tools/check_vocabulary.py` passes.
- Alice/Bob acceptance test: resolve Alice, switch to Bob, rematerialize from
  the stored write → the event is still Alice's.
- `follow(target)` one-arg compiles engine-free with no `fava-signer` dep;
  `fava.by(carol).publish(edit)?` compiles; the no-`by` branch refuses with a
  typed error before `fava-session` exists.
- `preview_write_routes` resolves the author the same way as `publish`.
- No-signer at `publish` is a typed refusal, not a silent stall.

## Falsifier evidence

Recovery that re-consults the session after an account switch rematerializes
Alice's follow as Bob's — the WRITE-003 acceptance test catches it. An edit
that still carries an `actor` keeps the redundant `actor == coordinate.author`
check and the `fava-write → fava-signer` dependency cycle, both of which this
design deletes.