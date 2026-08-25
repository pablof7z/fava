# fava-state

Pure universal Nostr event-state values and rules. The crate owns no cache,
provider, observation, serialization, or lifecycle. Callers supply finite
current atomic relay contributions and commit each returned mutation batch
atomically.

An admitted `RelayEvent` contains one signed event and one exact
`RelaySessionKey` occurrence. `relay_occurrences_for_event` creates the private,
event-id-bound aggregate used by query records. Replacement is per exact
session; deletion and expiration may retract several exact contributions.

Winner order is universal: later `created_at` wins, then the lower event id.
Malformed optional or sibling tags remain scoped to themselves.

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_state` |  |
| Enum | `fava_state::EventCoordinate` |  |
| Enum variant | `fava_state::EventCoordinate::Event` |  |
| Public field | `fava_state::EventCoordinate::Event::0` |  |
| Enum variant | `fava_state::EventCoordinate::Replaceable` |  |
| Public field | `fava_state::EventCoordinate::Replaceable::author` |  |
| Public field | `fava_state::EventCoordinate::Replaceable::identifier` |  |
| Public field | `fava_state::EventCoordinate::Replaceable::kind` |  |
| Enum | `fava_state::EventStateMutation` |  |
| Enum variant | `fava_state::EventStateMutation::Retract` |  |
| Public field | `fava_state::EventStateMutation::Retract::cause` |  |
| Public field | `fava_state::EventStateMutation::Retract::event_id` |  |
| Public field | `fava_state::EventStateMutation::Retract::session` |  |
| Enum variant | `fava_state::EventStateMutation::Upsert` |  |
| Public field | `fava_state::EventStateMutation::Upsert::0` |  |
| Struct | `fava_state::RelayEvent` |  |
| Method | `fava_state::RelayEvent::event` |  |
| Method | `fava_state::RelayEvent::new` |  |
| Method | `fava_state::RelayEvent::occurrence` |  |
| Struct | `fava_state::RelayOccurrence` |  |
| Public field | `fava_state::RelayOccurrence::observed_at` |  |
| Public field | `fava_state::RelayOccurrence::session` |  |
| Struct | `fava_state::RelayOccurrences` |  |
| Method | `fava_state::RelayOccurrences::event_id` |  |
| Method | `fava_state::RelayOccurrences::is_empty` |  |
| Method | `fava_state::RelayOccurrences::len` |  |
| Method | `fava_state::RelayOccurrences::occurrences` |  |
| Enum | `fava_state::RetractionCause` |  |
| Enum variant | `fava_state::RetractionCause::Deleted` |  |
| Public field | `fava_state::RetractionCause::Deleted::deletion` |  |
| Enum variant | `fava_state::RetractionCause::Evicted` |  |
| Enum variant | `fava_state::RetractionCause::Expired` |  |
| Enum variant | `fava_state::RetractionCause::Superseded` |  |
| Public field | `fava_state::RetractionCause::Superseded::by` |  |
| Function | `fava_state::deletion_applies` |  |
| Function | `fava_state::event_coordinate` |  |
| Function | `fava_state::event_is_expired` |  |
| Function | `fava_state::event_is_newer` |  |
| Function | `fava_state::mutations_for_event` |  |
| Function | `fava_state::mutations_for_expiration` |  |
| Function | `fava_state::relay_occurrences_for_event` |  |
<!-- END crate-readme-api inventory -->
