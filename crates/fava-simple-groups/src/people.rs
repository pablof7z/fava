use std::collections::BTreeSet;

use fava_write::{EventValue, PublicKey, Tag};

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
        let admins = collect_rows(
            boundary.tags(),
            "p",
            parse_admin,
            |(key, _): &(PublicKey, Vec<String>)| *key,
        );
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
        let members = collect_rows(
            boundary.tags(),
            "p",
            |index, values| parse_key(index, values, false),
            |key| *key,
        );
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
        let roles = collect_rows(
            boundary.tags(),
            "role",
            parse_role,
            |(name, _): &(String, Option<String>)| name.clone(),
        );
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
        let participants = collect_rows(
            boundary.tags(),
            "participant",
            |index, values| parse_key(index, values, true),
            |key| *key,
        );
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
    pub fn participants(&self) -> &[Result<PublicKey, GroupError>] {
        &self.participants
    }
}

fn parse_admin(
    tag_index: usize,
    values: &[String],
) -> Result<(PublicKey, Vec<String>), GroupError> {
    if values.len() < 3 {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "admin row requires a public key and at least one role",
        });
    }
    let key = parse_key_prefix(tag_index, values, false)?;
    Ok((key, values[2..].to_vec()))
}

fn parse_key(
    tag_index: usize,
    values: &[String],
    require_lowercase: bool,
) -> Result<PublicKey, GroupError> {
    if values.len() != 2 {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "public-key row must contain exactly one value",
        });
    }
    parse_key_prefix(tag_index, values, require_lowercase)
}

fn parse_key_prefix(
    tag_index: usize,
    values: &[String],
    require_lowercase: bool,
) -> Result<PublicKey, GroupError> {
    let value = values.get(1).ok_or(GroupError::MalformedRecordRow {
        tag_index,
        reason: "public key is missing or invalid",
    })?;
    if require_lowercase
        && (value.len() != 64 || value.bytes().any(|byte| byte.is_ascii_uppercase()))
    {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "participant public key must be lowercase hex",
        });
    }
    PublicKey::from_hex(value).map_err(|_| GroupError::MalformedRecordRow {
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
    if values[1].is_empty() {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "role name must not be empty",
        });
    }
    Ok((values[1].clone(), values.get(2).cloned()))
}

fn collect_rows<T, K>(
    tags: &[Tag],
    row_name: &str,
    parse: impl Fn(usize, &[String]) -> Result<T, GroupError>,
    key: impl Fn(&T) -> K,
) -> Vec<Result<T, GroupError>>
where
    K: Ord,
{
    let mut seen = BTreeSet::new();
    tags.iter()
        .enumerate()
        .filter(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some(row_name))
        .map(|(tag_index, tag)| match parse(tag_index, tag.as_slice()) {
            Ok(value) if seen.insert(key(&value)) => Ok(value),
            Ok(_) => Err(GroupError::DuplicateRecordRow { tag_index }),
            Err(error) => Err(error),
        })
        .collect()
}
