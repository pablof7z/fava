# Publish scope handles: `PublishAs` and `PublishTo`

**Status:** proposed (awaiting Pablo approval — vocabulary change)
**Authority:** `AGENTS.md` §"Architectural vocabulary" (`:51-60`) — vocabulary is
closed by default; a public nominal type is a vocabulary change; a feature change
cannot approve its own new vocabulary.
**Companion:** `docs/issues/0014-publish-door-ergonomics.md` is the feature slice
that consumes these two nouns. It depends on this issue and does not declare them.

## Problem

The publish door narrows a publish along two axes before the payload is named:
which signer authors it, and which relays receive it. Rust has no keyword
arguments, so the narrowing has to be a value. Two values exist because the
author axis is meaningful only for a replaceable-event edit — an unsigned event
already carries its `pubkey` and a pre-signed event has already used a signer, so
naming a signer for either could only be ignored or contradictory, which
WRITE-003 forbids ("No parallel author field may contradict the event or edit").

Making that unrepresentable rather than refused at runtime — `AGENTS.md:72`,
"Make invalid use unrepresentable or refuse it before opening work" — requires
the signer-scoped value to accept only edits, and therefore to be a distinct type
from the relay-scoped value.

## The two terms

```rust
#[must_use] pub struct PublishAs<'a> { /* private */ }
#[must_use] pub struct PublishTo<'a> { /* private */ }
```

Both are produced by `Fava`, borrow it, and are consumed in the expression that
creates them. Neither is stored, serialized, or recovered.

### `PublishAs`

- **Closest existing concept.** `WriteIntent` (`vocabulary.toml:395-409`) — an
  application request asking Fava to accept responsibility for publishing.
- **Observable distinction.** A `WriteIntent` is complete and carries its payload;
  `PublishAs` carries no payload at all. It carries one resolved signer public key
  and, optionally, a relay set, and it exists only until `publish` is called on
  it. It is the narrowing, not the request.
- **Counterexample.** `fava.by(carol)` with no following `publish` produces no
  write, no receipt, no store row, and no provider work — it is inert. A
  `WriteIntent` that reaches `accept` always produces durable custody.
- **Owner and lifecycle.** Owned by the `fava` facade. Created by `Fava::by`,
  borrows the engine for its lifetime, consumed by `PublishAs::publish` or dropped.
  Never persisted; never crosses a provider contract.
- **Forcing requirement.** WRITE-003 (`GOALS:716`) plus `AGENTS.md:72`. The author
  axis must exist for edits and must not exist for the other two accepted forms
  (WRITE-002, `GOALS:706`). A single scope type carrying an optional signer would
  make `signed_by` on a pre-signed event a runtime refusal instead of a compile
  error.
- **Why existing state is insufficient.** `WriteIntent` cannot express "signer
  chosen, payload not yet named"; it requires the payload at construction.
  `WriteRouting` carries no author. There is no existing type for a partially
  specified publish.
- **Executable falsifier.** A `compile_fail` doctest: `fava.by(carol).publish(e)`
  where `e` is an `UnsignedEvent` must fail to compile. If it compiles, the
  distinction this term claims does not exist.

### `PublishTo`

- **Closest existing concept.** `WriteRouting::Explicit` (a symbol of the
  `WriteIntent` term, `vocabulary.toml:395-409`).
- **Observable distinction.** `WriteRouting` is a field of a complete intent and
  is carried through acceptance into the receipt (`Receipt.routing`,
  `fava-write/src/lib.rs:424`). `PublishTo` is a pre-payload narrowing that never
  reaches the store; it produces a `WriteRouting` and is discarded.
- **Counterexample.** `fava.to([r1, r2])` with no following `publish` contacts no
  relay, opens no route session, and leaves no receipt. A `WriteRouting` inside an
  accepted intent always names destinations the receipt must account for.
- **Owner and lifecycle.** Owned by the `fava` facade. Created by `Fava::to`,
  borrows the engine, consumed by `PublishTo::publish` or by `PublishTo::by`
  (which yields a `PublishAs`), or dropped. Never persisted.
- **Forcing requirement.** WRITE-011 (`GOALS:808`) gives every write exactly two
  routing modes and makes automatic the ordinary one. Keeping the explicit mode
  off the ordinary call site, while still allowing it without an options struct,
  requires a value that can hold relays before the payload exists.
- **Why existing state is insufficient.** Passing `WriteRouting` positionally to
  `publish` puts routing syntax on a call that WRITE-011 and
  `partial-spec-api-semantics.md:26` say should need none in the ordinary case,
  and it cannot compose with the signer axis without a second parameter.
- **Executable falsifier.** A test asserting that constructing `fava.to([r1])` and
  dropping it produces zero write-store rows and zero transport calls. If it does
  not, the type is not the inert narrowing this term claims.

## Distinction from `Publisher`

`Publisher` (`vocabulary.toml`, owner `fava-publisher`) is the NIP-01 provider
contract that hands one attempt to a transport (`ARCHITECTURE.md:1621-1626`).
`PublishAs` and `PublishTo` are application-facing narrowings that never reach a
provider. A `Publisher` is selected once at assembly and lives for the engine's
lifetime; a scope handle lives for one expression. `tools/check_vocabulary.py`'s
`closest_registered_noun` will match on the shared word, so both terms must state
this distinction explicitly.

## Why `#[must_use]`

`.planning/codebase/CONVENTIONS.md:84` — "Mark constructors, builders, accessors,
and immutable transforms with `#[must_use]` when silently discarding the result
would be suspicious." A scope handle that is built and dropped does nothing at
all, which is the definition of suspicious. Both types carry it.

## Exit gates

- `docs/internals/vocabulary.toml` gains both terms with all seven items, and the
  symbols exist in the same commit — `tools/check_vocabulary.py` enforces closure
  in both directions, so registering early fails as hard as registering late.
- `python3 tools/check_vocabulary.py` and its unit tests pass.
- The two falsifiers above exist as tests.
- No other issue in this series declares either noun.
