Feature: Universal synchronous publication door

  # fava:rust=fava/publication_door#publish_payload_forms_share_one_door_and_unscoped_edit_refuses
  Scenario: one payload door
    Given an application has unsigned, pre-signed, and replaceable-edit payloads
    When it supplies each payload directly to Fava publish without constructing a write intent
    Then raw payloads return writes and the unscoped edit receives a typed missing-author refusal before custody

  # fava:rust=fava/publication_door#publish_returns_after_local_acceptance
  Scenario: local acceptance precedes downstream progress
    Given signing and every later publication step are unable to advance
    When an application publishes one unsigned event
    Then publish returns a write whose accepted event is already query-visible

  # fava:rust=fava/publication_door#equivalent_publications_have_distinct_custody_identities
  Scenario: equivalent publication identities
    Given two publications contain the same finalized event
    When both enter the universal publication door
    Then they have distinct write and receipt identities while one semantic event remains query-visible

  # fava:rust=fava/publication_door#invalid_payload_refuses_without_custody
  Scenario: invalid payload has no partial acceptance
    Given an unsigned event has an already-expired timestamp
    When the application tries to publish it
    Then the typed refusal creates no write receipt or local query contribution
