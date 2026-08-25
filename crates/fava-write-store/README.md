# fava-write-store

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_write_store` |  |
| Struct | `fava_write_store::AcceptedWrite` |  |
| Public field | `fava_write_store::AcceptedWrite::current` |  |
| Public field | `fava_write_store::AcceptedWrite::receipt_id` |  |
| Public field | `fava_write_store::AcceptedWrite::write_id` |  |
| Trait | `fava_write_store::WriteStore` |  |
| Method | `fava_write_store::WriteStore::accept` |  |
| Method | `fava_write_store::WriteStore::accept_materialized` |  |
| Method | `fava_write_store::WriteStore::accept_materialized_edit` |  |
| Method | `fava_write_store::WriteStore::accept_reserved_materialized_edit` |  |
| Method | `fava_write_store::WriteStore::active_capacity` |  |
| Method | `fava_write_store::WriteStore::apply_route` |  |
| Method | `fava_write_store::WriteStore::authorize_signing` |  |
| Method | `fava_write_store::WriteStore::begin_attempt` |  |
| Method | `fava_write_store::WriteStore::cancel` |  |
| Method | `fava_write_store::WriteStore::install_materialization` |  |
| Method | `fava_write_store::WriteStore::install_signed` |  |
| Method | `fava_write_store::WriteStore::is_empty` |  |
| Method | `fava_write_store::WriteStore::len` |  |
| Method | `fava_write_store::WriteStore::materialized_edits` |  |
| Method | `fava_write_store::WriteStore::receipt` |  |
| Method | `fava_write_store::WriteStore::receipt_changes` |  |
| Method | `fava_write_store::WriteStore::receipt_event` |  |
| Method | `fava_write_store::WriteStore::record_materialization_failure` |  |
| Method | `fava_write_store::WriteStore::record_outcome` |  |
| Method | `fava_write_store::WriteStore::record_signer_refusal` |  |
| Method | `fava_write_store::WriteStore::record_signer_retryable` |  |
| Method | `fava_write_store::WriteStore::recover_materialized_edits` |  |
| Method | `fava_write_store::WriteStore::recover_open` |  |
| Method | `fava_write_store::WriteStore::release_active` |  |
| Method | `fava_write_store::WriteStore::remove_receipt` |  |
| Method | `fava_write_store::WriteStore::reserve_active` |  |
| Method | `fava_write_store::WriteStore::signing_successor` |  |
| Enum | `fava_write_store::WriteStoreError` |  |
| Enum variant | `fava_write_store::WriteStoreError::Closed` |  |
| Enum variant | `fava_write_store::WriteStoreError::InvalidEvent` |  |
| Public field | `fava_write_store::WriteStoreError::InvalidEvent::0` |  |
| Enum variant | `fava_write_store::WriteStoreError::InvalidIntent` |  |
| Public field | `fava_write_store::WriteStoreError::InvalidIntent::0` |  |
| Enum variant | `fava_write_store::WriteStoreError::Refused` |  |
| Public field | `fava_write_store::WriteStoreError::Refused::0` |  |
| Function | `fava_write_store::apply_route_to_receipt` |  |
| Function | `fava_write_store::destination_evidence_capacity` |  |
| Function | `fava_write_store::validate_current_materialization` |  |
| Function | `fava_write_store::validate_delivery_outcome` |  |
| Function | `fava_write_store::validate_receipt_text` |  |
<!-- END crate-readme-api inventory -->
