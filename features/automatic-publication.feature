Feature: Automatic write routing makes partial progress

  # fava:id=WRITE-AUTOMATIC-001
  # fava:status=built
  # fava:evidence=rust:fava::known_destinations_deliver_now_and_later_route_uses_same_receipt
  # fava:falsifier=Wait for every unresolved recipient before starting a lane; the first handoffs cannot precede the final relay-list publication.
  @acceptance @real-relay
  Scenario: Known recipient relays deliver before all NIP-65 knowledge settles
    Given an event p-tags three recipients
    And two recipient relay lists and an app relay are immediately known
    And the third relay list is unresolved at an explicit indexer relay
    When Fava accepts the automatically routed write
    Then the known relays receive the event before the third relay list is served
    When the indexer later serves the third recipient relay list
    Then its relay receives the same signed event under the same receipt
    And no existing destination receives a duplicate EVENT

  # fava:id=ROUTER-OUTBOX-001
  # fava:status=built
  # fava:evidence=rust:fava_router_outbox::known_lists_are_immediate_and_missing_list_uses_exact_indexer_query
  # fava:falsifier=Route the missing relay-list Query automatically; router recursion or an unexpected relay appears.
  Scenario: Outbox discovery uses an exact ordinary Query
    Given locally known NIP-65 lists cover some write targets
    And another recipient list is missing
    When the outbox router opens
    Then known author and recipient destinations are immediate
    And one kind 10002 Query asks exactly the configured indexer relays
    And a later relay-list event replaces the unresolved target

  # fava:id=ROUTER-HINT-001
  # fava:status=built
  # fava:evidence=rust:fava_router_hints::reference_hint_and_actual_relay_evidence_are_independent_reasons
  # fava:falsifier=Require outbox knowledge for referenced events; the justified target relay disappears from the plan.
  @acceptance @real-relay
  Scenario: A reference hint and admitted relay evidence route independently
    Given an explicitly queried target event was admitted from one relay
    When an application builds a reply with the generic EventBuilder
    Then the hint router contributes the justified target relay
    And no outbox router is required

  # fava:id=ROUTER-PREVIEW-001
  # fava:status=built
  # fava:evidence=rust:fava::known_destinations_deliver_now_and_later_route_uses_same_receipt
  # fava:falsifier=Open live router acquisition during preview; a receipt, REQ, signer call, or EVENT appears before publish.
  @acceptance @real-relay
  Scenario: Route preview matches the initial real route without side effects
    Given current automatic routing facts
    When the application previews a prospective write
    Then no receipt, signing, store entry, router acquisition, or relay work occurs
    When the application publishes without changing route facts
    Then the receipt's initial desired destinations equal the preview

  # fava:id=ROUTER-PROFILE-001
  # fava:status=built
  # fava:falsifier=Move app-relay or fallback choice into core; the same assembly selection can no longer produce two plans.
  @acceptance @real-relay
  Scenario: Applications independently choose app-relay and fallback policies
    Given one assembly selects an app-relay write policy
    And another selects a fallback write policy
    When both publish the same event
    Then each produces its documented distinct relay plan
    And Fava core is unchanged between profiles
