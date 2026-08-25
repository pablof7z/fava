# fava-query

Declarative event queries and neutral current-source snapshots. A query keeps
acquisition, result authority, and exact relay access as independent identity.
Evaluation qualifies each atomic relay contribution by access and, for
`OnlyRelays`, URL before same-id aggregation and one coordinate winner.

```rust
use fava_query::Query;
use fava_relay::RelayAccess;
use nostr::key::Keys;

let public = Query::events().with_relay_access(RelayAccess::Public);
let authenticated = Query::events()
    .with_relay_access(RelayAccess::Authenticated(Keys::generate().public_key()));
assert_ne!(public, authenticated);
```

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_query` |  |
| Enum | `fava_query::AuthenticationState` |  |
| Enum variant | `fava_query::AuthenticationState::AcceptedButStillRefused` |  |
| Enum variant | `fava_query::AuthenticationState::Attempted` |  |
| Enum variant | `fava_query::AuthenticationState::ChallengeReceived` |  |
| Enum variant | `fava_query::AuthenticationState::Declined` |  |
| Enum variant | `fava_query::AuthenticationState::Rejected` |  |
| Public field | `fava_query::AuthenticationState::Rejected::message` |  |
| Struct | `fava_query::BoundedText` |  |
| Constant | `fava_query::BoundedText::MAX_BYTES` |  |
| Method | `fava_query::BoundedText::as_str` |  |
| Method | `fava_query::BoundedText::new` |  |
| Method | `fava_query::BoundedText::truncated_bytes` |  |
| Struct | `fava_query::DesiredPlanEvidence` |  |
| Public field | `fava_query::DesiredPlanEvidence::installed` |  |
| Public field | `fava_query::DesiredPlanEvidence::relays` |  |
| Public field | `fava_query::DesiredPlanEvidence::revision` |  |
| Public field | `fava_query::EventId` |  |
| Struct | `fava_query::EventRecord` |  |
| Method | `fava_query::EventRecord::created_at` |  |
| Method | `fava_query::EventRecord::event` |  |
| Method | `fava_query::EventRecord::id` |  |
| Method | `fava_query::EventRecord::new` |  |
| Method | `fava_query::EventRecord::publication` |  |
| Method | `fava_query::EventRecord::relay_occurrences` |  |
| Struct | `fava_query::FilterSelection` |  |
| Public field | `fava_query::FilterSelection::authors` |  |
| Public field | `fava_query::FilterSelection::ids` |  |
| Public field | `fava_query::FilterSelection::kinds` |  |
| Public field | `fava_query::FilterSelection::tag_values` |  |
| Enum | `fava_query::Freshness` |  |
| Enum variant | `fava_query::Freshness::CacheOnly` |  |
| Enum variant | `fava_query::Freshness::Live` |  |
| Public field | `fava_query::Kind` |  |
| Struct | `fava_query::ObservationId` |  |
| Method | `fava_query::ObservationId::get` |  |
| Method | `fava_query::ObservationId::new` |  |
| Struct | `fava_query::ObservationIds` |  |
| Method | `fava_query::ObservationIds::allocate` |  |
| Method | `fava_query::ObservationIds::new` |  |
| Struct | `fava_query::OpenedQuerySource` |  |
| Public field | `fava_query::OpenedQuerySource::changes` |  |
| Public field | `fava_query::OpenedQuerySource::initial` |  |
| Struct | `fava_query::OperationGeneration` |  |
| Public field | `fava_query::OperationGeneration::0` |  |
| Method | `fava_query::OperationGeneration::next` |  |
| Public field | `fava_query::PublicKey` |  |
| Struct | `fava_query::Query` |  |
| Method | `fava_query::Query::access` |  |
| Method | `fava_query::Query::authors` |  |
| Method | `fava_query::Query::cache_only` |  |
| Method | `<fava_query::Query as core::default::Default>::default` |  |
| Method | `fava_query::Query::events` |  |
| Method | `fava_query::Query::freshness` |  |
| Method | `fava_query::Query::from_relays` |  |
| Method | `fava_query::Query::ids` |  |
| Method | `fava_query::Query::kind` |  |
| Method | `fava_query::Query::limit` |  |
| Method | `fava_query::Query::oldest_first` |  |
| Method | `fava_query::Query::only_from_relays` |  |
| Method | `fava_query::Query::ordering` |  |
| Method | `fava_query::Query::result_limit` |  |
| Method | `fava_query::Query::selection` |  |
| Method | `fava_query::Query::source` |  |
| Method | `fava_query::Query::tag_values` |  |
| Method | `fava_query::Query::with_relay_access` |  |
| Enum | `fava_query::QueryAcquisition` |  |
| Enum variant | `fava_query::QueryAcquisition::Automatic` |  |
| Enum variant | `fava_query::QueryAcquisition::Explicit` |  |
| Public field | `fava_query::QueryAcquisition::Explicit::0` |  |
| Struct | `fava_query::QueryBounds` |  |
| Public field | `fava_query::QueryBounds::limit` |  |
| Public field | `fava_query::QueryBounds::since` |  |
| Public field | `fava_query::QueryBounds::until` |  |
| Struct | `fava_query::QueryBranchId` |  |
| Public field | `fava_query::QueryBranchId::0` |  |
| Constant | `fava_query::QueryBranchId::ROOT` |  |
| Enum | `fava_query::QueryError` |  |
| Enum variant | `fava_query::QueryError::EmptyExplicitRelays` |  |
| Enum variant | `fava_query::QueryError::ZeroLimit` |  |
| Enum | `fava_query::QueryEvaluationError` |  |
| Enum variant | `fava_query::QueryEvaluationError::MissingEventId` |  |
| Enum variant | `fava_query::QueryEvaluationError::Refused` |  |
| Public field | `fava_query::QueryEvaluationError::Refused::0` |  |
| Enum variant | `fava_query::QueryEvaluationError::RelayOccurrenceEventMismatch` |  |
| Public field | `fava_query::QueryEvaluationError::RelayOccurrenceEventMismatch::event` |  |
| Public field | `fava_query::QueryEvaluationError::RelayOccurrenceEventMismatch::occurrences` |  |
| Trait | `fava_query::QueryEvaluator` |  |
| Method | `fava_query::QueryEvaluator::evaluate` |  |
| Struct | `fava_query::QueryEvidence` |  |
| Method | `fava_query::QueryEvidence::all_relays_stored_events_complete` |  |
| Public field | `fava_query::QueryEvidence::plan` |  |
| Method | `fava_query::QueryEvidence::relay` |  |
| Public field | `fava_query::QueryEvidence::relays` |  |
| Method | `fava_query::QueryEvidence::relays_at` |  |
| Public field | `fava_query::QueryEvidence::shortfalls` |  |
| Method | `fava_query::QueryEvidence::source` |  |
| Public field | `fava_query::QueryEvidence::sources` |  |
| Enum | `fava_query::QueryOrdering` |  |
| Enum variant | `fava_query::QueryOrdering::NewestFirst` |  |
| Enum variant | `fava_query::QueryOrdering::OldestFirst` |  |
| Struct | `fava_query::QueryRevision` |  |
| Public field | `fava_query::QueryRevision::0` |  |
| Enum | `fava_query::QueryShortfall` |  |
| Enum variant | `fava_query::QueryShortfall::CoalescedUpdates` |  |
| Public field | `fava_query::QueryShortfall::CoalescedUpdates::dropped` |  |
| Enum variant | `fava_query::QueryShortfall::LiveRetentionLimit` |  |
| Public field | `fava_query::QueryShortfall::LiveRetentionLimit::limit` |  |
| Public field | `fava_query::QueryShortfall::LiveRetentionLimit::refused` |  |
| Public field | `fava_query::QueryShortfall::LiveRetentionLimit::session` |  |
| Enum variant | `fava_query::QueryShortfall::ResultLimitApplied` |  |
| Public field | `fava_query::QueryShortfall::ResultLimitApplied::limit` |  |
| Enum variant | `fava_query::QueryShortfall::SourceUnavailable` |  |
| Public field | `fava_query::QueryShortfall::SourceUnavailable::detail` |  |
| Public field | `fava_query::QueryShortfall::SourceUnavailable::kind` |  |
| Struct | `fava_query::QuerySnapshot` |  |
| Method | `fava_query::QuerySnapshot::evaluated` |  |
| Public field | `fava_query::QuerySnapshot::events` |  |
| Public field | `fava_query::QuerySnapshot::evidence` |  |
| Public field | `fava_query::QuerySnapshot::revision` |  |
| Trait | `fava_query::QuerySource` |  |
| Method | `fava_query::QuerySource::open` |  |
| Struct | `fava_query::QuerySourceClosed` |  |
| Public field | `fava_query::QuerySourceClosed::cause` |  |
| Method | `fava_query::QuerySourceClosed::local_close` |  |
| Method | `fava_query::QuerySourceClosed::new` |  |
| Method | `fava_query::QuerySourceClosed::provider_closed` |  |
| Method | `fava_query::QuerySourceClosed::provider_failed` |  |
| Method | `fava_query::QuerySourceClosed::shutdown` |  |
| Method | `fava_query::QuerySourceClosed::status` |  |
| Enum | `fava_query::QuerySourceError` |  |
| Enum variant | `fava_query::QuerySourceError::Closed` |  |
| Enum variant | `fava_query::QuerySourceError::Refused` |  |
| Public field | `fava_query::QuerySourceError::Refused::0` |  |
| Struct | `fava_query::QuerySourcePolicy` |  |
| Method | `fava_query::QuerySourcePolicy::acquisition` |  |
| Method | `fava_query::QuerySourcePolicy::authority` |  |
| Method | `<fava_query::QuerySourcePolicy as core::default::Default>::default` |  |
| Enum | `fava_query::RelayDeadline` |  |
| Enum variant | `fava_query::RelayDeadline::Close` |  |
| Enum variant | `fava_query::RelayDeadline::Establish` |  |
| Enum variant | `fava_query::RelayDeadline::Idle` |  |
| Enum variant | `fava_query::RelayDeadline::Write` |  |
| Struct | `fava_query::RelayQueryEvidence` |  |
| Public field | `fava_query::RelayQueryEvidence::branches` |  |
| Public field | `fava_query::RelayQueryEvidence::generation` |  |
| Method | `fava_query::RelayQueryEvidence::is_live` |  |
| Public field | `fava_query::RelayQueryEvidence::plan_revision` |  |
| Public field | `fava_query::RelayQueryEvidence::route` |  |
| Public field | `fava_query::RelayQueryEvidence::session` |  |
| Public field | `fava_query::RelayQueryEvidence::shared_with` |  |
| Public field | `fava_query::RelayQueryEvidence::shortfall` |  |
| Public field | `fava_query::RelayQueryEvidence::state` |  |
| Method | `fava_query::RelayQueryEvidence::stored_events_complete` |  |
| Struct | `fava_query::RelayShortfall` |  |
| Public field | `fava_query::RelayShortfall::branches` |  |
| Public field | `fava_query::RelayShortfall::detail` |  |
| Enum | `fava_query::RelaySourceState` |  |
| Enum variant | `fava_query::RelaySourceState::AuthenticationRequired` |  |
| Public field | `fava_query::RelaySourceState::AuthenticationRequired::at` |  |
| Public field | `fava_query::RelaySourceState::AuthenticationRequired::state` |  |
| Enum variant | `fava_query::RelaySourceState::Connecting` |  |
| Enum variant | `fava_query::RelaySourceState::Disconnected` |  |
| Public field | `fava_query::RelaySourceState::Disconnected::detail` |  |
| Enum variant | `fava_query::RelaySourceState::Open` |  |
| Public field | `fava_query::RelaySourceState::Open::requested_at` |  |
| Enum variant | `fava_query::RelaySourceState::Planned` |  |
| Enum variant | `fava_query::RelaySourceState::Refused` |  |
| Public field | `fava_query::RelaySourceState::Refused::at` |  |
| Public field | `fava_query::RelaySourceState::Refused::message` |  |
| Enum variant | `fava_query::RelaySourceState::StoredEventsComplete` |  |
| Public field | `fava_query::RelaySourceState::StoredEventsComplete::at` |  |
| Enum variant | `fava_query::RelaySourceState::TimedOut` |  |
| Public field | `fava_query::RelaySourceState::TimedOut::after_ms` |  |
| Public field | `fava_query::RelaySourceState::TimedOut::deadline` |  |
| Enum variant | `fava_query::RelaySourceState::Unreachable` |  |
| Public field | `fava_query::RelaySourceState::Unreachable::attempts` |  |
| Public field | `fava_query::RelaySourceState::Unreachable::detail` |  |
| Enum variant | `fava_query::RelaySourceState::Withdrawn` |  |
| Public field | `fava_query::RelaySourceState::Withdrawn::reason` |  |
| Public field | `fava_query::RelayUrl` |  |
| Enum | `fava_query::RelayWithdrawal` |  |
| Enum variant | `fava_query::RelayWithdrawal::ObservationClosed` |  |
| Enum variant | `fava_query::RelayWithdrawal::RouteWithdrawn` |  |
| Enum variant | `fava_query::RelayWithdrawal::Shutdown` |  |
| Enum | `fava_query::ResultAuthority` |  |
| Enum variant | `fava_query::ResultAuthority::AnyLocal` |  |
| Enum variant | `fava_query::ResultAuthority::OnlyRelays` |  |
| Public field | `fava_query::ResultAuthority::OnlyRelays::0` |  |
| Enum | `fava_query::RouteOrigin` |  |
| Enum variant | `fava_query::RouteOrigin::Automatic` |  |
| Public field | `fava_query::RouteOrigin::Automatic::revision` |  |
| Enum variant | `fava_query::RouteOrigin::Explicit` |  |
| Public field | `fava_query::SingleLetterTag` |  |
| Type alias | `fava_query::SourceChangeFuture` |  |
| Trait | `fava_query::SourceChanges` |  |
| Method | `fava_query::SourceChanges::close` |  |
| Method | `fava_query::SourceChanges::next_change` |  |
| Enum | `fava_query::SourceEvent` |  |
| Enum variant | `fava_query::SourceEvent::Local` |  |
| Public field | `fava_query::SourceEvent::Local::0` |  |
| Enum variant | `fava_query::SourceEvent::Relay` |  |
| Public field | `fava_query::SourceEvent::Relay::0` |  |
| Struct | `fava_query::SourceEvidence` |  |
| Public field | `fava_query::SourceEvidence::kind` |  |
| Method | `fava_query::SourceEvidence::retraction` |  |
| Public field | `fava_query::SourceEvidence::retractions` |  |
| Public field | `fava_query::SourceEvidence::revision` |  |
| Public field | `fava_query::SourceEvidence::status` |  |
| Enum | `fava_query::SourceKind` |  |
| Enum variant | `fava_query::SourceKind::EventCache` |  |
| Enum variant | `fava_query::SourceKind::LiveRelay` |  |
| Public field | `fava_query::SourceKind::LiveRelay::session` |  |
| Enum variant | `fava_query::SourceKind::WriteStore` |  |
| Struct | `fava_query::SourceRetraction` |  |
| Public field | `fava_query::SourceRetraction::cause` |  |
| Public field | `fava_query::SourceRetraction::event_id` |  |
| Method | `fava_query::SourceRetraction::is_protocol_rule` |  |
| Method | `fava_query::SourceRetraction::new` |  |
| Struct | `fava_query::SourceRevision` |  |
| Public field | `fava_query::SourceRevision::0` |  |
| Struct | `fava_query::SourceSnapshot` |  |
| Method | `fava_query::SourceSnapshot::current` |  |
| Method | `fava_query::SourceSnapshot::empty` |  |
| Public field | `fava_query::SourceSnapshot::events` |  |
| Public field | `fava_query::SourceSnapshot::kind` |  |
| Public field | `fava_query::SourceSnapshot::retractions` |  |
| Public field | `fava_query::SourceSnapshot::revision` |  |
| Public field | `fava_query::SourceSnapshot::status` |  |
| Enum | `fava_query::SourceStatus` |  |
| Enum variant | `fava_query::SourceStatus::Closed` |  |
| Public field | `fava_query::SourceStatus::Closed::cause` |  |
| Enum variant | `fava_query::SourceStatus::Open` |  |
| Enum | `fava_query::SourceTerminationCause` |  |
| Enum variant | `fava_query::SourceTerminationCause::LocalClose` |  |
| Enum variant | `fava_query::SourceTerminationCause::ProviderClosed` |  |
| Enum variant | `fava_query::SourceTerminationCause::ProviderFailed` |  |
| Public field | `fava_query::SourceTerminationCause::ProviderFailed::detail` |  |
| Enum variant | `fava_query::SourceTerminationCause::Shutdown` |  |
| Method | `<fava_query::SourceTerminationCause as core::fmt::Display>::fmt` |  |
| Public field | `fava_query::Timestamp` |  |
<!-- END crate-readme-api inventory -->
