use std::collections::BTreeMap;
use std::sync::Arc;

use fava_query::{OpenedQuerySource, Query, SourceEvent, SourceSnapshot};
use fava_routing::{RoutePlan, RouteRequest};
use fava_state::{EventCoordinate, RelayAccess, event_coordinate};
use fava_write::{
    Event, EventValue, Kind, ReceiptId, ReplaceableEventEdit, ReplaceableEventMaterializer,
    Timestamp, UnsignedEvent, WriteIntent, WritePayload, WriteRouting,
};

use super::{Publication, PublicationError};

pub(super) const MAX_MATERIALIZERS: usize = 64;

pub(super) struct OpenedSemanticSources {
    pub(super) cache: OpenedQuerySource,
    pub(super) writes: OpenedQuerySource,
}

impl OpenedSemanticSources {
    pub(super) fn snapshots(&self) -> [SourceSnapshot; 2] {
        [self.cache.initial.clone(), self.writes.initial.clone()]
    }

    pub(super) fn close(&mut self) {
        self.cache.changes.close();
        self.writes.changes.close();
    }
}

pub(super) struct PreparedSemantic {
    pub(super) event: UnsignedEvent,
    pub(super) source: Option<Event>,
    pub(super) route: RoutePlan,
    pub(super) sources: OpenedSemanticSources,
}

impl Publication {
    pub(super) fn index_materializers(
        materializers: impl IntoIterator<Item = Arc<dyn ReplaceableEventMaterializer>>,
    ) -> Result<BTreeMap<Kind, Arc<dyn ReplaceableEventMaterializer>>, PublicationError> {
        let mut indexed = BTreeMap::new();
        for materializer in materializers {
            if indexed.len() == MAX_MATERIALIZERS {
                return Err(PublicationError::Routing(format!(
                    "materializer selection exceeds bound: {} > {MAX_MATERIALIZERS}",
                    indexed.len() + 1
                )));
            }
            let kind = materializer.kind();
            if indexed.insert(kind, materializer).is_some() {
                return Err(PublicationError::Routing(format!(
                    "duplicate materializer for kind {}",
                    kind.as_u16()
                )));
            }
        }
        Ok(indexed)
    }

    pub(super) fn prepare_semantic(
        &self,
        intent: &WriteIntent,
        exclude: Option<(ReceiptId, fava_write::MaterializationId)>,
        current: Option<&EventValue>,
    ) -> Result<PreparedSemantic, PublicationError> {
        if self.store.active_capacity() == 0 {
            return Err(fava_write_store::WriteStoreError::Refused(
                "write store does not support replaceable-event edits".to_owned(),
            )
            .into());
        }
        let WritePayload::Edit(edit) = intent.payload() else {
            return Err(PublicationError::Routing(
                "semantic preparation requires a replaceable-event edit".to_owned(),
            ));
        };
        let materializer = self.materializer(edit)?;
        let mut sources = self.open_semantic_sources(edit)?;
        let snapshots = sources.snapshots();
        let source = match self.select_source(edit, &snapshots, exclude) {
            Ok(source) => source,
            Err(error) => {
                sources.close();
                return Err(error);
            }
        };
        let created_at = injected_timestamp(source.as_ref(), current)?;
        let event = match materializer.materialize(edit, source.as_ref(), created_at) {
            Ok(event) => event,
            Err(error) => {
                sources.close();
                return Err(PublicationError::Routing(format!(
                    "semantic materialization refused: {error}"
                )));
            }
        };
        if let Err(error) = validate_materialization(edit, &event, intent.routing()) {
            sources.close();
            return Err(error);
        }
        let route = match self.route_for(&event, intent.routing()) {
            Ok(route) => route,
            Err(error) => {
                sources.close();
                return Err(error);
            }
        };
        Ok(PreparedSemantic {
            event,
            source,
            route,
            sources,
        })
    }

    pub(super) fn materializer(
        &self,
        edit: &ReplaceableEventEdit,
    ) -> Result<Arc<dyn ReplaceableEventMaterializer>, PublicationError> {
        let kind = edit_kind(edit)?;
        let materializer = self.materializers.get(&kind).ok_or_else(|| {
            PublicationError::Routing(format!(
                "no selected materializer for kind {}",
                kind.as_u16()
            ))
        })?;
        if !materializer.supports(edit) {
            return Err(PublicationError::Routing(format!(
                "selected materializer does not support kind {} edit format {}",
                kind.as_u16(),
                edit.format()
            )));
        }
        Ok(Arc::clone(materializer))
    }

    pub(super) fn open_semantic_sources(
        &self,
        edit: &ReplaceableEventEdit,
    ) -> Result<OpenedSemanticSources, PublicationError> {
        let query = exact_query(edit)?;
        let cache = self
            .event_source
            .open(&query)
            .map_err(|error| PublicationError::Routing(error.to_string()))?;
        let writes = match self.store.open(&query) {
            Ok(writes) => writes,
            Err(error) => {
                let mut changes = cache.changes;
                changes.close();
                return Err(PublicationError::Routing(error.to_string()));
            }
        };
        Ok(OpenedSemanticSources { cache, writes })
    }

    pub(super) fn select_source(
        &self,
        edit: &ReplaceableEventEdit,
        sources: &[SourceSnapshot],
        exclude: Option<(ReceiptId, fava_write::MaterializationId)>,
    ) -> Result<Option<Event>, PublicationError> {
        let mut qualified = sources.to_vec();
        for source in &mut qualified {
            source.events.retain(|event| match event {
                SourceEvent::Cached(_) => true,
                SourceEvent::Local(local) => {
                    matches!(local.event, EventValue::Signed(_))
                        && exclude.is_none_or(|(receipt_id, materialization_id)| {
                            local.publication.receipt_id != receipt_id
                                || local.publication.materialization_id != materialization_id
                        })
                }
            });
        }
        let snapshot = self
            .evaluator
            .evaluate(&exact_query(edit)?, &qualified)
            .map_err(|error| PublicationError::Routing(error.to_string()))?;
        Ok(snapshot
            .events
            .first()
            .and_then(|record| match &record.event {
                EventValue::Signed(event) => Some(event.clone()),
                EventValue::Unsigned(_) => None,
            }))
    }

    pub(super) fn route_for(
        &self,
        event: &UnsignedEvent,
        routing: &WriteRouting,
    ) -> Result<RoutePlan, PublicationError> {
        let request = RouteRequest::Write(EventValue::Unsigned(event.clone()));
        match routing {
            WriteRouting::Explicit(relays) => RoutePlan::explicit(
                relays.iter().cloned(),
                &RelayAccess::public(),
                &request.targets(),
            )
            .map_err(|error| PublicationError::Routing(error.to_string())),
            WriteRouting::Automatic => fava_routing::preview(self.routers.as_slice(), &request)
                .map_err(|error| PublicationError::Routing(error.to_string())),
        }
    }
}

pub(super) fn edit_kind(edit: &ReplaceableEventEdit) -> Result<Kind, PublicationError> {
    match edit.coordinate() {
        EventCoordinate::Replaceable {
            kind,
            identifier: None,
            ..
        } => Ok(*kind),
        EventCoordinate::Replaceable {
            identifier: Some(_),
            ..
        } => Err(PublicationError::Routing(
            "addressable replaceable-event edits are not supported".to_owned(),
        )),
        EventCoordinate::Event(_) => Err(PublicationError::Routing(
            "semantic edit requires a replaceable coordinate".to_owned(),
        )),
    }
}

fn exact_query(edit: &ReplaceableEventEdit) -> Result<Query, PublicationError> {
    Ok(Query::events()
        .authors([edit.actor()])
        .kind(edit_kind(edit)?)
        .cache_only())
}

pub(super) fn injected_timestamp(
    source: Option<&Event>,
    current: Option<&EventValue>,
) -> Result<Timestamp, PublicationError> {
    let newest = source
        .map(|event| event.created_at.as_secs())
        .into_iter()
        .chain(current.map(|event| event.created_at().as_secs()))
        .max();
    let minimum = match newest {
        Some(timestamp) => timestamp.checked_add(1).ok_or_else(|| {
            PublicationError::Routing("materialization timestamp exhausted".to_owned())
        })?,
        None => 0,
    };
    Ok(Timestamp::from(Timestamp::now().as_secs().max(minimum)))
}

pub(super) fn validate_materialization(
    edit: &ReplaceableEventEdit,
    event: &UnsignedEvent,
    routing: &WriteRouting,
) -> Result<(), PublicationError> {
    WriteIntent::event(event.clone(), routing.clone()).map_err(|error| {
        PublicationError::Routing(format!("semantic materialization refused: {error}"))
    })?;
    let id = event.id.ok_or_else(|| {
        PublicationError::Routing("semantic materialization has no event id".to_owned())
    })?;
    let coordinate = event_coordinate(id, event.pubkey, event.kind, event.tags.as_slice());
    if event.pubkey != edit.actor() || coordinate != *edit.coordinate() {
        return Err(PublicationError::Routing(
            "semantic materialization actor or coordinate does not match edit".to_owned(),
        ));
    }
    Ok(())
}
