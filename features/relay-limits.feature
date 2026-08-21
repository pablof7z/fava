Feature: Declared relay limits produce an exact plan or an exact shortfall

  # fava:id=HARD-LIMIT-001
  # fava:status=built
  # fava:evidence=canary:relay-limit-shortfall
  # fava:evidence=rust:fava_subscriptions_standard::a_stricter_relay_subscription_claim_produces_exact_shortfall_not_omission
  # fava:evidence=rust:fava::a_declared_message_limit_refuses_the_query_before_any_connection
  # fava:falsifier=Honor only Fava's configured bound and ignore the relay claim; the over-limit plan is built and the shortfall assertion fails.
  @acceptance @real-relay
  Scenario: A relay's stricter claim refuses the plan instead of omitting work
    Given a relay advertising a strict NIP-11 subscription or message limit
    When an application opens a query whose exact wire plan exceeds it
    Then the open is refused with the exact required and permitted values
    And no relay connection is opened for the refused work
    And the shortfall is a reported diagnostic fact, not a silent omission

  # fava:id=HARD-LIMIT-002
  # fava:status=built
  # fava:evidence=rust:fava::an_unreachable_relay_information_document_leaves_limits_unknown
  # fava:evidence=rust:fava_nip11::an_absent_limitation_block_declares_nothing
  # fava:evidence=rust:fava_nip11::a_non_document_body_is_malformed_rather_than_an_invented_default
  # fava:falsifier=Substitute a default limit for an unreachable or malformed document; the unknown-claim assertion fails and work is refused that the relay never refused.
  @acceptance
  Scenario: A missing or malformed claim stays unknown
    Given a relay whose information document is unreachable, absent, or malformed
    When Fava plans work for that relay
    Then every relay-declared limit remains unknown
    And only Fava's own configured bounds apply
    And the reason the limits are unknown remains an exact reported fact

  # fava:id=HARD-LIMIT-003
  # fava:status=built
  # fava:evidence=rust:fava::a_declared_content_limit_refuses_publication_before_any_connection
  # fava:evidence=rust:fava::an_undeclared_content_limit_refuses_nothing
  # fava:falsifier=Skip the declared-limit check in the publisher; the over-limit event is handed off and the no-connection assertion fails.
  @acceptance
  Scenario: Publication refuses knowingly invalid work before any bytes move
    Given a relay declaring an event content, tag, size, or proof-of-work bound
    When an application publishes an event that exceeds it
    Then the destination terminates with an exact declared-limit refusal
    And no connection is opened for that destination
    And a relay declaring no such bound still receives the attempt
