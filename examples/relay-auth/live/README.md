# relay-auth live proof

The harness starts four disposable relays, runs ordinary `relay-auth`
command lines through the real binary, and independently re-derives every
claim over its own fresh connections -- never the application's own.

```sh
cargo build --manifest-path examples/relay-auth/Cargo.toml
cargo build --manifest-path examples/crates/e2e-support/Cargo.toml --bin nip01_wire
python3 examples/relay-auth/live/harness.py \
  --app examples/relay-auth/target/debug/relay-auth \
  --relay ~/.cargo/bin/nostr-rs-relay
```

## What proves what

- **`nostr-rs-relay` 0.8.12** (`~/.cargo/bin/nostr-rs-relay`, `nip42_auth =
  true`) proves the real challenge/response wire leg against a genuine
  third-party relay: it sends an actual `AUTH` challenge, and Fava answers
  with a real signed kind-22242 event. It proves nothing about enforcement --
  confirmed by hand, this exact binary answers a *malformed* `AUTH` with `OK
  false "restricted: ..."` but gives no verdict at all to a valid one, and
  never gates `REQ`/`EVENT` either way.
- **`examples/crates/e2e-support/live/nip42_relay.py`** (harness-owned, three
  fixed-mode instances: `accept`, `reject`, `accept-refuse`) proves real
  enforcement: every `REQ` and `EVENT` is refused with `auth-required:` until
  a genuinely verified kind-22242 event arrives, and each mode's exact reply
  to a *valid* one is what drives `Accepted`, `Rejected`, and
  `AcceptedButStillRefused`.

Every kind-22242 event this relay receives -- from the application under
test or from this harness's own independent inspection connections -- is
verified for its real id and BIP-340 schnorr signature by
`examples/crates/e2e-support/src/bin/nip01_wire.rs`, built once and shared:
Python has no bundled implementation, so nothing here is asserted without a
real check.

## Independent inspection, not self-report

Command-result rows only prove what the application claims happened. This
harness never stops there:

- The two publicly readable events (`nostr`, which does not gate reads) are
  independently fetched by exact id over a fresh `REQ`/`EOSE` connection and
  their signatures independently re-verified.
- The two authenticated events on the `accept` relay are fetched the same
  way, but only after this harness's *own* connection independently
  completes a real NIP-42 handshake as the same account -- proving the
  relay's enforcement is real, not merely believed by the one connection
  that happens to be the application under test.
- The `reject` and `accept-refuse` relays' refusals are independently
  reproduced: a fresh connection authenticates as the exact account the
  scenario used, and this harness asserts the *exact* wire reply
  (`OK false "error: ..."` / `OK false "restricted: ..."`), then confirms a
  still-unauthenticated `REQ` keeps being refused.

## What it proves live

- Every `AuthenticationState` but `ChallengeReceived` (real but transient;
  see the app README), driven through real relay wire traffic and asserted
  through `auth state`, in this exact order: `accepted` (accept relay),
  `failed` (no signer attached, over a real connection to `accept-refuse`),
  `declined` (a local policy decision, never touching the wire), `rejected`
  (a valid `AUTH` genuinely refused by `reject`), `accepted-but-still-refused`
  (a valid `AUTH` genuinely refused by `accept-refuse` with `restricted:`),
  `awaiting-answer` then `attempted` (a deferred demand, answered by a
  person, completing against `nostr-rs-relay`).
- **A deferred demand is genuinely answered by a person, end to end**: the
  demand appears in `auth pending`, `auth answer authenticate` resolves it,
  and the session's own verdict moves `awaiting-answer` -> `attempted`. What
  this scenario does *not* show is a write visibly *held open* by that
  demand: the deferred write targets `nostr-rs-relay`, which never gates
  `EVENT` at all, so its own completion says nothing about the auth
  mechanism. See the app README's "Public-API developer-experience gaps"
  for why that stronger claim currently cannot be made against any real
  publisher.
- **Reads are gated exactly like writes, on the same session**: after the
  authenticated writes above, `query open accept-read as:alice 1 accept`
  opens with an empty local snapshot (`event_count: 0`, `revision: 1`,
  matching LOCAL-08), then `query wait` blocks on a real authenticated `REQ`
  against `accept` and returns `event_count: 1` -- the same event this
  harness's own independent inspection connection reads back separately.
- **`Fava::with_account` authors as one account over another's authenticated
  connection**: the `cross-write` case publishes an event authored by Bob
  while authenticated as Alice, and this harness independently confirms both
  facts -- the event's real author, and which account's authenticated
  session carried it -- by authenticating as *Alice* on its own inspection
  connection before reading it back.
- **Secrets never enter retained evidence**: `require_no_secrets` greps the
  entire evidence tree for all three imported nsecs and fails the run if any
  appear. This is a bounded evidence check, not an app-side redaction
  policy -- issue 0053 deliberately removed secret guardrails from every E2E
  testing surface, and this app carries none.

Canonical evidence is in `evidence/2026-09-01-live-nip42/`; `manifest.json`
records every retained artifact's byte count and SHA-256 digest.
