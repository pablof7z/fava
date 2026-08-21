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

## Slice 2 — declared relay limits reach planning and publication (HARD-04)

- `fava-nip11` owns NIP-11 document values, `RelayLimitation`, bounded parsing
  and validation, and the `RelayInformationFetcher` contract.
- `fava-nip11-http` owns one bounded HTTP exchange and nothing else.
- The declared bounds reach universal owners as protocol-independent values:
  `fava_subscriptions::RelayLimits` for planning and
  `fava_publisher::RelayWriteLimits` for publication. Neither universal owner
  reads a NIP-11 document.
- An absent field means the relay did not say. It never means unlimited and
  never becomes an invented default. An unreachable, absent, or malformed
  document leaves every limit unknown, and why it is unknown stays a reported
  diagnostic fact.
- `SubscriptionPlanner::plan` takes the relay's limits. `enforce_limits` honors
  the stricter of Fava's configured bound and the relay's claim and produces an
  exact typed refusal naming the actual and permitted values. Nothing is
  truncated, clamped, or collided; explicit query opening stays all-or-nothing,
  so an over-limit plan is a refusal rather than a silent omission.
- `Nip01Publisher::with_relay_information` evaluates the locally decidable write
  claims — frame size, content size, tag count, proof-of-work difficulty — and
  returns `RelayDeliveryOutcome::RefusedByLimit` **before opening a connection**,
  so knowingly invalid bytes never move. Claims that depend on relay-side
  authorization (`auth_required`, `restricted_writes`) inform but never refuse.

### Bounds

- Relay information document: 65,536 bytes, typed refusal above it.
- Relay information text field: 4,096 bytes.
- Complete relay information acquisition: 10 s.
- HTTP response including headers: document bound plus 16,384 bytes.
- Parsed HTTP response headers: 64.

### Evidence

- `fava_nip11::an_absent_limitation_block_declares_nothing`
- `fava_nip11::declared_limits_project_into_planning_and_write_bounds`
- `fava_nip11::an_oversized_document_is_refused_with_exact_counts`
- `fava_nip11::a_non_document_body_is_malformed_rather_than_an_invented_default`
- `fava_nip11_http::a_non_success_status_is_a_refusal_rather_than_an_empty_document`
- `fava_subscriptions_standard::an_unknown_relay_claim_leaves_favas_own_bound_in_force`
- `fava_subscriptions_standard::a_stricter_relay_subscription_claim_produces_exact_shortfall_not_omission`
- `fava_subscriptions_standard::a_stricter_relay_message_claim_refuses_the_frame_before_handoff`
- `fava_subscriptions_standard::a_declared_subscription_id_length_refuses_the_identifier_it_cannot_carry`
- `fava_subscriptions_standard::a_declared_filter_limit_refuses_a_larger_requested_bound`
- `fava::a_declared_message_limit_refuses_the_query_before_any_connection`
- `fava::an_unreachable_relay_information_document_leaves_limits_unknown`
- `fava::a_declared_content_limit_refuses_publication_before_any_connection`
- `fava::an_undeclared_content_limit_refuses_nothing`
- `features/relay-limits.feature`

### Deliberate breaks confirmed

| Break | Failing evidence |
|-------|------------------|
| Honor only the configured bound and ignore the relay claim | `a_stricter_relay_subscription_claim_produces_exact_shortfall_not_omission` |
| Skip the declared-limit check inside the publisher | `a_declared_content_limit_refuses_publication_before_any_connection` |

### Vocabulary

`RelayInformationFetcher` moves from specification-only to implemented and gains
its `fava-nip11` and `fava-nip11-http` symbols. One focused addition records the
protocol-independent bound values universal planning and publication consume:
`RelayLimits`, whose distinction from a NIP-11 limitation object is that an
absent field means unknown rather than unlimited. It requires Pablo's
ratification before M8 is claimed complete.

## Not claimed by this issue

- NIP-11 freshness, staleness, negative caching, single-flight, and `FetchCache`
  use remain M9.
- NIP-05 remains M9.
- Native platform authentication evidence remains M11.
