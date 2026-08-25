use fava_query::{Kind, PublicKey, Query, QueryError};

/// One NIP-29 relay-generated simple-group state-event kind.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SimpleGroupStateEventKind {
    /// Kind 39000 metadata.
    Metadata,
    /// Kind 39001 administrators.
    Admins,
    /// Kind 39002 members.
    Members,
    /// Kind 39003 roles.
    Roles,
    /// Kind 39004 `LiveKit` participants.
    LivekitParticipants,
    /// Kind 39005 pins.
    Pins,
}

impl SimpleGroupStateEventKind {
    /// All six state-event kinds.
    pub const ALL: [Self; 6] = [
        Self::Metadata,
        Self::Admins,
        Self::Members,
        Self::Roles,
        Self::LivekitParticipants,
        Self::Pins,
    ];
}

impl From<SimpleGroupStateEventKind> for Kind {
    fn from(value: SimpleGroupStateEventKind) -> Self {
        let kind = match value {
            SimpleGroupStateEventKind::Metadata => 39_000,
            SimpleGroupStateEventKind::Admins => 39_001,
            SimpleGroupStateEventKind::Members => 39_002,
            SimpleGroupStateEventKind::Roles => 39_003,
            SimpleGroupStateEventKind::LivekitParticipants => 39_004,
            SimpleGroupStateEventKind::Pins => 39_005,
        };
        Self::from_u16(kind)
    }
}

/// Build the ordinary kind-10009 Simple Group List query for exact authors.
///
/// # Errors
///
/// Returns [`QueryError`] when the query owner refuses the author input under
/// its provisional resource cap.
pub fn saved_group_lists(
    authors: impl IntoIterator<Item = PublicKey>,
) -> Result<Query, QueryError> {
    Query::events()
        .kinds([Kind::from_u16(10_009)])?
        .authors(authors)
}
