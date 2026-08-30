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
| **`bookmark_coordinate`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::bookmark_coordinate","signature":"pub fn fava_bookmarks::bookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::EventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::bookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::EventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
| **`bookmark_event`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::bookmark_event","signature":"pub fn fava_bookmarks::bookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::EventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::bookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::EventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
| **`unbookmark_coordinate`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::unbookmark_coordinate","signature":"pub fn fava_bookmarks::unbookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::EventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::unbookmark_coordinate(fava_state::EventCoordinate) -> core::result::Result<fava_write::edit::EventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |
| **`unbookmark_event`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_bookmarks::unbookmark_event","signature":"pub fn fava_bookmarks::unbookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::EventEdit, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::unbookmark_event(nostr::event::id::EventId) -> core::result::Result<fava_write::edit::EventEdit, fava_write::WriteIntentError>"} --> | Compiler-visible function owned by `fava_bookmarks`. |

### `Bookmarks` (Trait)

Compiler-visible trait `fava_bookmarks::Bookmarks`.
<!-- api-item {"kind":"Trait","item":"fava_bookmarks::Bookmarks","signature":"pub trait fava_bookmarks::Bookmarks: core::marker::Sized","evidence":"cargo-public-api@0.52.0: pub trait fava_bookmarks::Bookmarks: core::marker::Sized"} -->

| Item | Purpose |
| --- | --- |
| **`with_bookmarks`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_bookmarks::Bookmarks::with_bookmarks","signature":"pub fn fava_bookmarks::Bookmarks::with_bookmarks(self) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_bookmarks::Bookmarks::with_bookmarks(self) -> Self"} --> | Compiler-visible method owned by `fava_bookmarks::Bookmarks`. |
<!-- END crate-readme-api inventory -->
