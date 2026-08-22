//! Pure NIP-29 group values over ordinary Fava query and write contracts.

mod bounds;
mod group;
mod metadata;
mod people;
mod query;
mod records;

#[cfg(test)]
mod tests;

pub use group::{Group, GroupError};
pub use metadata::GroupMetadata;
pub use people::{GroupAdmins, GroupMembers, GroupParticipants, GroupRoles};
pub use query::GroupRecords;
