//! Pure NIP-29 group values over ordinary Fava query and write contracts.

mod group;
mod query;

pub use group::{Group, GroupError};
pub use query::GroupRecords;
