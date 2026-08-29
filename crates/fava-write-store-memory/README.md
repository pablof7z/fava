# fava-write-store-memory

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_write_store_memory` |  |
| Struct | `fava_write_store_memory::MemoryWriteStore` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::accept` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::accept_applied_edit` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::accept_reserved_applied_edit` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::active_capacity` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::applied_edits` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::apply_route` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::authorize_signing` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::begin_attempt` |  |
| Method | `fava_write_store_memory::MemoryWriteStore::bounded` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::cancel` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as core::default::Default>::default` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::install_revision` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::install_signed` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::len` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_query::QuerySource>::open` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::receipt` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::receipt_changes` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::record_outcome` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::record_revision_failure` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::record_signer_refusal` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::record_signer_retryable` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::recover_applied_edits` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::recover_open` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::release_active` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::remove_receipt` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::reserve_active` |  |
| Method | `<fava_write_store_memory::MemoryWriteStore as fava_write_store::WriteStore>::signing_successor` |  |
<!-- END crate-readme-api inventory -->
