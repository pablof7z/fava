//! Pure NIP-29 group values over ordinary Fava query and write contracts.

mod bounds;
mod edit;
mod group;
mod metadata;
mod people;
mod pins;
mod query;
mod records;
mod saved;
mod snapshot;

#[cfg(test)]
mod tests;

pub use group::{Group, GroupError};
pub use metadata::GroupMetadata;
pub use people::{GroupAdmins, GroupMembers, GroupParticipants, GroupRoles};
pub use pins::{GroupPins, PinnedItem};
pub use query::{GroupRecords, SimpleGroups};
pub use saved::{SavedGroup, SavedRelay};
pub use snapshot::GroupSnapshot;
