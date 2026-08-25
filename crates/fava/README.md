# fava

The facade re-exports the neutral `EventBuildError` and `WriteIntentError`
contracts. Materializers can return `WriteIntentError` after custody, but
current publication erases it to
`PublicationError::Routing(error.to_string())`; the typed value does not
survive that boundary. Structured attribution is owned by
[issue 0025](../../docs/issues/0025-publication-materializer-error-attribution.md).

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava` |  |
| Public field | `fava::BoundKind` |  |
| Enum | `fava::BuildError` |  |
| Enum variant | `fava::BuildError::MissingDeliveryPolicy` |  |
| Enum variant | `fava::BuildError::MissingEventCache` |  |
| Enum variant | `fava::BuildError::MissingPublicationTransport` |  |
| Enum variant | `fava::BuildError::MissingPublisher` |  |
| Enum variant | `fava::BuildError::MissingQueryEvaluator` |  |
| Enum variant | `fava::BuildError::MissingWriteStore` |  |
| Enum variant | `fava::BuildError::Publication` |  |
| Public field | `fava::BuildError::Publication::0` |  |
| Enum variant | `fava::BuildError::Session` |  |
| Public field | `fava::BuildError::Session::0` |  |
| Public field | `fava::DiagnosticsSnapshot` |  |
| Public field | `fava::DroppedFacts` |  |
| Public field | `fava::Event` |  |
| Public field | `fava::EventBuildError` |  |
| Public field | `fava::EventBuilder` |  |
| Public field | `fava::EventCoordinate` |  |
| Public field | `fava::EventRecord` |  |
| Public field | `fava::EventValue` |  |
| Struct | `fava::Fava` |  |
| Method | `fava::Fava::add_signer` |  |
| Method | `fava::Fava::builder` |  |
| Method | `fava::Fava::by` |  |
| Method | `fava::Fava::cancel_publication` |  |
| Method | `fava::Fava::cancel_write` |  |
| Method | `fava::Fava::diagnostics` |  |
| Method | `fava::Fava::observe` |  |
| Method | `<fava::Fava as fava_query::QuerySource>::open` |  |
| Method | `fava::Fava::open_receipts` |  |
| Method | `fava::Fava::preview_routes` |  |
| Method | `fava::Fava::publish` |  |
| Method | `fava::Fava::receipt` |  |
| Method | `fava::Fava::receipt_changes` |  |
| Method | `fava::Fava::remove_receipt` |  |
| Method | `fava::Fava::remove_signer` |  |
| Method | `fava::Fava::replace_signer` |  |
| Method | `fava::Fava::to` |  |
| Struct | `fava::FavaBuilder` |  |
| Method | `fava::FavaBuilder::build` |  |
| Method | `fava::FavaBuilder::delivery_policy` |  |
| Method | `fava::FavaBuilder::diagnostics_capacity` |  |
| Method | `fava::FavaBuilder::event_cache` |  |
| Method | `fava::FavaBuilder::materializer` |  |
| Method | `fava::FavaBuilder::materializers` |  |
| Method | `fava::FavaBuilder::publisher` |  |
| Method | `fava::FavaBuilder::query_evaluator` |  |
| Method | `fava::FavaBuilder::router` |  |
| Method | `fava::FavaBuilder::routers` |  |
| Method | `fava::FavaBuilder::runtime` |  |
| Method | `fava::FavaBuilder::signer` |  |
| Method | `fava::FavaBuilder::signers` |  |
| Method | `fava::FavaBuilder::subscription_planner` |  |
| Method | `fava::FavaBuilder::transport` |  |
| Method | `fava::FavaBuilder::write_store` |  |
| Public field | `fava::Freshness` |  |
| Public field | `fava::Kind` |  |
| Public field | `fava::LimitDiagnostic` |  |
| Public field | `fava::LimitScope` |  |
| Public field | `fava::LogicalDemandDiagnostic` |  |
| Public field | `fava::MaterializationId` |  |
| Public field | `fava::Observation` |  |
| Public field | `fava::ObservationClosed` |  |
| Public field | `fava::ObservationId` |  |
| Public field | `fava::ObservationWireBinding` |  |
| Public field | `fava::ObserveError` |  |
| Public field | `fava::OperationGeneration` |  |
| Public field | `fava::ProviderDiagnostic` |  |
| Public field | `fava::ProviderKind` |  |
| Public field | `fava::ProviderOperation` |  |
| Public field | `fava::ProviderOperationState` |  |
| Public field | `fava::PublicKey` |  |
| Public field | `fava::PublicationError` |  |
| Function | `fava::PublishAs` |  |
| Struct | `fava::PublishAs` |  |
| Enum | `fava::PublishError` |  |
| Enum variant | `fava::PublishError::Intent` |  |
| Public field | `fava::PublishError::Intent::0` |  |
| Enum variant | `fava::PublishError::InvalidSettlementThreshold` |  |
| Enum variant | `fava::PublishError::MissingAuthor` |  |
| Enum variant | `fava::PublishError::NotReached` |  |
| Public field | `fava::PublishError::NotReached::receipt` |  |
| Enum variant | `fava::PublishError::Publication` |  |
| Public field | `fava::PublishError::Publication::0` |  |
| Function | `fava::PublishTo` |  |
| Struct | `fava::PublishTo` |  |
| Public field | `fava::Query` |  |
| Public field | `fava::QueryDiagnostic` |  |
| Public field | `fava::QueryRevision` |  |
| Public field | `fava::QuerySnapshot` |  |
| Public field | `fava::Receipt` |  |
| Public field | `fava::ReceiptId` |  |
| Public field | `fava::ReceiptOutcome` |  |
| Public field | `fava::RelayDeliveryOutcome` |  |
| Public field | `fava::RelayDiagnostic` |  |
| Public field | `fava::RelaySessionState` |  |
| Public field | `fava::RelayUrl` |  |
| Public field | `fava::ReplaceableEventEdit` |  |
| Public field | `fava::ReplaceableEventMaterializer` |  |
| Public field | `fava::ResultAuthority` |  |
| Public field | `fava::RoutePlan` |  |
| Public field | `fava::Runtime` |  |
| Public field | `fava::RuntimeConfig` |  |
| Public field | `fava::SessionError` |  |
| Public field | `fava::SingleLetterTag` |  |
| Public field | `fava::Tag` |  |
| Public field | `fava::Timestamp` |  |
| Public field | `fava::UnsignedEvent` |  |
| Public field | `fava::WireSubscriptionDiagnostic` |  |
| Struct | `fava::Write` |  |
| Method | `<fava::Write as core::fmt::Debug>::fmt` |  |
| Method | `fava::Write::receipt` |  |
| Method | `fava::Write::receipt_id` |  |
| Method | `fava::Write::settled` |  |
| Method | `fava::Write::write_id` |  |
| Public field | `fava::WriteDiagnostic` |  |
| Public field | `fava::WriteId` |  |
| Public field | `fava::WriteIntentError` |  |
| Public field | `fava::WriteRouting` |  |
| Public field | `fava::WriteStall` |  |
| Public field | `fava::WriteStoreError` |  |
| Function | `fava::all` |  |
| Function | `fava::at_least` |  |
<!-- END crate-readme-api inventory -->
