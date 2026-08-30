# 0059: account signer status through the facade

## problem

A downstream account application can mutate signer attachments through `Fava`
but cannot inspect whether an account currently has one or attribute replacement
to its exact generation. Inferring attachment generation from the session wake
revision or retaining a second application signer map would duplicate session
ownership.

## decision

`Fava::signer_status(public_key)` delegates to the existing session snapshot and
returns the exact attachment generation and `SignerAvailability`, or `None` for
a pubkey-only account. `Fava` re-exports `SignerAvailability`; no second status
type or lifecycle is introduced.

## evidence

The facade test proves absent, attached, replaced, and removed states. Replacement
advances the exact generation while preserving the account public key. The
account E2E application consumes this read-only status in diagnostics and retains
only aliases.
