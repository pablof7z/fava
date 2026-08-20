Feature: Ordered asynchronous routing

  # fava:id=ROUTING-ASYNC-001
  # fava:status=built
  # fava:evidence=canary:async-route-partial-read
  # fava:evidence=rust:fava::immediate_route_starts_before_delayed_router_and_preview_opens_nothing
  # fava:falsifier=Await a later contribution before using the immediate plan; Query open exceeds its deadline before the delayed router changes.
  @acceptance @real-relay
  Scenario: Known routes start before later routing knowledge settles
    Given an app-relay router immediately contributes one real relay
    And a later router currently contributes no relay
    When an automatic live Query opens
    Then the proxy witnesses REQ to the app relay immediately
    And the later relay has no work
    When the later router contributes a second real relay
    Then the same Query receives events from the second relay
    And the first relay session remains uninterrupted

  # fava:id=ROUTING-EXPLICIT-001
  # fava:status=built
  # fava:evidence=canary:explicit-route-bypass
  # fava:evidence=rust:fava::explicit_query_bypasses_every_automatic_router
  # fava:falsifier=Open the configured router chain for an explicit Query; router open counts and diagnostics become nonzero.
  @acceptance @real-relay
  Scenario: Explicit relay selection bypasses automatic routing
    Given an automatic router is configured
    When a live Query explicitly names one relay
    Then the exact relay receives query work
    And no automatic router session opens

  # fava:id=ROUTING-FALLBACK-001
  # fava:status=built
  # fava:evidence=canary:fallback-reacts
  # fava:evidence=rust:fava::fallback_retracts_when_upstream_coverage_arrives_without_restarting_other_relays
  # fava:falsifier=Freeze fallback at its initial contribution; its relay never receives CLOSE after upstream coverage becomes adequate.
  @acceptance @real-relay
  Scenario: Fallback reacts to the current upstream plan
    Given upstream coverage is below the fallback policy minimum
    When an automatic Query opens
    Then fallback relay work begins
    When a prior router supplies adequate coverage
    Then fallback relay work receives CLOSE
    And unrelated relay work remains live

  # fava:id=ROUTING-PREVIEW-001
  # fava:status=built
  # fava:evidence=rust:fava::immediate_route_starts_before_delayed_router_and_preview_opens_nothing
  # fava:falsifier=Implement preview by opening live routing; the router and transport open counts become nonzero.
  Scenario: Route preview has no ownership side effects
    Given configured routers have current known contributions
    When the application previews routes
    Then the preview uses the ordered current derivation
    And no router session or relay session opens

  # fava:id=ROUTING-ATTRIBUTION-001
  # fava:status=built
  # fava:evidence=rust:fava::identical_relay_contributions_deduplicate_and_retain_both_reasons
  # fava:falsifier=Replace the destination map entry on duplicate relay identity; one router reason disappears.
  Scenario: Identical relay destinations retain all contribution reasons
    Given two routers contribute the same relay for the same target
    When the current route plan is merged
    Then the plan contains one relay destination
    And it retains both router reasons
