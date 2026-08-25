# fava-write

`EventBuildError` is the checked construction boundary for one generic
`UnsignedEvent`. Its exhaustive conversion to `WriteIntentError` preserves tag
cardinality, serialized-byte overflow, and encoding failure without assigning
publication lifecycle meaning. `WriteIntentError` is returned both before
custody and by replaceable materializers; publication attribution remains the
separate work tracked by [issue 0025](../../docs/issues/0025-publication-materializer-error-attribution.md).

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_write` |  |
| Public field | `fava_write::Event` |  |
| Enum | `fava_write::EventBuildError` |  |
| Enum variant | `fava_write::EventBuildError::Encoding` |  |
| Public field | `fava_write::EventBuildError::Encoding::0` |  |
| Enum variant | `fava_write::EventBuildError::TooLarge` |  |
| Public field | `fava_write::EventBuildError::TooLarge::bytes` |  |
| Public field | `fava_write::EventBuildError::TooLarge::maximum` |  |
| Enum variant | `fava_write::EventBuildError::TooManyTags` |  |
| Public field | `fava_write::EventBuildError::TooManyTags::actual` |  |
| Public field | `fava_write::EventBuildError::TooManyTags::maximum` |  |
| Struct | `fava_write::EventBuilder` |  |
| Method | `fava_write::EventBuilder::build` |  |
| Method | `fava_write::EventBuilder::content` |  |
| Method | `fava_write::EventBuilder::created_at` |  |
| Method | `fava_write::EventBuilder::from_parts` |  |
| Method | `fava_write::EventBuilder::new` |  |
| Method | `fava_write::EventBuilder::tag` |  |
| Method | `fava_write::EventBuilder::tags` |  |
| Public field | `fava_write::EventId` |  |
| Enum | `fava_write::EventValue` |  |
| Enum variant | `fava_write::EventValue::Signed` |  |
| Public field | `fava_write::EventValue::Signed::0` |  |
| Enum variant | `fava_write::EventValue::Unsigned` |  |
| Public field | `fava_write::EventValue::Unsigned::0` |  |
| Method | `fava_write::EventValue::author` |  |
| Method | `fava_write::EventValue::coordinate` |  |
| Method | `fava_write::EventValue::created_at` |  |
| Method | `fava_write::EventValue::id` |  |
| Method | `fava_write::EventValue::kind` |  |
| Method | `fava_write::EventValue::tags` |  |
| Enum | `fava_write::InvalidEventValue` |  |
| Enum variant | `fava_write::InvalidEventValue::MissingId` |  |
| Public field | `fava_write::Kind` |  |
| Struct | `fava_write::LocalWriteEvent` |  |
| Public field | `fava_write::LocalWriteEvent::event` |  |
| Method | `fava_write::LocalWriteEvent::id` |  |
| Method | `fava_write::LocalWriteEvent::new` |  |
| Public field | `fava_write::LocalWriteEvent::publication` |  |
| Struct | `fava_write::MaterializationId` |  |
| Method | `fava_write::MaterializationId::as_u64` |  |
| Method | `fava_write::MaterializationId::from_u64` |  |
| Public field | `fava_write::PublicKey` |  |
| Struct | `fava_write::PublicationEvidence` |  |
| Public field | `fava_write::PublicationEvidence::destinations` |  |
| Public field | `fava_write::PublicationEvidence::materialization_failure` |  |
| Public field | `fava_write::PublicationEvidence::materialization_id` |  |
| Public field | `fava_write::PublicationEvidence::materialization_source` |  |
| Public field | `fava_write::PublicationEvidence::receipt_id` |  |
| Public field | `fava_write::PublicationEvidence::retired_materializations` |  |
| Public field | `fava_write::PublicationEvidence::signature` |  |
| Public field | `fava_write::PublicationEvidence::write_id` |  |
| Struct | `fava_write::Receipt` |  |
| Method | `fava_write::Receipt::acknowledged` |  |
| Public field | `fava_write::Receipt::attempts` |  |
| Public field | `fava_write::Receipt::current` |  |
| Method | `fava_write::Receipt::desired` |  |
| Public field | `fava_write::Receipt::desired_destinations` |  |
| Method | `fava_write::Receipt::desires` |  |
| Method | `fava_write::Receipt::destinations` |  |
| Method | `fava_write::Receipt::is_terminal` |  |
| Public field | `fava_write::Receipt::outcome` |  |
| Public field | `fava_write::Receipt::receipt_id` |  |
| Method | `fava_write::Receipt::rejected` |  |
| Public field | `fava_write::Receipt::route_revision` |  |
| Public field | `fava_write::Receipt::route_settled` |  |
| Public field | `fava_write::Receipt::route_shortfalls` |  |
| Public field | `fava_write::Receipt::routing` |  |
| Public field | `fava_write::Receipt::write_id` |  |
| Struct | `fava_write::ReceiptId` |  |
| Method | `fava_write::ReceiptId::as_u64` |  |
| Method | `fava_write::ReceiptId::from_u64` |  |
| Enum | `fava_write::ReceiptOutcome` |  |
| Enum variant | `fava_write::ReceiptOutcome::Cancelled` |  |
| Enum variant | `fava_write::ReceiptOutcome::Complete` |  |
| Enum variant | `fava_write::ReceiptOutcome::NoDestination` |  |
| Enum variant | `fava_write::ReceiptOutcome::Open` |  |
| Enum | `fava_write::RelayDeliveryOutcome` |  |
| Enum variant | `fava_write::RelayDeliveryOutcome::Acknowledged` |  |
| Public field | `fava_write::RelayDeliveryOutcome::Acknowledged::message` |  |
| Enum variant | `fava_write::RelayDeliveryOutcome::Attempting` |  |
| Enum variant | `fava_write::RelayDeliveryOutcome::AuthenticationDenied` |  |
| Public field | `fava_write::RelayDeliveryOutcome::AuthenticationDenied::reason` |  |
| Enum variant | `fava_write::RelayDeliveryOutcome::CancelledBeforeHandoff` |  |
| Enum variant | `fava_write::RelayDeliveryOutcome::GivenUp` |  |
| Public field | `fava_write::RelayDeliveryOutcome::GivenUp::reason` |  |
| Enum variant | `fava_write::RelayDeliveryOutcome::Pending` |  |
| Enum variant | `fava_write::RelayDeliveryOutcome::Rejected` |  |
| Public field | `fava_write::RelayDeliveryOutcome::Rejected::message` |  |
| Enum variant | `fava_write::RelayDeliveryOutcome::Retryable` |  |
| Public field | `fava_write::RelayDeliveryOutcome::Retryable::reason` |  |
| Enum variant | `fava_write::RelayDeliveryOutcome::Unknown` |  |
| Public field | `fava_write::RelayDeliveryOutcome::Unknown::reason` |  |
| Method | `fava_write::RelayDeliveryOutcome::is_terminal` |  |
| Struct | `fava_write::ReplaceableEventEdit` |  |
| Method | `fava_write::ReplaceableEventEdit::change` |  |
| Method | `<fava_write::ReplaceableEventEdit as serde_core::de::Deserialize<'de>>::deserialize` |  |
| Method | `fava_write::ReplaceableEventEdit::identifier` |  |
| Method | `fava_write::ReplaceableEventEdit::kind` |  |
| Method | `fava_write::ReplaceableEventEdit::new` |  |
| Method | `<fava_write::ReplaceableEventEdit as serde_core::ser::Serialize>::serialize` |  |
| Trait | `fava_write::ReplaceableEventMaterializer` |  |
| Method | `fava_write::ReplaceableEventMaterializer::kind` |  |
| Method | `fava_write::ReplaceableEventMaterializer::materialize` |  |
| Method | `fava_write::ReplaceableEventMaterializer::supports` |  |
| Enum | `fava_write::SignatureState` |  |
| Enum variant | `fava_write::SignatureState::Refused` |  |
| Public field | `fava_write::SignatureState::Refused::0` |  |
| Enum variant | `fava_write::SignatureState::Signed` |  |
| Enum variant | `fava_write::SignatureState::Unsigned` |  |
| Public field | `fava_write::Tag` |  |
| Public field | `fava_write::Timestamp` |  |
| Public field | `fava_write::UnsignedEvent` |  |
| Struct | `fava_write::WriteId` |  |
| Method | `fava_write::WriteId::as_u64` |  |
| Method | `fava_write::WriteId::from_u64` |  |
| Struct | `fava_write::WriteIntent` |  |
| Method | `fava_write::WriteIntent::author` |  |
| Method | `fava_write::WriteIntent::edit_as` |  |
| Method | `fava_write::WriteIntent::event` |  |
| Method | `fava_write::WriteIntent::into_parts` |  |
| Method | `fava_write::WriteIntent::payload` |  |
| Method | `fava_write::WriteIntent::presigned` |  |
| Method | `fava_write::WriteIntent::routing` |  |
| Enum | `fava_write::WriteIntentError` |  |
| Enum variant | `fava_write::WriteIntentError::DuplicateExplicitRelay` |  |
| Public field | `fava_write::WriteIntentError::DuplicateExplicitRelay::relay` |  |
| Enum variant | `fava_write::WriteIntentError::EmptyExplicitRelays` |  |
| Enum variant | `fava_write::WriteIntentError::Encoding` |  |
| Public field | `fava_write::WriteIntentError::Encoding::0` |  |
| Enum variant | `fava_write::WriteIntentError::Expired` |  |
| Enum variant | `fava_write::WriteIntentError::InvalidEvent` |  |
| Public field | `fava_write::WriteIntentError::InvalidEvent::0` |  |
| Enum variant | `fava_write::WriteIntentError::TooLarge` |  |
| Public field | `fava_write::WriteIntentError::TooLarge::bytes` |  |
| Public field | `fava_write::WriteIntentError::TooLarge::maximum` |  |
| Enum variant | `fava_write::WriteIntentError::TooManyExplicitRelays` |  |
| Public field | `fava_write::WriteIntentError::TooManyExplicitRelays::actual` |  |
| Public field | `fava_write::WriteIntentError::TooManyExplicitRelays::maximum` |  |
| Enum variant | `fava_write::WriteIntentError::TooManyTags` |  |
| Public field | `fava_write::WriteIntentError::TooManyTags::actual` |  |
| Public field | `fava_write::WriteIntentError::TooManyTags::maximum` |  |
| Method | `<fava_write::WriteIntentError as core::convert::From<fava_write::EventBuildError>>::from` |  |
| Enum | `fava_write::WritePayload` |  |
| Enum variant | `fava_write::WritePayload::Edit` |  |
| Public field | `fava_write::WritePayload::Edit::author` |  |
| Public field | `fava_write::WritePayload::Edit::edit` |  |
| Enum variant | `fava_write::WritePayload::Event` |  |
| Public field | `fava_write::WritePayload::Event::0` |  |
| Enum variant | `fava_write::WritePayload::Presigned` |  |
| Public field | `fava_write::WritePayload::Presigned::0` |  |
| Enum | `fava_write::WriteRouting` |  |
| Enum variant | `fava_write::WriteRouting::Automatic` |  |
| Enum variant | `fava_write::WriteRouting::Explicit` |  |
| Public field | `fava_write::WriteRouting::Explicit::0` |  |
| Method | `fava_write::WriteRouting::explicit` |  |
<!-- END crate-readme-api inventory -->
