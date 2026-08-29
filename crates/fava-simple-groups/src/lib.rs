//! Implements NIP-29 simple-group semantics and the kind-10009 Simple Group List.
//!
//! ```compile_fail
//! use fava_simple_groups::saved_group_list_materializer;
//! ```
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

#[cfg(test)]
pub(crate) use edit::materializer;
pub use edit::{
    remove_saved_relay, remove_saved_simple_group, rename_saved_simple_group, save_relay,
    save_simple_group,
};
pub use management::{
    GroupAccess, GroupVisibility, MetadataEdit, create_group, delete_event, delete_group,
    edit_metadata, invite, join_request, leave_group, put_user, remove_user,
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

/// Doc-hidden Fava facade integration; not application API.
#[doc(hidden)]
pub mod __fava {
    use std::sync::Arc;

    use fava_write::ReplaceableEventMaterializer;

    #[must_use]
    pub fn materializer() -> Arc<dyn ReplaceableEventMaterializer> {
        super::edit::materializer()
    }
}
