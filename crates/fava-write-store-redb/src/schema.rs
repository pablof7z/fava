use std::collections::BTreeMap;
use std::num::NonZeroU64;

use fava_state::EventCoordinate;
use fava_write::{EventId, PublicKey, Receipt, ReceiptId, EventEdit, Timestamp};
use fava_write_store::WriteStoreError;
use redb::{Database, Durability, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::{SemanticCustody, StoreState, refused, validation};

const RECEIPTS: TableDefinition<u64, &[u8]> = TableDefinition::new("receipts");
/// Redb table holding the on-disk schema version and the next receipt identity.
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
const NEXT_ID: &str = "next_id";
const SCHEMA_VERSION_KEY: &str = "schema_version";
const SCHEMA_VERSION: u64 = 4;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedReceipt {
    receipt: Receipt,
    semantic: Option<PersistedSemantic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedSemantic {
    edits: Vec<EventEdit>,
    author: PublicKey,
    current_source: Option<(EventId, Timestamp)>,
    failed_source: Option<EventId>,
    #[allow(clippy::type_complexity)] // Mirrors universal custody without a second persisted noun.
    successor: Option<(
        Option<EventEdit>,
        fava_write::UnsignedEvent,
        Option<(EventId, Timestamp)>,
        Option<fava_routing::RoutePlan>,
    )>,
}

impl PersistedReceipt {
    fn from_current(receipt: &Receipt, semantic: Option<&SemanticCustody>) -> Self {
        Self {
            receipt: receipt.clone(),
            semantic: semantic.map(
                |(edits, author, current_source, failed_source, successor)| PersistedSemantic {
                    edits: edits.clone(),
                    author: *author,
                    current_source: *current_source,
                    failed_source: *failed_source,
                    successor: successor.clone(),
                },
            ),
        }
    }
}

pub(super) fn initialize(database: &Database, is_new: bool) -> Result<(), WriteStoreError> {
    if !is_new {
        return validate_schema(database);
    }
    let mut transaction = database.begin_write().map_err(refused)?;
    transaction
        .set_durability(Durability::Immediate)
        .map_err(refused)?;
    transaction.open_table(RECEIPTS).map_err(refused)?;
    {
        let mut meta = transaction.open_table(META).map_err(refused)?;
        meta.insert(NEXT_ID, 1_u64).map_err(refused)?;
        meta.insert(SCHEMA_VERSION_KEY, SCHEMA_VERSION)
            .map_err(refused)?;
    }
    transaction.commit().map_err(refused)
}

fn validate_schema(database: &Database) -> Result<(), WriteStoreError> {
    let transaction = database.begin_read().map_err(refused)?;
    let meta = transaction.open_table(META).map_err(refused)?;
    let version = meta
        .get(SCHEMA_VERSION_KEY)
        .map_err(refused)?
        .ok_or_else(|| WriteStoreError::Refused("write-store schema version missing".to_owned()))?
        .value();
    if version != SCHEMA_VERSION {
        return Err(WriteStoreError::Refused(format!(
            "write-store schema version mismatch: {version} != {SCHEMA_VERSION}"
        )));
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
pub(super) fn load(
    database: &Database,
) -> Result<
    (
        NonZeroU64,
        BTreeMap<ReceiptId, Receipt>,
        BTreeMap<EventCoordinate, ReceiptId>,
        BTreeMap<ReceiptId, SemanticCustody>,
    ),
    WriteStoreError,
> {
    let transaction = database.begin_read().map_err(refused)?;
    let next_identity = NonZeroU64::new(
        transaction
            .open_table(META)
            .map_err(refused)?
            .get(NEXT_ID)
            .map_err(refused)?
            .ok_or_else(|| WriteStoreError::Refused("write identity metadata missing".to_owned()))?
            .value(),
    )
    .ok_or_else(|| WriteStoreError::Refused("durable next identity is zero".to_owned()))?;
    let table = transaction.open_table(RECEIPTS).map_err(refused)?;
    let mut receipts = BTreeMap::new();
    let mut coordinates = BTreeMap::new();
    let mut semantics = BTreeMap::new();
    for entry in table.iter().map_err(refused)? {
        let (key, value) = entry.map_err(refused)?;
        let row: PersistedReceipt = serde_json::from_slice(value.value()).map_err(refused)?;
        let receipt_id = ReceiptId::try_from(key.value())
            .map_err(|_| WriteStoreError::Refused("durable receipt identity is zero".to_owned()))?;
        if row.receipt.receipt_id != receipt_id || row.receipt.write_id.as_u64() != key.value() {
            return Err(WriteStoreError::Refused(
                "durable receipt identity does not match its row".to_owned(),
            ));
        }
        if let Some(semantic) = row.semantic {
            if row.receipt.current.publication.revision_source
                != semantic.current_source.map(|(id, _)| id)
            {
                return Err(WriteStoreError::Refused(
                    "durable semantic custody is incoherent".to_owned(),
                ));
            }
            if !row.receipt.is_terminal()
                && coordinates
                    .insert(
                        crate::semantic::edit_coordinate(
                            semantic.edits.last().ok_or_else(|| {
                                WriteStoreError::Refused(
                                    "durable semantic edit sequence is empty".to_owned(),
                                )
                            })?,
                            semantic.author,
                        ),
                        receipt_id,
                    )
                    .is_some()
            {
                return Err(WriteStoreError::Refused(
                    "duplicate durable semantic coordinate owner".to_owned(),
                ));
            }
            semantics.insert(
                receipt_id,
                (
                    semantic.edits,
                    semantic.author,
                    semantic.current_source,
                    semantic.failed_source,
                    semantic.successor,
                ),
            );
        }
        if receipts.insert(receipt_id, row.receipt).is_some() {
            return Err(WriteStoreError::Refused(
                "duplicate durable receipt identity".to_owned(),
            ));
        }
    }
    validation::reconstructed(next_identity, &receipts, &semantics)?;
    Ok((next_identity, receipts, coordinates, semantics))
}

pub(super) fn commit_accept(
    database: &Database,
    next_identity: NonZeroU64,
    receipt: &Receipt,
    semantic: Option<&SemanticCustody>,
    removals: &[ReceiptId],
) -> Result<(), WriteStoreError> {
    commit(
        database,
        Some(next_identity),
        Some((receipt, semantic)),
        removals,
    )
}

pub(super) fn commit_update(
    database: &Database,
    receipt: Option<&Receipt>,
    semantic: Option<&SemanticCustody>,
    removals: &[ReceiptId],
) -> Result<(), WriteStoreError> {
    commit(
        database,
        None,
        receipt.map(|receipt| (receipt, semantic)),
        removals,
    )
}

pub(super) fn persist_existing(
    database: &Database,
    state: &StoreState,
    receipt_ids: &[ReceiptId],
) -> Result<(), WriteStoreError> {
    let mut transaction = database.begin_write().map_err(refused)?;
    transaction
        .set_durability(Durability::Immediate)
        .map_err(refused)?;
    {
        let mut table = transaction.open_table(RECEIPTS).map_err(refused)?;
        for receipt_id in receipt_ids {
            let receipt = state.receipts.get(receipt_id).ok_or_else(|| {
                WriteStoreError::Refused("recovered receipt disappeared".to_owned())
            })?;
            write_row(&mut table, receipt, state.semantics.get(receipt_id))?;
        }
    }
    transaction.commit().map_err(refused)
}

fn commit(
    database: &Database,
    next_identity: Option<NonZeroU64>,
    receipt: Option<(&Receipt, Option<&SemanticCustody>)>,
    removals: &[ReceiptId],
) -> Result<(), WriteStoreError> {
    let mut transaction = database.begin_write().map_err(refused)?;
    transaction
        .set_durability(Durability::Immediate)
        .map_err(refused)?;
    {
        let mut table = transaction.open_table(RECEIPTS).map_err(refused)?;
        if let Some((receipt, semantic)) = receipt {
            write_row(&mut table, receipt, semantic)?;
        }
        for id in removals {
            table.remove(id.as_u64()).map_err(refused)?;
        }
    }
    if let Some(next_identity) = next_identity {
        transaction
            .open_table(META)
            .map_err(refused)?
            .insert(NEXT_ID, next_identity.get())
            .map_err(refused)?;
    }
    transaction.commit().map_err(refused)
}

fn write_row(
    table: &mut redb::Table<'_, u64, &[u8]>,
    receipt: &Receipt,
    semantic: Option<&SemanticCustody>,
) -> Result<(), WriteStoreError> {
    let bytes =
        serde_json::to_vec(&PersistedReceipt::from_current(receipt, semantic)).map_err(refused)?;
    table
        .insert(receipt.receipt_id.as_u64(), bytes.as_slice())
        .map_err(refused)?;
    Ok(())
}
