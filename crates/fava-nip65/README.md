# fava-nip65

`fava-nip65` currently decodes one event-shaped value as a kind-10002 relay
list. It owns the interpretation and bounded result of NIP-65 `r` tags. It does
not admit events, query relays, select replaceable-event winners, retain cache
state, choose routes, publish events, or report delivery.

## Decode a relay list

`RelayList::from_event` accepts the current `fava_write::EventValue` boundary.
For each tag independently:

- exact lowercase `r` with no marker contributes to read and write;
- exact `read` or `write` contributes only to that set;
- short tags, invalid relay URLs, unknown markers, present empty markers, and
  unrelated tags are ignored;
- later tag cells are ignored and repeated parsed relay identities deduplicate.

The union of the two sets is bounded to 256 distinct relay identities. The
decoder stops at identity 257 and returns no partial list.

```rust
use fava_nip65::RelayList;

# fn inspect(event: &fava_write::EventValue) -> Result<(), Box<dyn std::error::Error>> {
let list = RelayList::from_event(event)?;
println!("author: {}", list.author());
for relay in list.read_relays() {
    println!("reads mentions at {relay}");
}
for relay in list.write_relays() {
    println!("publishes at {relay}");
}
# Ok(())
# }
```

## Current foundation boundary

The compiler-visible surface still retains `event_id`, `created_at`,
`supersedes`, and `MissingEventId`. The current outbox router consumes those
members through its `KnownLists` and `remember` state. Removing that duplicate
winner state requires the separately unapproved state/query observation and
replaceable-winner foundation. This crate therefore does not expose the
foundation-dependent `relay_lists` query helper, and this README does not claim
the blocked final surface is implemented.

The current normal `fava-state` dependency likewise remains because it owns the
available `RelayUrl` path in this checkout. No compatibility path or alternate
representation is added.

## Current refusals

- `WrongKind { actual }` reports a non-10002 kind.
- `TooManyRelays { actual: 257, maximum: 256 }` reports the exact first
  over-bound distinct relay.
- `MissingEventId` remains only at the blocked current event/state boundary.

Malformed relay URLs are ignored per tag and are not event-level refusals.

## Executable evidence

```sh
cargo test -p fava-nip65 --doc
cargo test -p fava-nip65 --all-targets
bazel test //crates/fava-nip65:unit_tests
python3 tools/crate_readme_api.py check fava-nip65
```

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_nip65` | Current NIP-65 relay-list decoder crate. |
| Struct | `fava_nip65::RelayList` | Decoded author and deterministic read/write relay sets; currently also retains blocked source identity. |
| Method | `fava_nip65::RelayList::author` | Returns the author whose kind-10002 event was decoded. |
| Method | `fava_nip65::RelayList::created_at` | Returns the currently retained source timestamp; removal awaits the state/query foundation. |
| Method | `fava_nip65::RelayList::event_id` | Returns the currently retained source event id; removal awaits the state/query foundation. |
| Method | `fava_nip65::RelayList::from_event` | Tolerantly decodes one event-shaped value with an exact distinct-result bound. |
| Method | `fava_nip65::RelayList::read_relays` | Borrows the distinct decoded read relay set. |
| Method | `fava_nip65::RelayList::supersedes` | Applies the current duplicated winner comparison; removal awaits the universal state/query comparator. |
| Method | `fava_nip65::RelayList::write_relays` | Borrows the distinct decoded write relay set. |
| Enum | `fava_nip65::RelayListError` | Current event-level relay-list decoding refusals. |
| Enum variant | `fava_nip65::RelayListError::MissingEventId` | Current unfinalized-event refusal; removal awaits the state/query event-record boundary. |
| Enum variant | `fava_nip65::RelayListError::TooManyRelays` | Refuses the 257th distinct valid accepted relay identity. |
| Public field | `fava_nip65::RelayListError::TooManyRelays::actual` | Distinct accepted relay count at refusal. |
| Public field | `fava_nip65::RelayListError::TooManyRelays::maximum` | Declared maximum of 256. |
| Enum variant | `fava_nip65::RelayListError::WrongKind` | Refuses an event whose kind is not 10002. |
| Public field | `fava_nip65::RelayListError::WrongKind::actual` | Exact received event kind number. |
<!-- END crate-readme-api inventory -->
