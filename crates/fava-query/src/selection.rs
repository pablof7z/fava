//! [`FilterSelection`], the declarative axes a literal [`Query`] is built from,
//! and the `Query` builder methods that populate them.

use std::collections::{BTreeMap, BTreeSet};

pub use nostr::filter::SingleLetterTag;

use crate::{
    EventId, Kind, PublicKey, Query, QueryError, bounded_authors, bounded_ids, bounded_kinds,
    bounded_tag_values, extend_bounded,
};

/// Declarative event-filter axes supported by literal query selection.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct FilterSelection {
    /// Event ids, or all ids when absent. A present empty set matches nothing.
    pub ids: Option<BTreeSet<EventId>>,
    /// Authors, or all authors when absent. A present empty set matches nothing.
    pub authors: Option<BTreeSet<PublicKey>>,
    /// Kinds, or all kinds when absent. A present empty set matches nothing.
    pub kinds: Option<BTreeSet<Kind>>,
    /// Exact strings accepted for each case-sensitive one-letter tag key.
    ///
    /// An absent key is unconstrained. A present empty set matches nothing.
    pub tag_values: BTreeMap<SingleLetterTag, BTreeSet<String>>,
}

impl Query {
    /// Start an unconstrained event query.
    #[must_use]
    pub fn events() -> Self {
        Self::default()
    }

    /// Match a literal author set. An empty set intentionally matches nothing.
    ///
    /// # Errors
    ///
    /// Returns [`QueryError`] for empty relay sets or zero limits.
    pub fn authors(
        mut self,
        authors: impl IntoIterator<Item = PublicKey>,
    ) -> Result<Self, QueryError> {
        self.selection.authors = Some(bounded_authors(authors));
        Ok(self)
    }

    /// Match a literal kind set. An empty set intentionally matches nothing.
    ///
    /// # Errors
    ///
    /// Returns [`QueryError`] for empty relay sets or zero limits.
    pub fn kinds(mut self, kinds: impl IntoIterator<Item = Kind>) -> Result<Self, QueryError> {
        self.selection.kinds = Some(bounded_kinds(kinds));
        Ok(self)
    }

    /// Match a literal event-id set. An empty set intentionally matches nothing.
    ///
    /// # Errors
    ///
    /// Returns [`QueryError`] for empty relay sets or zero limits.
    pub fn ids(mut self, ids: impl IntoIterator<Item = EventId>) -> Result<Self, QueryError> {
        self.selection.ids = Some(bounded_ids(ids));
        Ok(self)
    }

    /// Match one case-sensitive Nostr tag axis against the exact strings supplied.
    ///
    /// Repeated calls for the same key union values. An empty iterator retains
    /// a present tag axis that intentionally matches nothing.
    ///
    /// # Arguments
    ///
    /// * `key` - the case-sensitive single-letter tag key to constrain
    /// * `values` - the exact values that satisfy this tag axis
    ///
    /// # Errors
    ///
    /// Returns [`QueryError`] for empty relay sets or zero limits.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fava_query::{Query, SingleLetterTag};
    /// let e = SingleLetterTag::from_char('e').expect("valid single-letter tag key");
    /// let query = Query::events()
    ///     .tag_values(e, ["referenced-event-id"])
    ///     .expect("non-empty bounded values");
    /// ```
    pub fn tag_values<I, S>(mut self, key: SingleLetterTag, values: I) -> Result<Self, QueryError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        extend_bounded(
            values.into_iter().map(Into::into),
            self.selection.tag_values.entry(key).or_default(),
        );
        Ok(self)
    }

    /// Narrow one case-sensitive Nostr tag axis to the exact strings supplied.
    ///
    /// An absent axis becomes the supplied set. A present axis becomes its
    /// intersection with the supplied set. A disjoint intersection remains a
    /// present empty axis and therefore intentionally matches nothing.
    ///
    /// # Arguments
    ///
    /// * `key` - the case-sensitive single-letter tag key to narrow
    /// * `values` - the values to intersect the existing axis with
    ///
    /// # Errors
    ///
    /// Returns [`QueryError`] for empty relay sets or zero limits.
    pub fn intersect_tag_values<I, S>(
        mut self,
        key: SingleLetterTag,
        values: I,
    ) -> Result<Self, QueryError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let supplied = bounded_tag_values(values.into_iter().map(Into::into));
        if let Some(existing) = self.selection.tag_values.get_mut(&key) {
            existing.retain(|value| supplied.contains(value));
        } else {
            self.selection.tag_values.insert(key, supplied);
        }
        Ok(self)
    }

    /// Event selection.
    #[must_use]
    pub const fn selection(&self) -> &FilterSelection {
        &self.selection
    }
}
