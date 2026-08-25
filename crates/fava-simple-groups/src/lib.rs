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

#[cfg(test)]
mod tests;

pub use metadata::SimpleGroupMetadata;
pub use people::SimpleGroupAdmins;
pub use people::SimpleGroupMembers;
pub use people::SimpleGroupParticipants;
pub use people::SimpleGroupRoles;
pub use pins::{PinnedItem, SimpleGroupPins};
pub use query::{SimpleGroupRecords, SimpleGroups};
pub use saved::{SavedRelay, SavedSimpleGroup};
pub use simple_group::{SimpleGroup, SimpleGroupError};
