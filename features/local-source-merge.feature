Feature: Local event state is one coherent view

  # nmp:id=QUERY-LOCAL-001
  # nmp:status=specified
  # nmp:gap=implementation
  # nmp:issue=#1
  Scenario: An accepted local event appears without entering the relay cache
    Given an empty event cache and write store
    When an unsigned local event is accepted
    Then a matching query shows that local event with publication evidence
    And the event cache still contains no event

  # nmp:id=QUERY-LOCAL-002
  # nmp:status=specified
  # nmp:gap=implementation
  # nmp:issue=#1
  Scenario: A relay echo enriches one local event record
    Given a signed local event with a receipt
    When two relays serve that exact event
    Then a matching query shows one event record
    And that record keeps the receipt and names both relays

  # nmp:id=QUERY-LOCAL-003
  # nmp:status=specified
  # nmp:gap=implementation
  # nmp:issue=#1
  Scenario: Cancelling a local replacement reveals the cached predecessor
    Given a cached replaceable event
    And a newer local replacement at the same coordinate
    When the local replacement is cancelled
    Then the same open query shows the cached predecessor
    And no compensating event-cache write occurs

  # nmp:id=QUERY-SOURCE-001
  # nmp:status=specified
  # nmp:gap=implementation
  # nmp:issue=#1
  Scenario: Explicit acquisition does not become a provenance constraint
    Given a local event known only from another relay
    When a query asks an exact different relay set
    Then the local matching event remains visible

  # nmp:id=QUERY-SOURCE-002
  # nmp:status=specified
  # nmp:gap=implementation
  # nmp:issue=#1
  Scenario: Provenance-constrained acquisition excludes unrelated local evidence
    Given a local event known only from another relay
    When a query asks and trusts only an exact relay set
    Then that event is absent until one of those relays actually serves it

