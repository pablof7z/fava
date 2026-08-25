# Fava simple-groups vocabulary approval candidates

**Status:** unsigned review material; not authority and not approval

Each block below is exact canonical markdown currently rendered by
`tools/vocabulary_approval.py` from `vocabulary.toml`. The SHA-256 binds that
byte-exact unsigned candidate. `approvals.jsonl` is unchanged; no signature or
vocabulary approval is claimed here. Rejected `RelaySequence` identities are
absent, and the `SimpleGroup` candidate reflects Pablo's approved issue-0023
construction and issue-0024 query-composition architecture without treating
either decision as vocabulary approval.

## ContactList

Canonical SHA-256: `8e81fe16d086538e827b0903b6ad610bfc8b550daade6362104d9114897c1897`

```markdown
# ContactList

**source**: nostr

**protocol**: NIP-02

**owner**: fava-nip02

**nearest_nostr**: NIP-02 contact list

**meaning**: A replaceable kind 3 event describing an author's contacts and relay hints.

**distinction**: The protocol event is raw; fava_nip02::ContactList is the immutable validated projection that exposes valid entries and complete typed entry-local errors without owning query or publication work.

**counterexample**: An arbitrary kind 3 tag walk that drops malformed p entries is not a ContactList projection, and a QuerySnapshot remains query-owned rather than protocol-owned.

**lifecycle**: Callers construct the value from one EventValue; it owns immutable decoded fields and is dropped without state, acquisition, observation, or publication effects.

**forcing_requirement**: NIP-02 applications must read one author-scoped list without raw tag walking while retaining every uninterpreted entry and refusing invalid event boundaries.

**falsifier**: The contact_list tests must refuse wrong-kind or invalid events and must fail if valid empty kind-3 events do not decode.

**symbols**:
- fava_nip02::ContactList

**crates**:
- fava-nip02

**spec_crates**:
- fava-nip02
```

## Follow

Canonical SHA-256: `423b2f48125250c39c08c2f6c2dcc342d3793fdf8084d3037e9cccafcebd4350`

```markdown
# Follow

**source**: fava

**owner**: fava-nip02

**nearest_nostr**: NIP-02 p tag

**meaning**: One fully valid first-occurrence NIP-02 p entry with its source index, public key, optional relay hint, and optional petname.

**distinction**: A p tag is an untyped ordered string entry; Follow exists only after every understood value validates and preserves absent versus present-empty petname meaning.

**counterexample**: A malformed relay entry or later valid duplicate is ContactListEntryError, not a Follow, and cannot reserve a target before validation completes.

**lifecycle**: ContactList::from_event creates immutable Follow values; they borrow no event storage and disappear with their owning ContactList.

**forcing_requirement**: Applications need ordered typed public keys, relay hints, and exact petnames without duplicating NIP-02 parsing or accepting partially valid entries.

**falsifier**: The invalid_contact_entries_do_not_reserve_duplicate_targets test must fail if an invalid earlier entry becomes a Follow or poisons the later valid entry.

**symbols**:
- fava_nip02::Follow
```

## ContactListEntryError

Canonical SHA-256: `db0da76182c0cf6fff9fb2babe8932668744b85b512aa98ef3999fe5626ccf0a`

```markdown
# ContactListEntryError

**source**: fava

**owner**: fava-nip02

**nearest_nostr**: NIP-02 p tag

**meaning**: A typed entry-local refusal that retains one malformed, duplicate, or uninterpreted NIP-02 p tag exactly.

**distinction**: ContactListError refuses an invalid whole event; ContactListEntryError instead keeps the exact tag and source index for one entry that cannot become a Follow without discarding or inventing meaning.

**counterexample**: A parser warning without source index and exact raw tag values cannot conserve the entry and is not ContactListEntryError.

**lifecycle**: ContactList::from_event creates one immutable error value for each non-valid p entry; the ContactList owns it until dropped.

**forcing_requirement**: Shared kind-3 documents may contain malformed, duplicate, or future entries, and applications must not silently lose data another client may understand.

**falsifier**: The nip02_accounts_for_every_p_entry test must fail when parsing uses filter_map to retain only valid entries.

**symbols**:
- fava_nip02::ContactListEntryError
```

## ContactListError

Canonical SHA-256: `43d516c784161667e9b20e8ad3cb59fdd35528a3689032ba247aead33a8cb925`

```markdown
# ContactListError

**source**: fava

**owner**: fava-nip02

**nearest_nostr**: NIP-02 kind 3 event

**meaning**: A typed refusal for an invalid event-level boundary before NIP-02 entry decoding begins.

**distinction**: ContactListEntryError represents one valid document's uninterpretable contact entry; ContactListError instead refuses a wrong-kind, unfinalized, unverifiable, or over-bound whole event.

**counterexample**: An invalid relay hint inside an otherwise valid kind 3 is ContactListEntryError and must not become ContactListError.

**lifecycle**: ContactList::from_event returns the value synchronously and retains no failure state after the caller drops it.

**forcing_requirement**: Invalid event identity, signature, kind, or bounds must be rejected once before any typed entry result can be trusted.

**falsifier**: The invalid_contact_list_events_are_refused_before_entries test must fail if wrong-kind, missing-id, tampered, or over-bound events produce a ContactList.

**symbols**:
- fava_nip02::ContactListError
```

## SimpleGroup

Canonical SHA-256: `538d74f1519f47979ae2b8637ae8285fa7dc1a7471316d115b3a839812103328`

```markdown
# SimpleGroup

**source**: nostr

**protocol**: NIP-29

**owner**: fava-simple-groups

**nearest_nostr**: NIP-29 relay-based group identified by an h or d tag value

**meaning**: One opaque simple-group id plus a normalized non-empty sequence of application-selected relays.

**distinction**: A raw id or relay URL alone cannot lower both exact group selection and the complete caller-selected relay sequence.

**counterexample**: A globally canonical group id or a value that silently chooses one relay is not this SimpleGroup.

**lifecycle**: Constructed synchronously from one required parsed RelayUrl plus a finite owned Vec tail; immutable thereafter and owns no query or publication work.

**forcing_requirement**: Applications must reuse one exact id and non-empty relay selection across content queries, state queries, preparation, and explicit publication without a shared numeric bound or new relay owner.

**falsifier**: Compile-fail evidence rejects empty and arbitrary-iterator construction; runtime tests fail if first-occurrence relay order changes, content-query composition broadens an existing h axis, or query/write lowering omits the id or any selected relay.

**symbols**:
- fava_simple_groups::SimpleGroup

**crates**:
- fava-simple-groups
```

## SimpleGroupStateEventKind

Canonical SHA-256: `a6112fd47de0eda44ea4e34a903995cf11fb0a921b3d3bfc7a3819d552f816db`

```markdown
# SimpleGroupStateEventKind

**source**: nostr

**protocol**: NIP-29

**owner**: fava-simple-groups

**nearest_nostr**: Relay-generated group state kinds 39000 through 39005

**meaning**: A closed selector for one relay-generated simple-group state-event kind.

**distinction**: It names the exact six-family domain without owning an observation or arbitrary kind policy.

**counterexample**: An arbitrary event Kind or content kind selected through h is not a SimpleGroupStateEventKind.

**lifecycle**: An inert copyable query input consumed synchronously by state_events.

**forcing_requirement**: Callers need a type-safe subset, including the query owner's match-nothing empty set, without copying raw NIP-29 kind numbers.

**falsifier**: The ALL and state-event query tests fail if any kind is missing, duplicated, reordered, or lowered to another number.

**symbols**:
- fava_simple_groups::SimpleGroupStateEventKind
```

## SimpleGroupMetadata

Canonical SHA-256: `92db54b296e541ffecef4a4725ab679647ccf87adcc35873a2a0eab4b61f39cf`

```markdown
# SimpleGroupMetadata

**source**: nostr

**protocol**: NIP-29

**owner**: fava-simple-groups

**nearest_nostr**: Kind-39000 group metadata event

**meaning**: The semantic metadata fields decoded from one kind-39000 event.

**distinction**: It retains first singleton values, presence flags, value-local supported kinds, and ordered child entries without assigning trust.

**counterexample**: A metadata object merged across events or relays is not SimpleGroupMetadata.

**lifecycle**: Decoded synchronously from one EventValue and immutable thereafter.

**forcing_requirement**: Applications need typed metadata semantics without raw tag walking or losing malformed siblings.

**falsifier**: Metadata decoder tests fail if kind/d boundaries loosen, later singleton tags win, or malformed values erase valid siblings.

**symbols**:
- fava_simple_groups::SimpleGroupMetadata
```

## SimpleGroupAdmins

Canonical SHA-256: `10aacf7e83e174f3363cf101a6664dcd4ae40ab8066b0c5e523a7c2941e7d85f`

```markdown
# SimpleGroupAdmins

**source**: nostr

**protocol**: NIP-29

**owner**: fava-simple-groups

**nearest_nostr**: Kind-39001 group administrators event

**meaning**: The ordered administrator entries decoded from one kind-39001 event.

**distinction**: Each p tag retains one public key, every role value, or its own parse failure.

**counterexample**: A membership set, relay-authority projection, or merged admin state is not SimpleGroupAdmins.

**lifecycle**: Decoded synchronously from one EventValue and immutable thereafter.

**forcing_requirement**: Applications need all administrator entries and entry-local failures in source order.

**falsifier**: People decoder tests fail if repetitions collapse, roles reorder, or one malformed p tag removes valid siblings.

**symbols**:
- fava_simple_groups::SimpleGroupAdmins
```

## SimpleGroupMembers

Canonical SHA-256: `c5dd38a3cfd65b3097a6823c1ffb25d613c2e12612739fb86b2392817df27b34`

```markdown
# SimpleGroupMembers

**source**: nostr

**protocol**: NIP-29

**owner**: fava-simple-groups

**nearest_nostr**: Kind-39002 group members event

**meaning**: The ordered member entries decoded from one kind-39002 event.

**distinction**: Each p tag remains a public key or its own parse failure without set canonicalization.

**counterexample**: An inferred membership absence or member set merged across events is not SimpleGroupMembers.

**lifecycle**: Decoded synchronously from one EventValue and immutable thereafter.

**forcing_requirement**: Applications need typed member entries without raw tag walking or sibling erasure.

**falsifier**: People decoder tests fail if repeated members collapse or malformed entries erase later valid members.

**symbols**:
- fava_simple_groups::SimpleGroupMembers
```

## SimpleGroupRoles

Canonical SHA-256: `0f6556582caf6b23e8bb1b58e13467935a1dc7088891cef1b45c3974eff3158a`

```markdown
# SimpleGroupRoles

**source**: nostr

**protocol**: NIP-29

**owner**: fava-simple-groups

**nearest_nostr**: Kind-39003 group roles event

**meaning**: The ordered role names and optional descriptions decoded from one kind-39003 event.

**distinction**: Each role tag retains its first name, optional description, or local missing-value failure.

**counterexample**: Administrator role assignments on kind 39001 are not SimpleGroupRoles.

**lifecycle**: Decoded synchronously from one EventValue and immutable thereafter.

**forcing_requirement**: Applications need role definitions separately from administrator assignments.

**falsifier**: Role decoder tests fail if repetitions collapse, descriptions move, or missing names erase valid siblings.

**symbols**:
- fava_simple_groups::SimpleGroupRoles
```

## SimpleGroupLivekitParticipants

Canonical SHA-256: `5d4a76ce55d8503142ea98a18c18c45f7e8d70b7f9eb5d82c06d4cc7e2a65222`

```markdown
# SimpleGroupLivekitParticipants

**source**: nostr

**protocol**: NIP-29

**owner**: fava-simple-groups

**nearest_nostr**: Kind-39004 LiveKit participants event

**meaning**: The ordered exact-lowercase participant public keys decoded from one kind-39004 event.

**distinction**: Participant tags use their own lowercase-hex rule rather than member p-tag semantics.

**counterexample**: Kind-39002 members or case-normalized public keys are not SimpleGroupLivekitParticipants.

**lifecycle**: Decoded synchronously from one EventValue and immutable thereafter.

**forcing_requirement**: Applications need the protocol's participant-specific tag and key rules without raw decoding.

**falsifier**: LiveKit decoder tests fail if uppercase hex succeeds, valid lowercase keys fail, or malformed siblings erase valid entries.

**symbols**:
- fava_simple_groups::SimpleGroupLivekitParticipants
```

## SimpleGroupPins

Canonical SHA-256: `bdd26c9e706c0edb351403c6ada1ae4a727eaff0de77fce7d3575803e9d8c7e6`

```markdown
# SimpleGroupPins

**source**: nostr

**protocol**: NIP-29

**owner**: fava-simple-groups

**nearest_nostr**: Kind-39005 pinned items event

**meaning**: The source-interleaved event-id and address-coordinate pins decoded from one kind-39005 event.

**distinction**: Existing EventCoordinate represents both e and a targets while ordered Result entries retain malformed pins.

**counterexample**: A grouped pair of event-id and address lists or pins gathered from content events is not SimpleGroupPins.

**lifecycle**: Decoded synchronously from one EventValue and immutable thereafter.

**forcing_requirement**: Applications need typed pin targets without losing e/a interleaving or entry failures.

**falsifier**: Pin decoder tests fail if e/a entries regroup, invalid coordinates disappear, or repetitions collapse.

**symbols**:
- fava_simple_groups::SimpleGroupPins
```

## SimpleGroupDecodeError

Canonical SHA-256: `ca0e1dc2227ef7fb544a742f5ccf6148852cd80db8c928b7dccfaaa563f456f1`

```markdown
# SimpleGroupDecodeError

**source**: fava

**owner**: fava-simple-groups

**nearest_nostr**: NIP-29 group state event and semantic tag

**meaning**: A wrong event boundary or one semantic tag-value parse failure while decoding kinds 39000 through 39005.

**distinction**: Its variants share the single-event semantic decode boundary and retain exact source positions.

**counterexample**: Signature, event-id verification, relay provenance, replacement, query, and publication failures are not SimpleGroupDecodeError.

**lifecycle**: Created synchronously for a whole-event refusal or retained inside one decoded entry Result.

**forcing_requirement**: Malformed semantic input must remain attributable without widening into generic event validity.

**falsifier**: Decoder tests fail if wrong kinds or missing d values pass, entry indexes are lost, or one entry failure aborts valid siblings.

**symbols**:
- fava_simple_groups::SimpleGroupDecodeError
```

## SavedSimpleGroup

Canonical SHA-256: `85dd91924e8ef1cd52224574de5e77cf4ba8bc5561efbdf89974e43e59ceb5aa`

```markdown
# SavedSimpleGroup

**source**: nostr

**protocol**: NIP-29

**owner**: fava-simple-groups

**nearest_nostr**: Kind-10009 group tag

**meaning**: One valid saved group entry containing opaque id, relay URL, and optional display name.

**distinction**: The relay is inert saved-list data and the display name belongs to this exact entry.

**counterexample**: A SimpleGroup relay sequence or active routing destination is not SavedSimpleGroup.

**lifecycle**: Decoded as an immutable entry inside one SavedGroupList.

**forcing_requirement**: Applications need typed group-list entries without mistaking saved relays for active policy.

**falsifier**: Saved-list tests fail if id, relay, name, repetition, or source order is lost.

**symbols**:
- fava_simple_groups::SavedSimpleGroup
```

## SavedGroupList

Canonical SHA-256: `1b82c4266fae5831efe71c3ff1ed4d4b184825a1660969c30d1ad151c42c827e`

```markdown
# SavedGroupList

**source**: nostr

**protocol**: NIP-29

**owner**: fava-simple-groups

**nearest_nostr**: Kind-10009 Simple Group List event

**meaning**: One decoded kind-10009 event with author and ordered saved-group and relay entry results.

**distinction**: One value corresponds to one event and retains both tag families, repetitions, and local failures.

**counterexample**: Entries merged across authors or replacement candidates are not one SavedGroupList.

**lifecycle**: Decoded synchronously from one EventValue and immutable thereafter.

**forcing_requirement**: Applications need one-event list semantics without raw tags or hidden replacement policy.

**falsifier**: Saved-list tests fail if wrong kinds pass, authors change, entry families are rescanned, or malformed siblings erase valid entries.

**symbols**:
- fava_simple_groups::SavedGroupList
```

## SavedGroupListMaterializer

Canonical SHA-256: `a3b3b439ab6734045e8f4b53589b4b416f33e1a9dae263f00ad3f4ec7528427d`

```markdown
# SavedGroupListMaterializer

**source**: fava

**owner**: fava-simple-groups

**nearest_nostr**: Kind-10009 Simple Group List replacement

**meaning**: The private implementation that applies this crate's saved-list edit codec through ReplaceableEventMaterializer.

**distinction**: The public function returns the neutral contract, while this concrete type owns only decoding and applying the crate-private edit bytes.

**counterexample**: A signer, author selector, router, write store, publication owner, or public provider contract is not SavedGroupListMaterializer.

**lifecycle**: Constructed fresh behind Arc, called by Fava for one materialization, and retains no mutable state.

**forcing_requirement**: ReplaceableEventEdit requires one concrete neutral-contract implementation to rematerialize against the current kind-10009 source.

**falsifier**: Saved edit tests fail if another edit kind is accepted, source material is lost, or repeated calls retain state.
```

## SavedGroupListDecodeError

Canonical SHA-256: `7dfd0f2c30b7e5a5a2a3cabb3048af81f5cb90d5113967b470b5ffc79b2b8e32`

```markdown
# SavedGroupListDecodeError

**source**: fava

**owner**: fava-simple-groups

**nearest_nostr**: Kind-10009 Simple Group List event and entry

**meaning**: A wrong kind or one missing or invalid saved-list entry value.

**distinction**: It retains exact entry positions and the saved-list-specific relay URL failure.

**counterexample**: Generic event verification, replacement, query, or edit failures are not SavedGroupListDecodeError.

**lifecycle**: Returned for the whole event or retained inside one SavedGroupList entry Result.

**forcing_requirement**: Malformed saved-list entries must remain attributable without erasing valid sibling entries.

**falsifier**: Saved-list tests fail if wrong kinds pass, indexes disappear, invalid relay URLs succeed, or one malformed entry aborts the event.

**symbols**:
- fava_simple_groups::SavedGroupListDecodeError
```
