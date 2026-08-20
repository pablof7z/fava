Feature: Local event state is one coherent view

  # fava:id=QUERY-LOCAL-001
  # fava:status=built
  # fava:evidence=rust:fava::accepted_local_event_is_visible_without_cache_pollution
  # fava:falsifier=Ignore WriteStore source contributions; the accepted event disappears and this scenario fails.
  @acceptance
  Scenario: An accepted local event appears without entering the relay cache
    Given an empty event cache and write store
    When an unsigned local event is accepted
    Then a matching query shows that local event with publication evidence
    And the event cache still contains no event

  # fava:id=QUERY-LOCAL-002
  # fava:status=built
  # fava:evidence=rust:fava::relay_echo_enriches_one_record_without_erasing_receipt
  # fava:falsifier=Discard relay evidence while merging a cached and local event id; this scenario fails.
  @acceptance
  Scenario: A relay echo enriches one local event record
    Given a signed local event with a receipt
    When two relays serve that exact event
    Then a matching query shows one event record
    And that record keeps the receipt and names both relays

  # fava:id=QUERY-LOCAL-003
  # fava:status=built
  # fava:evidence=rust:fava::cancelling_local_replacement_reveals_cached_predecessor
  # fava:falsifier=Retain the older candidate when a local replacement is newer; this scenario fails.
  @acceptance
  Scenario: Cancelling a local replacement reveals the cached predecessor
    Given a cached replaceable event
    And a newer local replacement at the same coordinate
    When the local replacement is cancelled
    Then the same open query shows the cached predecessor
    And no compensating event-cache write occurs

  # fava:id=QUERY-SOURCE-001
  # fava:status=built
  # fava:evidence=rust:fava::acquisition_only_and_provenance_constraint_stay_distinct
  # fava:falsifier=Treat from_relays as a result-provenance constraint; the unrelated local event disappears and this scenario fails.
  @acceptance
  Scenario: Explicit acquisition does not become a provenance constraint
    Given a local event known only from another relay
    When a query asks an exact different relay set
    Then the local matching event remains visible

  # fava:id=QUERY-SOURCE-002
  # fava:status=built
  # fava:evidence=rust:fava::acquisition_only_and_provenance_constraint_stay_distinct
  # fava:falsifier=Ignore OnlyRelays result authority; the unrelated local event appears and this scenario fails.
  @acceptance
  Scenario: Provenance-constrained acquisition excludes unrelated local evidence
    Given a local event known only from another relay
    When a query asks and trusts only an exact relay set
    Then that event is absent until one of those relays actually serves it

  # fava:id=QUERY-OPEN-001
  # fava:status=built
  # fava:evidence=rust:fava-observe::second_source_open_failure_closes_the_first_source
  # fava:evidence=rust:fava-observe::initial_evaluation_failure_closes_both_sources
  # fava:falsifier=Return an open error without explicitly closing provisional source observations; this scenario fails.
  Scenario: A failed query open leaves no provisional local demand
    Given one or both local sources have opened provisionally
    When another source or initial evaluation refuses the query
    Then opening returns a typed refusal
    And every provisional source observation is closed

  # fava:id=QUERY-SOURCE-003
  # fava:status=built
  # fava:evidence=rust:fava-observe::post_open_source_closure_is_scoped_evidence
  # fava:falsifier=Close the whole query when one local source terminates; this scenario fails.
  Scenario: A local source termination remains scoped after open
    Given a query has a coherent initial view from independent local sources
    When one local source observation terminates
    Then the query retains the last coherent source state
    And its next snapshot reports that source as closed

  # fava:id=QUERY-DELIVERY-001
  # fava:status=built
  # fava:evidence=rust:fava::slow_consumer_receives_exact_latest_state_with_bounded_delivery
  # fava:falsifier=Deliver queued intermediate snapshots instead of the coalesced latest state; the first slow-consumer update is incomplete and this scenario fails.
  @acceptance
  Scenario: A slow consumer receives exact bounded latest state
    Given an open local query whose application is not polling
    When three accepted local events change its result
    Then the next delivered snapshot contains all three current events
    And no unbounded intermediate queue is required

  # fava:id=QUERY-IDENTITY-001
  # fava:status=built
  # fava:evidence=rust:fava-query::equivalent_relay_construction_has_one_query_identity
  # fava:falsifier=Preserve relay insertion order in Query equality or hashing; this scenario fails.
  Scenario: Equivalent query construction has one identity
    Given two queries contain the same explicit relays in different construction order
    When their public values and hashes are compared
    Then the queries are equal
    And their hashes are equal

  # fava:id=EVENT-STATE-001
  # fava:status=built
  # fava:evidence=rust:fava-state::authorized_deletion_retracts_and_prevents_resurrection
  # fava:evidence=rust:fava-state::another_author_cannot_delete_an_event
  # fava:evidence=rust:fava::deletion_and_expiration_update_the_same_open_query
  # fava:falsifier=Apply deletion requests without author validation; another author can retract the event and this scenario fails.
  @acceptance
  Scenario: Deletion and expiration retract events from an open query
    Given admitted signed events are visible in one open local query
    When an authorized deletion request is admitted
    And another event reaches its expiration timestamp
    Then both events disappear through ordinary query state changes
    And an unauthorized deletion request retracts nothing

  # fava:id=PROVIDER-SOURCE-001
  # fava:status=built
  # fava:evidence=rust:fava::memory_event_cache_runs_the_source_corpus
  # fava:evidence=rust:fava::memory_write_store_runs_the_source_corpus
  # fava:falsifier=Stop either memory provider from emitting removals; its shared corpus run fails.
  Scenario: Local source behavior is provider-independent
    Given the same add and remove corpus
    When it runs against the memory event cache and memory write store
    Then both providers expose the same source revisions

  # fava:id=EVENT-CACHE-001
  # fava:status=built
  # fava:evidence=rust:fava-event-cache-memory::invalid_signed_event_is_refused_without_mutation
  # fava:falsifier=Accept an upsert without verifying its event ID and signature; the cache retains a tampered event and this scenario fails.
  Scenario: The event cache refuses invalid signed events
    Given a signed event body was changed after signing
    When a provider mutation tries to retain it
    Then the entire mutation is refused
    And the event cache remains unchanged
