# Runtime signer lifecycle

**Status:** resolved
**Approved by:** Pablo, 2026-08-23
**Authority:** WRITE-003, WRITE-007, WRITE-008, ID-001, ID-005, ID-006;
`docs/spec/ARCHITECTURE.md` `fava-session`, `fava-publication`, and authority ledger

## Problem

Fava currently copies builder-supplied signers into an immutable
`fava-publication` map. An application cannot add, replace, or remove a signer
after `Fava::build`, and a write parked for a missing signer cannot wake when
that signer later becomes available. Rebuilding the engine would break the
specified separation between session state and durable accepted-write state.

## Approved scope

- Implement the already-specified `fava-session` runtime owner.
- Expose runtime add, explicit replace, and remove operations through the
  public `Fava` facade for exactly one attached signer per public key.
- Make `fava-publication` consume current session signer state rather than own
  a frozen signer map.
- Wake only parked writes whose exact event pubkey gains an available signer.
- Cancel or detach replaced/removed signer operations and reject every stale
  completion by exact current operation and materialization generation.
- Preserve explicit, inspectable awaiting-signer state without spending a
  delivery attempt or abandoning by elapsed time.
- Keep signer provider execution outside session/publication locks and store
  transactions.
- Bound the attached-signer set and return typed refusal at capacity.

## Explicitly out of scope

- `fava.by(signer)` convenience; `by(...)` remains the existing author scope.
- Multiple simultaneous signers or fallback policy for one pubkey.
- Full current-account convenience, account removal, and session import/export.
- NIP-46, hardware, extension, or platform SDK implementations.
- Replacing the signer contract with raw private-key input.

## Approved vocabulary

### `Session`

- **Closest existing concept:** Nostr event author public key plus an
  application signer provider.
- **Observable distinction:** `Session` is the Fava-owned runtime authority for
  which signer provider is currently attached to each exact pubkey; a `Signer`
  only reports and executes its own cryptographic capability.
- **Counterexample:** a durable Alice write accepted without a signer must begin
  signing when Alice's signer is added to the same running Fava instance;
  publication's current immutable signer map cannot produce that transition.
- **Owner and lifecycle:** `fava-session`; created with the Fava instance,
  mutated by bounded runtime facade operations, observed by publication/auth
  owners, and closed during Fava shutdown. Accepted writes remain owned by the
  selected `WriteStore` and survive signer removal.
- **Forcing requirement:** WRITE-007/008 and ID-001/005/006 require exact
  runtime signer attachment, missing-signer parking, and session/write
  separation.
- **Why existing state is insufficient:** `Signer` owns one provider operation;
  `FavaBuilder` owns only construction input; `Publication` must not own mutable
  account/provider registration state.
- **Executable falsifier:** accept an unsigned Alice write with no signer, add
  Alice's signer without rebuilding Fava, and require the same write/receipt to
  sign and continue. Deliberately suppressing the session change wakeup must
  fail the test.

Approved public nominal symbols:

```text
fava_session::Session
fava_session::SessionError
```

Approved crate:

```text
fava-session
```

No signer-registration wrapper, provider collection, account alias, options
bag, compatibility path, or second lifecycle owner is approved.

## Implemented result

`fava-session` now owns one bounded runtime signer attachment per exact public
key. `Fava::add_signer`, `Fava::replace_signer`, and `Fava::remove_signer`
mutate the running instance. Publication observes that session, wakes only the
matching parked pubkey, and rejects completions from stale attachment
generations before they can change signing or delivery state. The attachment
bound is 64 and every overflow, duplicate, or missing-target refusal is atomic
and typed.

## Required public behavior

```rust
let fava = Fava::builder()
    // selected cache/store/router/publisher/delivery providers
    .build()?;

let write = fava.publish(unsigned_for_alice)?;
assert!(write.receipt()?.is_awaiting_signer());

fava.add_signer(alice_signer)?;
let receipt = write.settled(fava::all_terminal()).await?;
```

Adding a second signer for the same pubkey refuses without mutation. Replacement
is explicit. Removal leaves accepted writes and receipts intact; matching work
returns to awaiting-signer state. Adding Bob's signer cannot wake Alice's work.

## Validation and falsifiers

- Public facade evidence for add-after-acceptance, explicit replacement,
  removal/re-add, exact-pubkey isolation, and recovery.
- A blocked old signer completion released after replacement remains inert.
- A deliberate missing wakeup fails add-after-acceptance evidence.
- A deliberate pubkey-agnostic wakeup fails Alice/Bob isolation evidence.
- Signer capacity overflow returns a typed refusal with no partial mutation.
- `python3 -m unittest tools.tests.test_vocabulary_check`
- `git diff --check`
