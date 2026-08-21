use std::sync::Arc;

use fava_state::EventCoordinate;
use fava_write::{
    EventId, Kind, PublicKey, ReplaceableEventEdit, ReplaceableEventMaterializer, WriteIntentError,
};

type EditResult = Result<ReplaceableEventEdit, WriteIntentError>;
type EventEdit = fn(PublicKey, EventId) -> EditResult;
type CoordinateEdit = fn(PublicKey, EventCoordinate) -> EditResult;
type Selection = fn() -> Arc<dyn ReplaceableEventMaterializer>;

const BOOKMARK_EVENT: EventEdit = fava_bookmarks::bookmark_event;
const UNBOOKMARK_EVENT: EventEdit = fava_bookmarks::unbookmark_event;
const BOOKMARK_COORDINATE: CoordinateEdit = fava_bookmarks::bookmark_coordinate;
const UNBOOKMARK_COORDINATE: CoordinateEdit = fava_bookmarks::unbookmark_coordinate;
const MATERIALIZER: Selection = fava_bookmarks::materializer;

#[test]
fn external_surface_uses_only_approved_functions_and_types() {
    let _event_functions: [EventEdit; 2] = [BOOKMARK_EVENT, UNBOOKMARK_EVENT];
    let _coordinate_functions: [CoordinateEdit; 2] = [BOOKMARK_COORDINATE, UNBOOKMARK_COORDINATE];
    assert_eq!(MATERIALIZER().kind(), Kind::from_u16(10_003));
}
