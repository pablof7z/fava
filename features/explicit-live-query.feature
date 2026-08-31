Feature: Explicit one-relay live query

  # fava:id=QUERY-LIVE-001
  # fava:status=built
  # fava:evidence=rust:fava::explicit_live_query_attributes_event_eose_and_exact_cancellation
  # fava:falsifier=Treat silence or a local timeout as EOSE; the diagnostics and proxy no longer agree and this scenario fails.
  @acceptance @real-relay
  Scenario: Stored events and EOSE retain exact relay and subscription identity
    Given a signed event stored by an independent seeder in one real relay
    When a public live Query asks exactly that relay
    Then one EventRecord carries evidence for the relay that served it
    And EOSE exists only after the proxy witnesses EOSE for the same subscription
    And no global completeness fact exists

  # fava:id=QUERY-LIVE-002
  # fava:status=built
  # fava:falsifier=Terminate relay demand at EOSE; the later matching event never reaches the open Query and this scenario fails.
  @acceptance @real-relay
  Scenario: A matching live event arrives after EOSE
    Given an explicit live Query has received a stored event and exact EOSE
    When an independent publisher sends another matching event
    Then the same open Query contains both events

  # fava:id=QUERY-LIVE-003
  # fava:status=built
  # fava:evidence=rust:fava::explicit_live_query_attributes_event_eose_and_exact_cancellation
  # fava:falsifier=Drop the cancellation branch before sending CLOSE; the proxy never witnesses exact withdrawal and this scenario fails.
  @acceptance @real-relay
  Scenario: Closing a live Query withdraws exact relay work
    Given the proxy witnessed REQ for one public Query
    When the application closes that Query twice
    Then the proxy witnesses CLOSE for the same subscription and connection
    And pending pulls wake as closed
    And a later matching publication produces no application update

  # fava:id=INGEST-001
  # fava:status=built
  # fava:evidence=rust:fava-ingest::forged_wrong_subscription_and_off_filter_events_never_enter_the_cache
  # fava:falsifier=Bypass event verification throughout relay admission; the hostile witness makes the forged event visible and the scenario fails.
  @acceptance
  Scenario: Only attributed verified matching relay events enter query state
    Given a relay sends a forged, off-filter, or wrong-subscription EVENT
    When relay admission evaluates the frame
    Then the event does not enter the EventCache
    And the event does not enter any Query result

  # fava:id=QUERY-EVIDENCE-001
  # fava:status=built
  # fava:evidence=rust:fava::silence_eose_auth_closed_and_disconnect_are_distinct_facts
  # fava:evidence=rust:fava-diagnostics::eose_closed_auth_failure_and_withdrawal_remain_distinct_and_bounded
  # fava:falsifier=Store EOSE, CLOSED, AUTH, and disconnect in one terminal flag; this scenario fails.
  Scenario: Relay source facts remain distinct
    Given an exact live relay subscription
    Then silence has no EOSE, CLOSED, AUTH, or failure fact
    And actual EOSE, CLOSED, AUTH, disconnect, and local withdrawal occupy distinct bounded facts

  # fava:id=TRANSPORT-001
  # fava:status=built
  # fava:evidence=rust:fava-transport-websocket::conformance
  # fava:falsifier=Report an oversized rejected frame as handed off; the conformance corpus fails.
  Scenario: WebSocket transport preserves handoff and close facts
    Given provider-arranged successful, oversized, disconnected, and closed sessions
    When the shared transport conformance assertions run
    Then success, definite refusal, disconnect, and idempotent close remain distinct
