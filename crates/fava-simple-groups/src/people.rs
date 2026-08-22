use fava_write::{EventValue, PublicKey};

use crate::GroupError;
use crate::records::record_boundary;

/// Positive administrator rows from one signed kind-39001 record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAdmins {
    id: String,
    author: PublicKey,
    admins: Vec<Result<(PublicKey, Vec<String>), GroupError>>,
}

/// Positive member rows from one signed kind-39002 record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupMembers {
    id: String,
    author: PublicKey,
    members: Vec<Result<PublicKey, GroupError>>,
}

/// Role definitions from one signed kind-39003 record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupRoles {
    id: String,
    author: PublicKey,
    roles: Vec<Result<(String, Option<String>), GroupError>>,
}

/// Positive live participant rows from one signed kind-39004 record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupParticipants {
    id: String,
    author: PublicKey,
    participants: Vec<Result<PublicKey, GroupError>>,
}

impl GroupAdmins {
    /// Parse one signed kind-39001 record.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the signed record boundary is invalid.
    pub fn from_event(event: &EventValue) -> Result<Self, GroupError> {
        let boundary = record_boundary(event, 39_001)?;
        let author = boundary.author();
        let admins = boundary
            .tags()
            .iter()
            .enumerate()
            .find(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("p"))
            .map(|(index, tag)| parse_admin(index, tag.as_slice()))
            .into_iter()
            .collect();
        Ok(Self {
            id: boundary.id,
            author,
            admins,
        })
    }

    /// Exact group id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Relay author that signed this record.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Source-ordered positive rows and row-local failures.
    #[must_use]
    pub fn admins(&self) -> &[Result<(PublicKey, Vec<String>), GroupError>] {
        &self.admins
    }
}

impl GroupMembers {
    /// Parse one signed kind-39002 record.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the signed record boundary is invalid.
    pub fn from_event(event: &EventValue) -> Result<Self, GroupError> {
        let boundary = record_boundary(event, 39_002)?;
        let author = boundary.author();
        let members = boundary
            .tags()
            .iter()
            .enumerate()
            .find(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("p"))
            .map(|(index, tag)| parse_key(index, tag.as_slice(), "member"))
            .into_iter()
            .collect();
        Ok(Self {
            id: boundary.id,
            author,
            members,
        })
    }

    /// Exact group id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Relay author that signed this record.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Source-ordered positive rows and row-local failures.
    #[must_use]
    pub fn members(&self) -> &[Result<PublicKey, GroupError>] {
        &self.members
    }
}

impl GroupRoles {
    /// Parse one signed kind-39003 record.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the signed record boundary is invalid.
    pub fn from_event(event: &EventValue) -> Result<Self, GroupError> {
        let boundary = record_boundary(event, 39_003)?;
        let author = boundary.author();
        let roles = boundary
            .tags()
            .iter()
            .enumerate()
            .find(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("role"))
            .map(|(index, tag)| parse_role(index, tag.as_slice()))
            .into_iter()
            .collect();
        Ok(Self {
            id: boundary.id,
            author,
            roles,
        })
    }

    /// Exact group id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Relay author that signed this record.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Source-ordered role names and optional descriptions with row-local failures.
    #[must_use]
    pub fn roles(&self) -> &[Result<(String, Option<String>), GroupError>] {
        &self.roles
    }
}

impl GroupParticipants {
    /// Parse one signed kind-39004 record.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the signed record boundary is invalid.
    pub fn from_event(event: &EventValue) -> Result<Self, GroupError> {
        let boundary = record_boundary(event, 39_004)?;
        let author = boundary.author();
        let participants = boundary
            .tags()
            .iter()
            .enumerate()
            .find(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("participant"))
            .map(|(index, tag)| parse_key(index, tag.as_slice(), "participant"))
            .into_iter()
            .collect();
        Ok(Self {
            id: boundary.id,
            author,
            participants,
        })
    }

    /// Exact group id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Relay author that signed this record.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Source-ordered positive rows and row-local failures.
    #[must_use]
    pub fn participants(&self) -> &[Result<PublicKey, GroupError>] {
        &self.participants
    }
}

fn parse_admin(
    tag_index: usize,
    values: &[String],
) -> Result<(PublicKey, Vec<String>), GroupError> {
    let key = parse_key_prefix(tag_index, values, "admin")?;
    Ok((key, values[2..].to_vec()))
}

fn parse_key(
    tag_index: usize,
    values: &[String],
    row: &'static str,
) -> Result<PublicKey, GroupError> {
    if values.len() != 2 {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "public-key row must contain exactly one value",
        });
    }
    parse_key_prefix(tag_index, values, row)
}

fn parse_key_prefix(
    tag_index: usize,
    values: &[String],
    _row: &'static str,
) -> Result<PublicKey, GroupError> {
    values
        .get(1)
        .and_then(|value| PublicKey::from_hex(value).ok())
        .ok_or(GroupError::MalformedRecordRow {
            tag_index,
            reason: "public key is missing or invalid",
        })
}

fn parse_role(tag_index: usize, values: &[String]) -> Result<(String, Option<String>), GroupError> {
    if !(2..=3).contains(&values.len()) {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "role row requires a name and optional description",
        });
    }
    Ok((values[1].clone(), values.get(2).cloned()))
}
