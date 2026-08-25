use fava_write::{EventValue, Kind, PublicKey};

use crate::records::{SimpleGroupDecodeError, required_value, state_event};

/// Semantic kind-39000 simple-group metadata from one event.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct SimpleGroupMetadata {
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
    supported_kinds: Option<Vec<Result<Kind, SimpleGroupDecodeError>>>,
    parent: Option<String>,
    children: Vec<Result<String, SimpleGroupDecodeError>>,
}

impl SimpleGroupMetadata {
    /// Decode one kind-39000 event without establishing trust or provenance.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupDecodeError`] for the wrong kind or a missing first `d` value.
    pub fn from_event(event: &EventValue) -> Result<Self, SimpleGroupDecodeError> {
        let (id, author, tags) = state_event(event, 39_000)?;
        let mut value = Self {
            id: id.to_owned(),
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
        };
        for (tag_index, tag) in tags.iter().enumerate() {
            let values = tag.as_slice();
            match values.first().map(String::as_str) {
                Some("name") => set_first(&mut value.name, values.get(1)),
                Some("picture") => set_first(&mut value.picture, values.get(1)),
                Some("banner") => set_first(&mut value.banner, values.get(1)),
                Some("about") => set_first(&mut value.about, values.get(1)),
                Some("private") => value.private = true,
                Some("restricted") => value.restricted = true,
                Some("hidden") => value.hidden = true,
                Some("closed") => value.closed = true,
                Some("livekit") => value.livekit = true,
                Some("supported_kinds") if value.supported_kinds.is_none() => {
                    value.supported_kinds = Some(
                        values[1..]
                            .iter()
                            .enumerate()
                            .map(|(offset, raw)| {
                                raw.parse::<u16>().map(Kind::from_u16).map_err(|_| {
                                    SimpleGroupDecodeError::InvalidKind {
                                        tag_index,
                                        value_index: offset + 1,
                                    }
                                })
                            })
                            .collect(),
                    );
                }
                Some("parent") => set_first(&mut value.parent, values.get(1)),
                Some("child") => value
                    .children
                    .push(required_value(values, tag_index, 1).map(str::to_owned)),
                _ => {}
            }
        }
        Ok(value)
    }

    /// Borrow the first `d` tag's first value.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return the event author.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Return the first usable `name` value.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Return the first usable `picture` value.
    #[must_use]
    pub fn picture(&self) -> Option<&str> {
        self.picture.as_deref()
    }

    /// Return the first usable `banner` value.
    #[must_use]
    pub fn banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }

    /// Return the first usable `about` value.
    #[must_use]
    pub fn about(&self) -> Option<&str> {
        self.about.as_deref()
    }

    /// Whether a `private` tag is present.
    #[must_use]
    pub const fn is_private(&self) -> bool {
        self.private
    }

    /// Whether a `restricted` tag is present.
    #[must_use]
    pub const fn is_restricted(&self) -> bool {
        self.restricted
    }

    /// Whether a `hidden` tag is present.
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Whether a `closed` tag is present.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Whether a `livekit` tag is present.
    #[must_use]
    pub const fn has_livekit(&self) -> bool {
        self.livekit
    }

    /// Values from the first `supported_kinds` tag, retaining value-local failures.
    #[must_use]
    pub fn supported_kinds(&self) -> Option<&[Result<Kind, SimpleGroupDecodeError>]> {
        self.supported_kinds.as_deref()
    }

    /// Return the first usable `parent` value.
    #[must_use]
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    /// Return every `child` tag as its first value or a local failure.
    pub fn children(&self) -> &[Result<String, SimpleGroupDecodeError>] {
        &self.children
    }
}

fn set_first(target: &mut Option<String>, candidate: Option<&String>) {
    if target.is_none() {
        *target = candidate.cloned();
    }
}
