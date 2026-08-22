Feature: Typed NIP-02 contact lists preserve every contact row

  # fava:rust=fava-nip02/fava_nip02#valid_empty_and_ordered_contact_lists_decode
  Scenario: empty and ordered contact lists
    Given an application receives valid kind 3 events with zero, one, or several contact rows
    When it decodes each event as a contact list
    Then valid follows retain their source order and the list may be empty

  # fava:rust=fava-nip02/fava_nip02#nip02_accounts_for_every_p_row
  Scenario: no contact row disappears
    Given one kind 3 event mixes valid, malformed, duplicate, short, and extra-column contact rows
    When the application decodes the event once
    Then every contact row appears exactly once as a valid follow or exact typed row evidence

  # fava:rust=fava-nip02/fava_nip02#invalid_contact_rows_do_not_reserve_duplicate_targets
  Scenario: invalid rows do not poison later valid contacts
    Given an invalid relay hint precedes a valid row and another valid duplicate for the same public key
    When the application decodes the contact list
    Then the middle row is the valid follow and only the later valid row is duplicate evidence

  # fava:rust=fava-nip02/fava_nip02#petname_presence_and_utf8_remain_exact
  Scenario: petname presence and bytes remain exact
    Given contact rows contain absent, present-empty, and decomposed UTF-8 petnames
    When the application reads typed follows
    Then absence remains distinct from an empty value and no petname is normalized

  # fava:rust=fava-nip02/fava_nip02#invalid_contact_list_events_are_refused_before_rows
  Scenario: invalid event boundaries are refused
    Given an event is the wrong kind, lacks an id, has an invalid signature, or exceeds a declared bound
    When the application tries to decode it as a contact list
    Then the whole event is refused before any row result is returned
