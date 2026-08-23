//! What is currently live on one relay session.

use std::collections::{BTreeMap, BTreeSet};

use fava_wire::SubscriptionId;
use nostr::filter::Filter;

use crate::demand::DemandId;

/// What is currently live on this relay session, as the planner's baseline.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstalledSubscriptions {
    /// Wire subscriptions accepted on the current generation, and the exact
    /// demand each was installed to serve.
    entries: BTreeMap<SubscriptionId, InstalledSubscription>,
}

/// One wire subscription currently live on a relay session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledSubscription {
    /// Filters carried by the installed REQ.
    pub filters: Vec<Filter>,
    /// Logical demand this subscription was installed to serve.
    pub serves: BTreeSet<DemandId>,
}

impl InstalledSubscriptions {
    /// An empty baseline: a fresh session or a fresh generation after reconnect.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct from installed entries.
    #[must_use]
    pub fn from_entries(
        entries: impl IntoIterator<Item = (SubscriptionId, InstalledSubscription)>,
    ) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Installed entry for one wire id.
    #[must_use]
    pub fn get(&self, id: &SubscriptionId) -> Option<&InstalledSubscription> {
        self.entries.get(id)
    }

    /// Every installed wire id, ascending.
    pub fn ids(&self) -> impl Iterator<Item = &SubscriptionId> {
        self.entries.keys()
    }

    /// Number of installed wire subscriptions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is installed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
