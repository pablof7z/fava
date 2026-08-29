//! External compile-surface proof for the public-bookmark capability crate.

use std::sync::Arc;

use fava_state::EventCoordinate;
use fava_write::{
    EventId, Kind, EventEdit, EditApplier, WriteIntentError,
};

type EditResult = Result<EventEdit, WriteIntentError>;
type EventTargetEdit = fn(EventId) -> EditResult;
type CoordinateEdit = fn(EventCoordinate) -> EditResult;
type Selection = fn() -> Arc<dyn EditApplier>;

const BOOKMARK_EVENT: EventTargetEdit = fava_bookmarks::bookmark_event;
const UNBOOKMARK_EVENT: EventTargetEdit = fava_bookmarks::unbookmark_event;
const BOOKMARK_COORDINATE: CoordinateEdit = fava_bookmarks::bookmark_coordinate;
const UNBOOKMARK_COORDINATE: CoordinateEdit = fava_bookmarks::unbookmark_coordinate;
const APPLIER: Selection = fava_bookmarks::applier;

#[test]
fn external_surface_uses_only_approved_functions_and_types() {
    let event_functions: [EventTargetEdit; 2] = [BOOKMARK_EVENT, UNBOOKMARK_EVENT];
    let coordinate_functions: [CoordinateEdit; 2] = [BOOKMARK_COORDINATE, UNBOOKMARK_COORDINATE];
    assert_eq!(event_functions.len(), 2);
    assert_eq!(coordinate_functions.len(), 2);
    assert_eq!(APPLIER().kind(), Kind::from_u16(10_003));
}
