Feature: Explicit, generation-scoped, isolated relay authentication

  # fava:id=HARD-AUTH-001
  # fava:status=built
  # fava:evidence=canary:nip42-write-and-reconnect
  # fava:evidence=rust:fava::an_authenticated_relay_serves_the_query_after_demand_is_restored
  # fava:evidence=rust:fava_auth::the_answer_frame_is_an_exact_nip42_auth_message
  # fava:falsifier=Skip restoring the accepted plan after acceptance; the authenticated read never receives its stored event.
  @acceptance @real-relay
  Scenario: A relay challenge is answered and active demand is restored
    Given a relay that serves only authenticated sessions
    And an application policy authorizing one exact relay access
    When Fava opens a live query under that relay access
    Then Fava answers the challenge with a kind 22242 event naming the exact relay and challenge
    And the relay reports the exact authenticated identity
    And the query receives its stored events without application lifecycle code

  # fava:id=HARD-AUTH-002
  # fava:status=built
  # fava:evidence=canary:auth-account-isolation
  # fava:evidence=rust:fava::declining_one_account_leaves_the_other_account_publishing
  # fava:evidence=rust:fava_auth::declining_one_relay_access_leaves_another_account_authenticated
  # fava:falsifier=Ignore the policy decision in the publisher; the declined account's destination is acknowledged and the isolation assertion fails.
  @acceptance @real-relay
  Scenario: Denying one account's authentication policy leaves the other account working
    Given two accounts publishing to one relay under two exact relay accesses
    When the application declines authentication for one relay access
    Then that destination terminates with an exact auth-denied outcome
    And the other account's destination is still acknowledged by the relay
    And only the authorized identity ever authenticated

  # fava:id=HARD-AUTH-003
  # fava:status=built
  # fava:evidence=rust:fava_auth::a_challenge_from_a_retired_generation_produces_no_relay_work
  # fava:falsifier=Compare only the relay session key and not the generation; the retired-generation challenge is answered and the no-handoff assertion fails.
  @acceptance
  Scenario: A challenge from a retired session generation cannot authenticate current work
    Given a relay challenge captured under an earlier transport generation
    When the current session generation is newer
    Then Fava refuses the challenge by identity and hands off no AUTH frame

  # fava:id=HARD-AUTH-004
  # fava:status=built
  # fava:evidence=rust:fava_auth::the_authenticated_identity_is_the_policy_choice_not_the_signer_registry_order
  # fava:evidence=rust:fava_auth::relay_refusal_is_never_reported_as_acceptance
  # fava:falsifier=Select the first registered signer instead of the policy identity; the authenticated pubkey assertion fails.
  @acceptance
  Scenario: Relay authorization identity is separate from authorship and signer order
    Given several registered signers
    When the policy authorizes one exact identity for a relay access
    Then Fava signs the challenge answer with that identity only
    And a relay refusal is never reported as acceptance
