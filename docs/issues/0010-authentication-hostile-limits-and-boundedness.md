# M8: authentication, hostile relays, limits, and boundedness

**Status:** in progress
**Authority:** `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, M8
**Depends on:** M3, M6 (implementation plan §5). M7 is not an M8 dependency.

## Product result

Every network and provider boundary produces an exact, isolated, bounded
outcome under relay authentication, hostile wire input, advertised relay
limits, offline destinations, retry ceilings, ambiguous handoff, and provider
failure.

## Slice 1 — explicit generation-scoped NIP-42 (HARD-01, HARD-02)

- `fava-auth` realizes the approved `Authentication` concept. It owns challenge
  correlation, the application policy call, signer selection, the NIP-42 kind
  22242 event, and the session-scoped outcome. It owns no connection.
- `RelayChallenge` binds one bounded challenge string to an exact
  `RelaySessionKey` and transport generation. `Authentication` refuses a
  challenge whose session or generation is not the caller's current one, so a
  retired generation is rejected by identity rather than by timing.
- `AuthenticationPolicy` is the replaceable application decision. It sees only
  relay access, generation, and challenge. It never sees query filters, event
  authorship, the current account, or the signer registry.
- `AuthorizationDecision::Authorize(PublicKey)` names the identity that may hold
  this relay access. Signer registry order never selects it.
- `Query::with_relay_access` and `WriteIntent::with_relay_access` make relay
  authorization identity explicit for reads and writes. `Receipt` carries the
  access its destinations execute under, and `RouteRequest::Write` carries it
  through automatic routing. Explicit-route writes no longer force public access.
- The read path answers a challenge on the generation that carried it, settles
  the relay `OK` in its own reader loop through `Authentication::prepare` and
  `Authentication::settle`, and re-issues the exact accepted plan so a relay
  that closed a subscription with `auth-required:` restores demand without
  application lifecycle code.
- `Nip01Publisher::authenticated` answers a mid-attempt challenge and re-sends
  the event once. A declined, refused, or failed authentication terminates that
  exact destination as `RelayDeliveryOutcome::AuthenticationDenied`.
- Isolation is structural. `RelayAccess` is part of `RelaySessionKey`, so two
  accounts on one relay occupy two sessions. Denying one cannot reach the other.

### Bounds

- Relay challenge text: 1,024 bytes, typed refusal above it.
- Retained relay authentication text: 4,096 bytes, truncated at a character
  boundary rather than silently dropped.
- One complete challenge/response round trip: 10 s.
- Frames read while awaiting the matching `OK`: 64.

### Evidence

- `fava_auth::declining_one_relay_access_leaves_another_account_authenticated`
- `fava_auth::a_challenge_from_a_retired_generation_produces_no_relay_work`
- `fava_auth::the_authenticated_identity_is_the_policy_choice_not_the_signer_registry_order`
- `fava_auth::an_unregistered_authorized_identity_fails_before_handoff`
- `fava_auth::relay_refusal_is_never_reported_as_acceptance`
- `fava_auth::the_answer_frame_is_an_exact_nip42_auth_message`
- `fava::an_authenticated_relay_serves_the_query_after_demand_is_restored`
- `fava::declining_one_account_leaves_the_other_account_publishing`
- `features/relay-authentication.feature`

### Deliberate breaks confirmed

| Break | Failing evidence |
|-------|------------------|
| Compare only the session key, not the generation | `a_challenge_from_a_retired_generation_produces_no_relay_work` |
| Skip re-issuing the accepted plan after acceptance | `an_authenticated_relay_serves_the_query_after_demand_is_restored` |
| Ignore the policy decision inside the publisher | `declining_one_account_leaves_the_other_account_publishing` |

### Vocabulary

`Authentication` moves from specification-only to implemented and gains its
`fava-auth` symbols. Two focused additions record concepts the approved
`Authentication` lifecycle did not name: `AuthorizationDecision` (the prior
application choice, distinct from the signed NIP-42 answer) and `RelayChallenge`
(an opaque NIP-42 string bound to an exact session generation). Both carry their
nearest Nostr concept, observable distinction, owner, and executable falsifier.
They require Pablo's ratification before M8 is claimed complete.

## Not claimed by this issue

- NIP-11 freshness, staleness, negative caching, single-flight, and `FetchCache`
  use remain M9.
- NIP-05 remains M9.
- Native platform authentication evidence remains M11.
