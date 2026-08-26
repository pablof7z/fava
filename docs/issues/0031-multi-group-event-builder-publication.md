# 0031 — Multi-group event publication belongs in one `EventBuilder`

**Status:** architecture approved by Pablo, 2026-08-26; core implementation present; fresh vocabulary signatures and controlled two-group relay run pending
**Owner:** `fava-write` for neutral event-plus-route construction; `fava-simple-groups` for NIP-29 `h` composition; `fava` for universal publication admission
**Related:** `docs/issues/0019-simple-groups.md`; OpenSpec `support-multi-group-event-publication`

This issue records the approved architecture decision. It is not a vocabulary
signature or merge-readiness claim.

## Decision

`EventBuilder` remains the concrete fluent type after every
`.simple_group(&SimpleGroup)` call. It holds two separate pre-custody facts:

- Nostr event fields, including the ordered exact `h` tags that determine one
  event id and signature;
- neutral `WriteRouting`, which carries the bounded, first-occurrence-ordered
  union of selected group hosts and is neither serialized nor signed.

`fava-simple-groups` supplies the extension trait and owns only the exact
two-cell `h` contribution. `fava-write` owns generic tags, event construction,
route normalization, bounds, and typed refusal. `fava.publish(builder)` lowers
the two builder facts through the existing `WriteIntent` and universal
publication lifecycle. It does not create a simple-groups publisher, signer,
receipt, router, or provider.

## Closest existing concepts and insufficiency

`EventBuilder` currently carries only event fields and `build()` returns only
`UnsignedEvent`; it cannot retain selected hosts. `WriteRouting` already owns
automatic versus exact explicit routing but is currently supplied only through
the facade. `SimpleGroup` already owns one opaque id and selected host set but
its `prepare` path only establishes a single matching context and requires the
application to restate hosts at publication.

Neither a wrapper, a global group registry, relay hints in signed tags, nor a
type-erased metadata bag meets the decision. A wrapper changes the fluent type;
a registry hides lifecycle state and can resolve a group id to the wrong hosts;
tags turn local routing into wire semantics; and an extension bag invents an
unbounded provider framework for one neutral route fact.

## Observable distinction and counterexample

Two distinct `SimpleGroup` values can contribute two exact `h` tags to one
unsigned event and their hosts to one deduplicated explicit route. The event is
signed once and each destination receives the same event id and signature.

Without this model, composing `group_a` then `group_b` either creates two
events, loses one route, changes the fluent type, or requires a second
publication door. Sending a builder with embedded routing through
`fava.to(...).publish(builder)` has two route authorities and must refuse
before signing or custody rather than merge silently.

## Lifecycle and forcing requirement

The builder owns transient declarative fields only until `fava.publish` opens
the existing durable write path. `WriteIntent` then owns the accepted event and
resolved route; receipt, retry, restart, and delivery remain with their current
owners. The forcing requirement is one signed kind-blind event that can be
selected by several group contexts and published to their complete routes
without coupling generic event construction to NIP-29.

Pre-signed events remain immutable. Their selected contexts are validated
without rewriting tags and their host union stays an explicit facade route.

## Relay limitation

Repeated `h` tags are a Fava publication contract, not a claim that all
third-party NIP-29 relays admit or moderate them as independent groups. A
shared relay acknowledges one event delivery, not group-qualified admissions.
Fava retains actual relay evidence and never infers unobserved group success.

## Executable falsifiers

- `cargo test -p fava-write --test event_builder --test routing_order`
- `cargo test -p fava-simple-groups --test public_api --test architecture`
- `cargo test -p fava --test publication_door --test publication_scopes --test simple_groups --test multi_relay`
- controlled two-group relay canary: publish one event with two exact `h` tags,
  query each group, and require the same event id/signature with actual relay
  evidence

Removing builder-carried routing, allowing `fava.to(...).publish(builder)` to
merge routes, duplicating a matching `h`, or routing by a hidden registry must
make the corresponding focused evidence fail.

## Current implementation evidence

- `cargo test -p fava-write` passes: route-only changes preserve event identity
  and event-only construction refuses attached routing.
- `cargo test -p fava-simple-groups --test architecture --test public_api`
  passes: fluent composition retains `EventBuilder`, exact group tags, and the
  normalized route union.
- `cargo test -p fava --test simple_groups --test publication_scopes` passes:
  direct builder publication retains its route and a second explicit facade
  route refuses before signer, custody, or relay work.
- `cargo check --manifest-path apps/canary/Cargo.toml` passes after replacing
  the removed unsigned `prepare` calls with builder composition or explicit
  low-level fixture tags.

The real Croissant two-group run has not been executed: it requires its pinned
relay binary and source-attested launch inputs. The global vocabulary checker
remains red on its inherited repository backlog; the new builder/group symbols
are registered and do not appear in its reported findings.
