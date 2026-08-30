//! External compile-surface proof for the public-bookmark capability crate.

use std::sync::Arc;

use fava_state::EventCoordinate;
use fava_write::{EditApplier, EditApplierSink, EventEdit, EventId, Kind, WriteIntentError};

use fava_bookmarks::Bookmarks;

type EditResult = Result<EventEdit, WriteIntentError>;
type EventTargetEdit = fn(EventId) -> EditResult;
type CoordinateEdit = fn(EventCoordinate) -> EditResult;

const BOOKMARK_EVENT: EventTargetEdit = fava_bookmarks::bookmark_event;
const UNBOOKMARK_EVENT: EventTargetEdit = fava_bookmarks::unbookmark_event;
const BOOKMARK_COORDINATE: CoordinateEdit = fava_bookmarks::bookmark_coordinate;
const UNBOOKMARK_COORDINATE: CoordinateEdit = fava_bookmarks::unbookmark_coordinate;

#[derive(Default)]
struct RecordingSink {
    appliers: Vec<Arc<dyn EditApplier>>,
}

impl EditApplierSink for RecordingSink {
    fn accept(mut self, applier: Arc<dyn EditApplier>) -> Self {
        self.appliers.push(applier);
        self
    }
}

#[test]
fn external_surface_uses_only_approved_functions_and_types() {
    let event_functions: [EventTargetEdit; 2] = [BOOKMARK_EVENT, UNBOOKMARK_EVENT];
    let coordinate_functions: [CoordinateEdit; 2] = [BOOKMARK_COORDINATE, UNBOOKMARK_COORDINATE];
    assert_eq!(event_functions.len(), 2);
    assert_eq!(coordinate_functions.len(), 2);

    let sink = RecordingSink::default().with_bookmarks();
    assert_eq!(sink.appliers.len(), 1);
    assert_eq!(sink.appliers[0].kind(), Kind::from_u16(10_003));
}
