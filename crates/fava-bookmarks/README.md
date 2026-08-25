# fava-bookmarks

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_bookmarks` (Module)

Compiler-visible module `fava_bookmarks`.
<!-- api-item {"kind":"Module","item":"fava_bookmarks","signature":"pub mod fava_bookmarks","evidence":"cargo-public-api@0.52.0: pub mod fava_bookmarks"} -->

| Item | Purpose |
| --- | --- |
| **`bookmark_coordinate`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::bookmark_coordinate","signature":"pub fn fava_bookmarks::bookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::bookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
| **`bookmark_event`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::bookmark_event","signature":"pub fn fava_bookmarks::bookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::bookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
| **`materializer`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::materializer","signature":"pub fn fava_bookmarks::materializer() -> alloc::sync::Arc<dyn fava_write::materialization::ReplaceableEventMaterializer>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::materializer() -> alloc::sync::Arc<dyn fava_write::materialization::ReplaceableEventMaterializer>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
| **`unbookmark_coordinate`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::unbookmark_coordinate","signature":"pub fn fava_bookmarks::unbookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::unbookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
| **`unbookmark_event`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::unbookmark_event","signature":"pub fn fava_bookmarks::unbookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::unbookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
<!-- END crate-readme-api inventory -->
