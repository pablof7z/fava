# fava-bookmarks

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_bookmarks` (Module)

Compiler-visible module `fava_bookmarks`.
<!-- api-item {"kind":"Module","item":"fava_bookmarks","signature":"pub mod fava_bookmarks","evidence":"cargo-public-api@0.52.0: pub mod fava_bookmarks"} -->

| Item | Purpose |
| --- | --- |
| **`bookmark_coordinate`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::bookmark_coordinate","signature":"pub fn fava_bookmarks::bookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::bookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
| **`bookmark_event`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::bookmark_event","signature":"pub fn fava_bookmarks::bookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::bookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
| **`unbookmark_coordinate`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::unbookmark_coordinate","signature":"pub fn fava_bookmarks::unbookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::unbookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
| **`unbookmark_event`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::unbookmark_event","signature":"pub fn fava_bookmarks::unbookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::unbookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |

### `fava_bookmarks::__fava` (Module)

Doc-hidden Fava facade bridge; applications use bookmark edit constructors instead.
<!-- api-item {"kind":"Module","item":"fava_bookmarks::__fava","signature":"pub mod fava_bookmarks::__fava","evidence":"cargo-public-api@0.52.0: pub mod fava_bookmarks::__fava"} -->

| Item | Purpose |
| --- | --- |
| **`materializer`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::__fava::materializer","signature":"pub fn fava_bookmarks::__fava::materializer() -> alloc::sync::Arc<dyn fava_write::materialization::ReplaceableEventMaterializer>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::__fava::materializer() -> alloc::sync::Arc<dyn fava_write::materialization::ReplaceableEventMaterializer>"} --> | Instantiates Fava's private bookmark codec. |
<!-- END crate-readme-api inventory -->
