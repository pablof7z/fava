Feature: Multi-relay reactivity and bounded observation

  # fava:id=QUERY-MULTI-001
  # fava:status=built
  # fava:evidence=canary:multi-relay-dedup-provenance
  # fava:evidence=rust:fava::duplicate_event_merges_only_actual_serving_relays
  # fava:falsifier=Credit every relay named by the Query; the third non-serving relay makes this scenario fail.
  @acceptance @real-relay
  Scenario: Duplicate relay events merge with actual serving provenance
    Given the same signed event exists in two real relays
    And a third queried relay does not contain it
    When one public live Query explicitly asks all three relays
    Then one EventRecord contains the event
    And its relay evidence names exactly the two relays that served it

  # fava:id=QUERY-RECONNECT-001
  # fava:status=built
  # fava:evidence=canary:reconnect-generation
  # fava:evidence=rust:fava::reconnect_uses_fresh_identity_and_rejects_old_subscription_frames
  # fava:falsifier=Accept an EVENT using any known filter instead of the current subscription attribution; the injected old-subscription event enters the cache and this scenario fails.
  @acceptance @real-relay
  Scenario: Reconnect replaces exact request identity
    Given a live Query has one active relay subscription
    When the relay disconnects and returns
    Then Fava sends a fresh REQ without application resubscription
    And an EVENT attributed to the prior subscription cannot affect current state
    And the same EVENT attributed to the current subscription becomes visible
    And EOSE exists only for actual EOSE frames

  # fava:id=OBSERVATION-BOUNDS-001
  # fava:status=built
  # fava:evidence=canary:slow-consumer-latest-state
  # fava:evidence=rust:fava::cancelled_pulls_and_large_burst_deliver_one_exact_latest_state
  # fava:falsifier=Replace the watch boundary with an unbounded update queue; retained work grows with the burst and this scenario fails its bounded-delivery contract.
  @acceptance
  Scenario: A slow observer receives exact current state through a bounded mailbox
    Given an application repeatedly cancels pending change pulls
    And 256 signed events commit while the application does not read
    When the application next reads the Query
    Then one current snapshot contains all 256 events
    And diagnostics report superseded current-state revisions

  # fava:id=OBSERVATION-BOUNDS-002
  # fava:status=built
  # fava:evidence=rust:fava::one_thousand_idle_observations_share_the_current_runtime_thread
  # fava:falsifier=Assign a dedicated operating-system thread per Observation; this scenario no longer remains on one current-thread runtime.
  Scenario: Idle observations do not require one thread each
    Given a current-thread asynchronous runtime
    When 1,000 cache-only observations remain idle simultaneously
    Then all 1,000 remain readable on that one operating-system thread
