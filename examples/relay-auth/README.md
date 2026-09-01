# relay-auth

An interactive and replayable shell over Fava's NIP-42 relay-authentication
surface: `fava_auth::{AuthenticationPolicy, Authenticator}`, `Fava::authentication()`,
`Fava::with_account`, and `AuthenticationState`.

A relay that requires proof of identity sends an `AUTH` challenge; the
application decides, per challenge, whether to sign a response, decline, or
hand the decision to a person; and every write or read against that relay
sees the same verdict, whichever door reached it. This app is a hands-on
surface for that whole lifecycle: run it, watch a relay demand
authentication, answer it, and see the exact wire and receipt consequences.

```sh
cargo build --manifest-path examples/relay-auth/Cargo.toml
./examples/relay-auth/target/debug/relay-auth
```

## The grammar

```
account new|import|add-pubkey|list|switch|replace|remove|clear
relay add|list|remove
policy set <authenticate|decline|defer>
auth pending
auth answer <demand-id> <authenticate|decline>
auth state <relay> <public|as:<account>>
query open <name> <public|as:<account>> <kind> <relay>...
query snapshot|wait|close <name> ...
publish <public|as:<account>> [for <author>] <kind> <content> <relay>...
receipt list|show|wait
diagnostics
routes
capture <name> <field>
dump
quit
```

Every command runs the same way whether typed at the prompt or replayed from
a `--script FILE`; the only difference is that a script refuses instead of
prompting when a required value is missing, so a bad scenario fails loudly
rather than silently blocking on stdin.

## What `public` and `as:<account>` actually mean

`RelayAccess::Public` and `RelayAccess::Authenticated(pubkey)` on the same
relay URL are two distinct sessions, with distinct connections and distinct
NIP-42 lifecycles. `query open mine as:alice 1 my-relay` and
`query open pub public 1 my-relay` against the *same* relay alias open two
separate observations; `auth state my-relay as:alice` and
`auth state my-relay public` can disagree. The app never conflates them.

`publish as:alice ...` authenticates the connection as alice and authors the
event as alice. `publish as:alice for bob ...` authenticates the connection
as alice but authors the event as bob -- built with `EventBuilder::by(bob)`,
which the facade layer respects because an author a payload already states
is never overridden. This is the "read Bob's mail over Alice's authenticated
line" shape a multi-account client actually needs, and it costs one extra
token in this app's grammar, not a new API.

## A policy a person can change while the engine runs

`fava_auth::AuthenticationPolicy::decide` is synchronous, effect-free, and
selected once at assembly time -- the same shape as `DeliveryPolicy`. `policy
set` doesn't reassemble the engine; it flips one `Mutex<AuthenticationDecision>`
the app's own `SwitchablePolicy` reads on every future challenge. Nothing
already decided is revisited: this changes what the *next* challenge gets.

## Answering for a person

`policy set defer` makes every future challenge on this engine wait for a
person. `auth pending` lists what's waiting; `auth answer <id> authenticate`
(or `decline`) resolves it. `publish` itself returns immediately once a
write is durably accepted (see below for why); `receipt wait <id>` blocks,
with an explicit bound, until routing settles.

The live proof demonstrates a demand actually appearing (`auth pending`),
being answered (`auth answer`), and the session's own `Attempted` verdict
following it -- through the real public API, against a real relay. It does
**not** demonstrate a *write* visibly held open by that demand:
`fava-publisher-nip01::Nip01Publisher`, the publisher every real Fava
assembly (this app and `examples/account` included) actually uses, maps
every relay `OK false` straight to `PublishOutcome::Rejected` and never
constructs `PublishOutcome::AuthenticationRequired` -- so the
`RelayDeliveryOutcome::Retryable`/`AuthenticationDenied` distinction
documented in `fava-publication/src/delivery.rs`, and proven in
`fava-publication`'s own tests, is reachable only through a scripted test
publisher, never through this or any other real application. See "Public-API
developer-experience gaps" below; this is the most consequential one this
app found.

## Why `publish` doesn't wait for its own write to settle

The account app's `publish` blocks until the write is terminal. This app's
does not: it returns as soon as the write is durably accepted, reporting
whatever receipt state exists at that instant. A deferred NIP-42 demand can
leave a receipt open for as long as it takes a person to run `auth answer`,
and this REPL processes one command line to completion before reading the
next -- if `publish` blocked for terminal settlement, nothing could ever run
`auth pending`/`auth answer` to unblock it from the same session. `receipt
wait <id>` is the explicit door for a caller that does want to block.

## Every reachable `AuthenticationState`

`auth state <relay> <public|as:<account>>` prints one of `unknown` (no
challenge seen yet), `challenge-received`, `declined`, `attempted`,
`awaiting-answer`, `accepted`, `accepted-but-still-refused`, `rejected`, or
`failed`, plus the relay's exact bounded message where one exists. The live
proof (`live/harness.py`) drives all of these except `challenge-received`,
which is real but transient: a policy decides synchronously in the same tick
a challenge arrives, so nothing can observe it standing still on purpose.

## Two relays, two different things they prove

`~/.cargo/bin/nostr-rs-relay` (0.8.12) is a real third-party implementation.
Configured with `nip42_auth = true` it sends a genuine `AUTH` challenge on
connect -- and that is *all* it does: an unauthenticated `REQ` or `EVENT`
still succeeds, and it does not answer a valid `AUTH` with any verdict at
all (a malformed one does get `OK false "restricted: ..."`, confirmed by
hand). It proves the real challenge/response wire leg, and nothing about
enforcement.

`examples/crates/e2e-support/live/nip42_relay.py` is a small relay this
harness fully owns, in three fixed modes (`accept`, `reject`,
`accept-refuse`). It refuses every `REQ` and `EVENT` with `auth-required:`
until a **verified** kind-22242 event arrives -- verified for real, by
shelling out to `nip01_wire` (below), never trusted on faith -- and then
answers exactly per its mode. This is what proves `Rejected`,
`AcceptedButStillRefused`, and real read/write enforcement; no relay this
repository can pin does that on its own.

`examples/crates/e2e-support/src/bin/nip01_wire.rs` exists because Python
ships no BIP-340 schnorr implementation: it verifies a delivered event's
id/signature, signs a fixture-side kind-22242 response, and derives a public
key from a secret, so the harness relay and the independent inspection
connection in `live/harness.py` never pretend to check what they cannot
check.

## Public-API developer-experience gaps this app surfaced

Building ordinary `publish as:<account>` workflows against this exact
surface found two things an app author should not have had to work around.
Both are documented at their exact call site (`examples/relay-auth/src/app.rs`,
`ensure_watched`), and both are reported to the surface's owner rather than
quietly patched over:

1. **`Fava::observe` wires up authentication watching for a query
   automatically; nothing wires it up for a write.** `Fava::observe` calls a
   private `watch_authenticated_relays` before opening. `Fava::publish`,
   `PublishAs::publish`, and `PublishTo::publish` do not call anything
   equivalent. A write to an authenticated session whose relay nothing has
   ever watched sees no `AuthenticationOutcomes::state`, is classified a
   flat denial, and no NIP-42 handshake is ever attempted -- silently. The
   app now calls `Authenticator::watch_session` itself before every
   `publish as:<account>`, which is the only public door available for this,
   and consequently `publish as:<account>` needs a reachable relay even to
   be *accepted* (see `tests/repl.rs` for why the offline suite only
   exercises `publish public`).
2. **A second `watch_session` on an already-resolved, still-live session
   destroys its verdict.** `SessionAuthentication::reconnected`, reached
   through `watch_session_inner`'s unconditional call on every invocation,
   resets state to `None` even when the connection's generation has not
   actually changed (`crates/fava-auth/src/state.rs`; contrast with
   `challenged`/`resolved`, which both guard on the generation first). A
   relay that (like this app's own harness relay, and many real ones)
   challenges once per connection then never repopulates that state, and the
   session reports `unknown` forever. The app now watches each session key
   at most once for the life of the process.

An independent DX review of this app (see `.pi/e2e-builder` for the process)
found five more, all confirmed against the exact cited source:

3. **`ensure_watched`'s bounded poll exists because `Authenticator` exposes
   no per-session change signal.** `Authenticator::subscribe()` fires when
   the *deferred-demand set* changes, not when one session's `state()`
   transitions. Waiting for a first challenge to arrive has no event to
   `.await`; polling is the only public option.
4. **`receipt_wait` duplicates logic Fava already has, one layer down.**
   `fava_publication::Publication::wait_until` -- exactly the bounded
   broadcast-with-lag-handling loop `receipt_wait` reimplements -- exists but
   is unreachable from `Fava`, which exposes `receipt()` and
   `receipt_changes()` and no `wait_until(receipt_id, predicate)`. `Write::settled`
   has the same logic reachable only from a live `Write` handle, not a
   reattached receipt id.
5. **"Why is this write stuck" has no typed answer.** `WriteStall` has no
   authentication-awaiting variant, and nothing in the workspace constructs
   `fava_diagnostics::WriteStall` at all (`diagnostics().writes` is always
   empty). The only place the real reason exists is
   `RelayDeliveryOutcome::Retryable`'s free-text `reason` string in
   `fava-publication/src/delivery.rs`. The app's `routes` + `auth state` +
   `receipt show` commands assemble the real picture from three public
   sources rather than inventing one, but a single-call answer does not
   exist.
6. **Four ways to reach roughly one door, only three of four axis
   combinations reachable.** `Fava::publish`, `PublishAs::publish`,
   `PublishTo::publish`, and `PublishTo::with_account(..).publish` each fix a
   different (author, access) pairing; "authored by bob, access as alice" is
   only reachable by escaping to `EventBuilder::by()`, which is exactly what
   `publish as:<account> for <author>` does. `with_account`'s signature names
   one fact (an account); its doc names two (author and access).
7. **`Authenticator::answer` accepts a decision it always rejects.**
   `AnswerError::DeferredAgain` exists purely to reject
   `AuthenticationDecision::Defer`, forcing an `unreachable!()` a few lines
   later. A narrower two-variant answer type would remove both the runtime
   error and the panic path.

None of the seven is app-owned ceremony this shell should quietly absorb;
all are recorded here with their exact evidence for Fava's owner to decide
on.

## Live proof

```sh
cargo build --manifest-path examples/relay-auth/Cargo.toml
cargo build --manifest-path examples/crates/e2e-support/Cargo.toml --bin nip01_wire
python3 examples/relay-auth/live/harness.py \
  --app examples/relay-auth/target/debug/relay-auth \
  --relay ~/.cargo/bin/nostr-rs-relay
```

See `live/README.md` for exactly what it proves and how.
