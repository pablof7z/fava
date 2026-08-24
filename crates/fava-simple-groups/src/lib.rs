//! Pure NIP-29 simple group values over ordinary Fava query and write contracts.

mod bounds;
mod edit;
mod management;
mod metadata;
mod people;
mod pins;
mod query;
mod records;
mod saved;
mod simple_group;
mod snapshot;

#[cfg(test)]
mod tests;

pub use metadata::SimpleGroupMetadata;
pub use people::{SimpleGroupAdmins, SimpleGroupMembers, SimpleGroupParticipants, SimpleGroupRoles};
pub use pins::{PinnedItem, SimpleGroupPins};
pub use query::{SimpleGroupRecords, SimpleGroups};
pub use saved::{SavedRelay, SavedSimpleGroup};
pub use simple_group::{SimpleGroup, SimpleGroupError};
pub use snapshot::SimpleGroupSnapshot;
