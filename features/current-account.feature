Feature: Session-owned current account

  # fava:rust=fava-session/session#account_set_selection_and_revision_are_atomic_and_bounded
  # fava:falsifier=Retain current selection or signer after removing its account; the exact snapshot and one-revision assertion fail.
  Scenario: Account lifecycle has one session owner
    Given a session contains signer-backed and pubkey-only accounts
    When the current account is selected cleared or removed
    Then the bounded account set and optional current public key change atomically
    And each accepted mutation advances one session revision

  # fava:rust=fava/current_account_publication#accepted_author_does_not_follow_current_account
  # fava:falsifier=Resolve current account after acceptance; switching to B retargets A's accepted write and this scenario fails.
  Scenario: Current-account writes resolve once
    Given account A is current and its signing completion is delayed
    When an authorless write is accepted and account B becomes current
    Then the accepted event and receipt remain attributed to A
    And a later authorless write is attributed to B

  # fava:rust=fava/current_account_publication#missing_current_account_refuses_before_custody
  # fava:falsifier=Create custody before resolving current account; the write-store count changes and this scenario fails.
  Scenario: Missing current account refuses before acceptance
    Given no account is current
    When an authorless payload is published
    Then a typed missing-author refusal creates no write or receipt

  # fava:rust=fava/current_account_observation#one_observation_reroots_when_current_account_changes
  # fava:falsifier=Ignore the session revision; the observation keeps A's result and relay demand after B is selected.
  Scenario: One observation follows current account
    Given one open query depends on $currentPubkey while account A is current
    When account B becomes current
    Then the same observation handle reports B's current result
    And owner-side synchronization cannot return A while B's generation opens
    And relay demand is recompiled from A to B without application reopening

  # fava:rust=fava/current_account_observation#empty_and_rapid_selection_preserve_current_truth
  # fava:falsifier=Apply an A or B completion after C's generation; stale events overwrite C's current snapshot and this scenario fails.
  Scenario: Empty and rapid selection remain exact
    Given account-dependent local and relay completions can be delayed
    When selection clears and then changes A to B to C
    Then empty selection matches nothing and emits no broad relay request
    And only C's exact generation can affect the current snapshot or demand
