use fava_write::{EventValue, Kind, PublicKey};

use crate::SimpleGroupError;
use crate::records::record_boundary;

/// Complete typed kind-39000 simple group metadata from one relay-authored event.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // NIP-29 defines five independent presence flags.
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
    supported_kinds: Option<Vec<Kind>>,
    parent: Option<String>,
    children: Vec<String>,
}

impl SimpleGroupMetadata {
    /// Parse one exact signed kind-39000 record without opening work.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when the event boundary or typed record is invalid.
    pub fn from_event(event: &EventValue) -> Result<Self, SimpleGroupError> {
        let boundary = record_boundary(event, 39_000)?;
        let author = boundary.author();
        let mut name = None;
        let mut picture = None;
        let mut banner = None;
        let mut about = None;
        let mut private = false;
        let mut restricted = false;
        let mut hidden = false;
        let mut closed = false;
        let mut livekit = false;
        let mut supported_kinds = None;
        let mut parent = None;
        let mut children = Vec::new();

        for (tag_index, tag) in boundary.tags().iter().enumerate() {
            let values = tag.as_slice();
            let Some(key) = values.first().map(String::as_str) else {
                continue;
            };
            match key {
                "name" => set_scalar(&mut name, values, tag_index, "name")?,
                "picture" => set_scalar(&mut picture, values, tag_index, "picture")?,
                "banner" => set_scalar(&mut banner, values, tag_index, "banner")?,
                "about" => set_scalar(&mut about, values, tag_index, "about")?,
                "private" => set_flag(&mut private, values, tag_index, "private")?,
                "restricted" => {
                    set_flag(&mut restricted, values, tag_index, "restricted")?;
                }
                "hidden" => set_flag(&mut hidden, values, tag_index, "hidden")?,
                "closed" => set_flag(&mut closed, values, tag_index, "closed")?,
                "livekit" => set_flag(&mut livekit, values, tag_index, "livekit")?,
                "supported_kinds" => {
                    if supported_kinds.is_some() {
                        return Err(SimpleGroupError::AmbiguousRecordField("supported_kinds"));
                    }
                    let kinds = values[1..]
                        .iter()
                        .map(|value| {
                            value.parse::<u16>().map(Kind::from_u16).map_err(|_| {
                                SimpleGroupError::MalformedRecordRow {
                                    tag_index,
                                    reason: "supported kind is not a decimal u16",
                                }
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    supported_kinds = Some(kinds);
                }
                "parent" => set_scalar(&mut parent, values, tag_index, "parent")?,
                "child" => {
                    if values.len() != 2 {
                        return Err(SimpleGroupError::MalformedRecordRow {
                            tag_index,
                            reason: "child row must contain exactly one value",
                        });
                    }
                    children.push(values[1].clone());
                }
                _ => {}
            }
        }

        Ok(Self {
            id: boundary.id,
            author,
            name,
            picture,
            banner,
            about,
            private,
            restricted,
            hidden,
            closed,
            livekit,
            supported_kinds,
            parent,
            children,
        })
    }

    /// Exact opaque simple group id from the `d` row.
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

    /// Optional exact parent simple group id.
    #[must_use]
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    /// Ordered exact child simple group ids.
    #[must_use]
    pub fn children(&self) -> &[String] {
        &self.children
    }
}

fn set_scalar(
    field: &mut Option<String>,
    values: &[String],
    tag_index: usize,
    name: &'static str,
) -> Result<(), SimpleGroupError> {
    if field.is_some() {
        return Err(SimpleGroupError::AmbiguousRecordField(name));
    }
    if values.len() != 2 {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "scalar row must contain exactly one value",
        });
    }
    *field = Some(values[1].clone());
    Ok(())
}

fn set_flag(
    field: &mut bool,
    values: &[String],
    tag_index: usize,
    name: &'static str,
) -> Result<(), SimpleGroupError> {
    if *field {
        return Err(SimpleGroupError::AmbiguousRecordField(name));
    }
    if values.len() != 1 {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "flag row must not contain values",
        });
    }
    *field = true;
    Ok(())
}
