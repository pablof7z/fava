Feature: Durable explicit-route publication

  # fava:id=WRITE-EXPLICIT-001
  # fava:status=built
  # fava:evidence=canary:explicit-publish-optimistic
  # fava:evidence=rust:fava::accepted_unsigned_event_is_visible_before_ok_and_cache_waits_for_echo
  # fava:falsifier=Insert the unsigned event into EventCache at acceptance; the cache-separation assertion fails before any relay echo.
  @acceptance @real-relay
  Scenario: An accepted unsigned event is optimistic without polluting relay cache
    Given a durable write store and an exact explicit relay
    When Fava accepts a complete unsigned event
    Then the matching Query shows it with the stable receipt immediately
    And no EVENT crosses the wire before its author signs it
    And EventCache remains empty until the verified relay echo arrives

  # fava:id=WRITE-EXPLICIT-002
  # fava:status=built
  # fava:evidence=canary:mixed-relay-outcomes
  # fava:evidence=rust:fava::mixed_relay_results_remain_exact_under_one_terminal_receipt
  # fava:falsifier=Collapse acknowledged, rejected, and definite pre-handoff failure into one success flag; exact outcome assertions fail.
  @acceptance @real-relay
  Scenario: One receipt retains different exact relay outcomes
    Given one accepting relay, one rejecting relay, and one unreachable relay
    When Fava publishes one verified signed event to that exact relay set
    Then one terminal receipt records acknowledged, rejected, and given-up facts
    And each real relay receives exactly its own publication attempt

  # fava:id=WRITE-CANCEL-001
  # fava:status=built
  # fava:evidence=canary:cancel-pre-handoff
  # fava:evidence=rust:fava::pre_handoff_cancel_retracts_query_and_is_idempotent_and_removable
  # fava:falsifier=Allow signing to continue into transport after cancellation; the wire records an EVENT and the scenario fails.
  @acceptance @real-relay
  Scenario: Cancellation before handoff retracts without publication
    Given an accepted event is waiting for its signer
    When the application cancels its receipt twice
    Then the local Query retracts the event
    And no EVENT crosses the wire
    And separate receipt removal succeeds after terminal cancellation

  # fava:id=WRITE-SIGNER-001
  # fava:status=built
  # fava:evidence=rust:fava::unsigned_write_without_its_author_signer_remains_inspectable
  # fava:falsifier=Discard an accepted unsigned write when its exact author signer is absent; open receipt inspection no longer finds it.
  Scenario: A missing author signer parks an inspectable accepted write
    Given an unsigned event has been durably accepted
    And no configured signer owns its author public key
    When the application inspects current open receipts
    Then the same receipt and unsigned event remain present
    And no signer for another public key may satisfy it

  # fava:id=WRITE-BOUNDS-001
  # fava:status=built
  # fava:evidence=rust:fava::explicit_write_relay_fanout_is_bounded_before_custody
  # fava:evidence=rust:fava::receipt_text_and_signed_body_are_checked_at_the_store_boundary
  # fava:falsifier=Accept 257 explicit relays or 4097 bytes of receipt text; durable work exceeds its declared bound.
  Scenario: External publication inputs are bounded before durable growth
    Given an explicit write or provider result exceeds a declared bound
    When Fava validates custody or durable receipt mutation
    Then the whole operation is refused without partial mutation
