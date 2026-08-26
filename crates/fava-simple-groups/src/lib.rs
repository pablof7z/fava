//! Implements NIP-29 simple-group semantics and the kind-10009 Simple Group List.
#![doc = include_str!("../README.md")]

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

pub use edit::{
    remove_saved_relay, remove_saved_simple_group, rename_saved_simple_group, save_relay,
    save_simple_group, saved_group_list_materializer,
};
pub use management::{
    GroupAccess, GroupVisibility, MetadataEdit, create_group, create_subgroup, delete_event,
    delete_group, edit_metadata, invite, join_request, leave_group, put_user, remove_user,
};
pub use metadata::SimpleGroupMetadata;
pub use people::{
    SimpleGroupAdmins, SimpleGroupLivekitParticipants, SimpleGroupMembers, SimpleGroupRoles,
};
pub use pins::SimpleGroupPins;
pub use query::{SimpleGroupStateEventKind, saved_group_lists};
pub use records::SimpleGroupDecodeError;
pub use saved::{SavedGroupList, SavedGroupListDecodeError, SavedSimpleGroup};
pub use simple_group::{SimpleGroup, SimpleGroupConstructionError, SimpleGroupEventBuilder};
