//! Pure NIP-02 contact-list values and semantic edits.
//!
//! The package README is the application-facing API and developer-experience
//! North Star.
//!
//! The compile-fail examples are external privacy checks for protocol nouns.
//!
//! ```compile_fail
//! use fava_nip02::Change;
//! ```
//!
//! ```compile_fail
//! use fava_nip02::Nip02Materializer;
//! ```
//!
//! ```compile_fail
//! use fava_nip02::IntoContactAuthors;
//!
//! struct ForeignAuthors;
//!
//! impl IntoContactAuthors for ForeignAuthors {
//!     type IntoIter = std::iter::Empty<fava_write::PublicKey>;
//!
//!     fn into_contact_authors(self) -> Self::IntoIter {
//!         std::iter::empty()
//!     }
//! }
//! ```

mod bounds;
mod contact_list;
mod edit;
mod query;

pub use contact_list::{ContactList, ContactListError, ContactListRowEvidence, Follow};
pub use edit::{follow, follow_with, materializer, unfollow};
pub use query::{IntoContactAuthors, contact_list, followers_of, follows_of};

#[cfg(test)]
mod tests;
