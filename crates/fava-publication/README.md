# fava-publication

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_publication` |  |
| Struct | `fava_publication::Publication` |  |
| Method | `fava_publication::Publication::accept` |  |
| Method | `fava_publication::Publication::cancel` |  |
| Method | `fava_publication::Publication::new` |  |
| Method | `fava_publication::Publication::preview_semantic_routes` |  |
| Method | `fava_publication::Publication::receipt` |  |
| Method | `fava_publication::Publication::recover` |  |
| Method | `fava_publication::Publication::remove_receipt` |  |
| Method | `fava_publication::Publication::stale_signer_completions` |  |
| Method | `fava_publication::Publication::wait_terminal` |  |
| Method | `fava_publication::Publication::wait_until` |  |
| Enum | `fava_publication::PublicationError` |  |
| Enum variant | `fava_publication::PublicationError::NotConfigured` |  |
| Enum variant | `fava_publication::PublicationError::ReceiptChangesClosed` |  |
| Enum variant | `fava_publication::PublicationError::ReceiptMissing` |  |
| Public field | `fava_publication::PublicationError::ReceiptMissing::0` |  |
| Enum variant | `fava_publication::PublicationError::Routing` |  |
| Public field | `fava_publication::PublicationError::Routing::0` |  |
| Enum variant | `fava_publication::PublicationError::RuntimeUnavailable` |  |
| Enum variant | `fava_publication::PublicationError::Store` |  |
| Public field | `fava_publication::PublicationError::Store::0` |  |
<!-- END crate-readme-api inventory -->
