# id: LAB-REAL-RELAY-001
# requirement: M0 evidence prerequisite
# status: needs re-proof — evidence lived entirely in the downstream acceptance
#   application, which was removed from the repository on 2026-08-31
# evidence: none currently in-repo; requires a real third-party relay process
#   fixture (previously provided by the removed downstream acceptance application)
# falsifier: restart generation two with a fresh data directory; the post-restart query reaches EOSE without the event and the scenario fails

Feature: Relay behavior is witnessed independently of Fava

  Rule: A real relay process proves its own persistence through public wire behavior

    Scenario: A signed event remains queryable after relay process death
      Given a fresh third-party relay process with isolated durable storage
      When the fixture publishes a genuinely signed event through a transparent proxy
      Then the relay acknowledges that exact event
      And an exact query returns the event followed by EOSE
      When the relay is hard-killed and restarted with the same storage
      Then another exact query returns the same event followed by EOSE
      And the run preserves process, wire, and application evidence

    Scenario: Fresh storage does not pretend to prove persistence
      Given the first relay process stored a signed event
      When the restarted relay uses a different fresh storage directory
      Then the exact query reaches EOSE without returning that event
      And the persistence scenario fails
