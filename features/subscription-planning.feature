Feature: Subscription planning preserves query meaning

  # fava:id=SUBSCRIPTION-GROUPING-001
  # fava:status=built
  # fava:evidence=rust:fava-subscriptions-standard::compatible_author_filters_group_with_exact_logical_attribution
  # fava:falsifier=Discard wire-to-logical attribution after grouping; logical query results cannot be reconstructed exactly.
  @acceptance @real-relay
  Scenario: Grouping changes wire shape without changing logical results
    Given three compatible author queries target one real relay
    When the standard planner groups them
    Then the proxy witnesses one REQ instead of three
    And every logical query receives exactly the same event ids as no grouping

  # fava:id=SUBSCRIPTION-SHORTFALL-001
  # fava:status=built
  # fava:evidence=rust:fava-subscriptions-standard::relay_subscription_bound_returns_exact_shortfall
  # fava:falsifier=Drop demand beyond the relay subscription limit; planning claims success with missing logical work.
  Scenario: Relay subscription limits never silently drop demand
    Given exact demand requires two incompatible subscriptions
    And the relay limit allows one
    When subscription planning runs
    Then planning returns the required and maximum counts as exact shortfall
