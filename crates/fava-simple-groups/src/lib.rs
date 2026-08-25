#![doc = include_str!("../README.md")]
//! Pure simple-group domain composition over ordinary Fava query and write contracts.

mod edit;
mod metadata;
mod people;
mod pins;
mod query;
mod records;
mod saved;
mod simple_group;

#[cfg(test)]
mod tests;

pub use edit::{
    remove_saved_relay, remove_saved_simple_group, rename_saved_simple_group, save_relay,
    save_simple_group, saved_group_list_materializer,
};
pub use metadata::SimpleGroupMetadata;
pub use people::{
    SimpleGroupAdmins, SimpleGroupLivekitParticipants, SimpleGroupMembers, SimpleGroupRoles,
};
pub use pins::SimpleGroupPins;
pub use query::{SimpleGroupStateEventKind, saved_group_lists};
pub use records::SimpleGroupDecodeError;
pub use saved::{SavedGroupList, SavedGroupListDecodeError, SavedSimpleGroup};
pub use simple_group::SimpleGroup;
