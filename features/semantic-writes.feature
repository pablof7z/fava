Feature: Semantic replaceable-event writes

  # fava:rust=fava/semantic_write_contract#first_value_receives_no_prior_and_exact_timestamp
  Scenario: first value
    Given Alice has no qualified replaceable event at the edit coordinate
    When the selected protocol applier applies Alice's semantic edit at an exact timestamp
    Then it receives no prior source and returns an unsigned event authored by Alice

  # fava:rust=fava/semantic_write_publication#newer_source_reapplies_once_and_preserves_unrelated_fields
  Scenario: source-v2 reapplication
    Given Alice's accepted edit is live over qualified source version one
    When a newer qualified source version two adds an unrelated field
    Then the successor revision preserves both the accepted change and the unrelated field

  # fava:rust=fava/semantic_write_store#memory_generation_swap_is_compare_and_set
  Scenario: stable receipt and new RevisionId
    Given one accepted semantic write has one WriteId and ReceiptId
    When a qualified source successor produces another immutable event
    Then its RevisionId changes while the WriteId and ReceiptId remain unchanged

  # fava:rust=fava/semantic_write_publication#interleavings::retired_completion_is_attributable_and_inert
  Scenario: retired completion
    Given a completion belongs to a retired RevisionId
    When that completion reaches the write store after a successor is current
    Then the completion remains attributable and cannot change current receipt or event state

  # fava:rust=fava/semantic_write_publication#distinct_unsigned_edits_compose_under_one_exact_operation
  Scenario: same-coordinate unsigned composition
    Given Alice has one accepted replaceable edit awaiting signature
    When Alice publishes a distinct edit at the same coordinate
    Then the same write and receipt identify the ordered composed revision
    And the revision generation advances exactly once
    And a signer completion for the retired generation is inert

  # fava:rust=fava/semantic_write_capabilities#nip02_passes_public_semantic_write_corpus
  Scenario: opposing operations
    Given a protocol capability has produced a bounded semantic edit
    When the capability produces and publishes the opposing edit
    Then both edits use the same write lifecycle and restore the intended protocol state

  # fava:rust=external-semantic-capability-proof/public_capability#external_capability_composes_through_public_fava
  Scenario: external N+1
    Given an external capability implements only public semantic-write contracts
    When an application selects that capability in its Fava assembly
    Then current and empty source apply without changing universal core behavior

  # fava:rust=external-semantic-capability-proof/public_capability#raw_future_event_kind_publishes_unchanged
  Scenario: raw future kinds
    Given an application constructs an arbitrary future event kind with the generic builder
    When it publishes the ordinary raw event without a semantic capability
    Then its kind content and tags remain usable without a universal kind switch
