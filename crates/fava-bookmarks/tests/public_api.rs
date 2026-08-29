//! External compile-surface proof for the public-bookmark capability crate.

use fava_state::EventCoordinate;
use fava_write::{EventId, ReplaceableEventEdit, WriteIntentError};

type EditResult = Result<ReplaceableEventEdit, WriteIntentError>;
type EventEdit = fn(EventId) -> EditResult;
type CoordinateEdit = fn(EventCoordinate) -> EditResult;

const BOOKMARK_EVENT: EventEdit = fava_bookmarks::bookmark_event;
const UNBOOKMARK_EVENT: EventEdit = fava_bookmarks::unbookmark_event;
const BOOKMARK_COORDINATE: CoordinateEdit = fava_bookmarks::bookmark_coordinate;
const UNBOOKMARK_COORDINATE: CoordinateEdit = fava_bookmarks::unbookmark_coordinate;

#[test]
fn external_surface_uses_only_approved_functions_and_types() {
    let event_functions: [EventEdit; 2] = [BOOKMARK_EVENT, UNBOOKMARK_EVENT];
    let coordinate_functions: [CoordinateEdit; 2] = [BOOKMARK_COORDINATE, UNBOOKMARK_COORDINATE];
    assert_eq!(event_functions.len(), 2);
    assert_eq!(coordinate_functions.len(), 2);
}
