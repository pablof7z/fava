//! Current facts about provider operations Fava has authorized.

use std::time::Duration;

use fava_query::{BoundedText, OperationGeneration};

/// A provider operation Fava has authorized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDiagnostic {
    /// Which provider.
    pub provider: ProviderKind,
    /// The operation.
    pub operation: ProviderOperation,
    /// Its current disposition.
    pub state: ProviderOperationState,
}

/// The replaceable providers Fava calls.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProviderKind {
    /// `EventCache`.
    EventCache,
    /// `WriteStore`.
    WriteStore,
    /// `FetchCache`.
    FetchCache,
    /// `QueryEvaluator`.
    QueryEvaluator,
    /// One `Router`.
    Router,
    /// `SubscriptionPlanner`.
    SubscriptionPlanner,
    /// `Transport`.
    Transport,
    /// `Publisher`.
    Publisher,
    /// `DeliveryPolicy`.
    DeliveryPolicy,
    /// `Signer`.
    Signer,
    /// A protocol service (NIP-05, NIP-11).
    Service,
}

/// Identity of one authorized provider operation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderOperation {
    /// Provider instance name supplied at assembly, bounded.
    pub instance: BoundedText,
    /// Generation of this operation slot. Late completions carrying an older
    /// generation are stale.
    pub generation: OperationGeneration,
}

/// Disposition of one provider operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderOperationState {
    /// Running, with the Fava-owned deadline it must beat.
    Running {
        /// Deadline supplied by the owner.
        deadline: Duration,
        /// Elapsed so far.
        elapsed: Duration,
    },
    /// Completed within its deadline.
    Completed,
    /// The deadline expired.
    TimedOut {
        /// The deadline that expired.
        after: Duration,
    },
    /// The provider returned an error.
    Failed {
        /// Bounded reason.
        detail: BoundedText,
    },
    /// The provider panicked and was isolated.
    Panicked {
        /// Bounded panic payload.
        detail: BoundedText,
    },
    /// The owner cancelled it.
    Cancelled,
}
