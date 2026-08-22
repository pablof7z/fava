use fava_write::{EventValue, Kind, PublicKey};

use crate::GroupError;
use crate::records::record_boundary;

/// Complete typed kind-39000 group metadata from one relay-authored event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupMetadata {
    id: String,
    author: PublicKey,
    name: Option<String>,
    picture: Option<String>,
    banner: Option<String>,
    about: Option<String>,
    private: bool,
    restricted: bool,
    hidden: bool,
    closed: bool,
    livekit: bool,
    supported_kinds: Option<Vec<Kind>>,
    parent: Option<String>,
    children: Vec<String>,
}

impl GroupMetadata {
    /// Parse one exact signed kind-39000 record without opening work.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the event boundary or typed record is invalid.
    pub fn from_event(event: &EventValue) -> Result<Self, GroupError> {
        let boundary = record_boundary(event, 39_000)?;
        let author = boundary.author();
        Ok(Self {
            id: boundary.id,
            author,
            name: None,
            picture: None,
            banner: None,
            about: None,
            private: false,
            restricted: false,
            hidden: false,
            closed: false,
            livekit: false,
            supported_kinds: None,
            parent: None,
            children: Vec::new(),
        })
    }

    /// Exact opaque group id from the `d` row.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Relay author that signed this record.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Optional display name, preserving present-empty input.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Optional picture URL text, without opening it.
    #[must_use]
    pub fn picture(&self) -> Option<&str> {
        self.picture.as_deref()
    }

    /// Optional banner URL text, without opening it.
    #[must_use]
    pub fn banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }

    /// Optional exact about text.
    #[must_use]
    pub fn about(&self) -> Option<&str> {
        self.about.as_deref()
    }

    /// Whether the record carries the exact `private` flag.
    #[must_use]
    pub const fn is_private(&self) -> bool {
        self.private
    }

    /// Whether the record carries the exact `restricted` flag.
    #[must_use]
    pub const fn is_restricted(&self) -> bool {
        self.restricted
    }

    /// Whether the record carries the exact `hidden` flag.
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Whether the record carries the exact `closed` flag.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Whether the record carries the exact `livekit` flag.
    #[must_use]
    pub const fn has_livekit(&self) -> bool {
        self.livekit
    }

    /// Supported kinds in source order; `None` means unspecified and `Some([])` means none.
    #[must_use]
    pub fn supported_kinds(&self) -> Option<&[Kind]> {
        self.supported_kinds.as_deref()
    }

    /// Optional exact parent group id.
    #[must_use]
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    /// Ordered exact child group ids.
    #[must_use]
    pub fn children(&self) -> &[String] {
        &self.children
    }
}
