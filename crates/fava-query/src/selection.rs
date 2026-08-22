use std::collections::{BTreeMap, BTreeSet};

pub use nostr::filter::SingleLetterTag;

use crate::{EventId, Kind, PublicKey, Query};

/// Declarative event-filter axes supported by literal query selection.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct FilterSelection {
    /// Event ids, or all ids when absent. A present empty set matches nothing.
    pub ids: Option<BTreeSet<EventId>>,
    /// Authors, or all authors when absent. A present empty set matches nothing.
    pub authors: Option<BTreeSet<PublicKey>>,
    /// Kinds, or all kinds when absent. A present empty set matches nothing.
    pub kinds: Option<BTreeSet<Kind>>,
    /// Exact tag values by case-sensitive one-letter key.
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

    /// Match one event kind.
    ///
    /// One call selects a singleton. Repeated calls union canonical kind
    /// values, so duplicates and call order do not change query identity.
    #[must_use]
    pub fn kind(mut self, kind: Kind) -> Self {
        self.selection.kinds.get_or_insert_default().insert(kind);
        self
    }

    /// Match a literal author set. An empty set intentionally matches nothing.
    #[must_use]
    pub fn authors(mut self, authors: impl IntoIterator<Item = PublicKey>) -> Self {
        self.selection.authors = Some(authors.into_iter().collect());
        self
    }

    /// Match a literal event-id set. An empty set intentionally matches nothing.
    #[must_use]
    pub fn ids(mut self, ids: impl IntoIterator<Item = EventId>) -> Self {
        self.selection.ids = Some(ids.into_iter().collect());
        self
    }

    /// Match exact values for one case-sensitive Nostr tag key.
    ///
    /// Repeated calls for the same key union values. An empty iterator retains
    /// a present tag axis that intentionally matches nothing.
    #[must_use]
    pub fn tag_values<I, S>(mut self, key: SingleLetterTag, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.selection
            .tag_values
            .entry(key)
            .or_default()
            .extend(values.into_iter().map(Into::into));
        self
    }

    /// Event selection.
    #[must_use]
    pub const fn selection(&self) -> &FilterSelection {
        &self.selection
    }
}
