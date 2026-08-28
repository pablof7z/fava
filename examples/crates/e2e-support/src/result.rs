//! Typed result records and human/JSONL renderers.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::ShellError;

/// Terminal classification for one command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResultStatus {
    /// Command completed its requested work.
    Ok,
    /// Command was rejected before an unsupported or unsafe action could run.
    Refused,
    /// Command attempted its work and reported a domain failure.
    Failed,
}

/// One JSON-safe value exposed by a bounded command result.
///
/// Arrays contain only scalar DTO values; objects, nested arrays, and arbitrary
/// serialized application values are deliberately excluded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum ResultValue {
    /// Exact public text.
    Text(String),
    /// A nonnegative public integer.
    Integer(u64),
    /// A public boolean fact.
    Boolean(bool),
    /// A bounded ordered sequence of scalar DTO values.
    Array(Vec<Self>),
}

impl ResultValue {
    /// Construct one text value.
    #[must_use]
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// Construct public text, replacing a secret-shaped external diagnostic.
    ///
    /// Relay-provided messages are public protocol facts unless they resemble
    /// credentials; that exceptional text is retained only as a redaction.
    #[must_use]
    pub fn public_text(value: impl Into<String>) -> Self {
        let value = value.into();
        if sensitive_value(&value) || contains_raw_hex_run(&value) {
            Self::Text("<redacted>".to_owned())
        } else {
            Self::Text(value)
        }
    }

    /// Construct one ordered array value.
    #[must_use]
    pub fn array(values: impl IntoIterator<Item = Self>) -> Self {
        Self::Array(values.into_iter().collect())
    }

    /// Return the deterministic interpolation text for a scalar value.
    #[must_use]
    pub fn capture_text(&self) -> Option<String> {
        match self {
            Self::Text(value) => Some(value.clone()),
            Self::Integer(value) => Some(value.to_string()),
            Self::Boolean(value) => Some(value.to_string()),
            Self::Array(_) => None,
        }
    }

    fn is_sensitive(&self) -> bool {
        match self {
            Self::Text(value) => sensitive_value(value),
            Self::Integer(_) | Self::Boolean(_) => false,
            Self::Array(values) => values.iter().any(Self::is_sensitive),
        }
    }

    fn enforce_bounds(&self, text_bytes: usize, array_items: usize) -> Result<(), ShellError> {
        match self {
            Self::Text(value) if value.len() > text_bytes => Err(ShellError::Limit {
                what: "result field bytes",
                maximum: text_bytes,
            }),
            Self::Text(_) | Self::Integer(_) | Self::Boolean(_) => Ok(()),
            Self::Array(values) => {
                if values.len() > array_items {
                    return Err(ShellError::Limit {
                        what: "result array items",
                        maximum: array_items,
                    });
                }
                for value in values {
                    if matches!(value, Self::Array(_)) {
                        return Err(ShellError::NestedResultArray);
                    }
                    value.enforce_bounds(text_bytes, array_items)?;
                }
                Ok(())
            }
        }
    }

    fn render_human(&self) -> Result<String, ShellError> {
        match self {
            Self::Text(value) => Ok(value.clone()),
            Self::Integer(value) => Ok(value.to_string()),
            Self::Boolean(value) => Ok(value.to_string()),
            Self::Array(_) => {
                serde_json::to_string(self).map_err(|error| ShellError::Json(error.to_string()))
            }
        }
    }
}

impl From<String> for ResultValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for ResultValue {
    fn from(value: &str) -> Self {
        Self::text(value)
    }
}

impl From<u64> for ResultValue {
    fn from(value: u64) -> Self {
        Self::Integer(value)
    }
}

impl From<usize> for ResultValue {
    fn from(value: usize) -> Self {
        Self::Integer(value as u64)
    }
}

impl From<bool> for ResultValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

/// One bounded public-facing result retained only until the next command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CommandResult {
    status: ResultStatus,
    kind: String,
    summary: String,
    fields: BTreeMap<String, ResultValue>,
}

impl CommandResult {
    /// Construct a successful result from one shell or domain command.
    #[must_use]
    pub fn success(kind: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            status: ResultStatus::Ok,
            kind: kind.into(),
            summary: summary.into(),
            fields: BTreeMap::new(),
        }
    }

    /// Construct a typed refusal that renderers can expose safely.
    #[must_use]
    pub fn refused(kind: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            status: ResultStatus::Refused,
            kind: kind.into(),
            summary: summary.into(),
            fields: BTreeMap::new(),
        }
    }

    /// Construct a typed post-attempt failure that preserves no unbounded error data.
    #[must_use]
    pub fn failed(kind: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            status: ResultStatus::Failed,
            kind: kind.into(),
            summary: summary.into(),
            fields: BTreeMap::new(),
        }
    }

    /// Add one safe scalar or array field available to JSONL output.
    ///
    /// # Errors
    ///
    /// Refuses secret-shaped field names or values before they can enter a
    /// renderer, capture, history-adjacent dump, or script transcript.
    pub fn with_field(
        mut self,
        name: impl Into<String>,
        value: impl Into<ResultValue>,
    ) -> Result<Self, ShellError> {
        let name = name.into();
        let value = value.into();
        if sensitive_field_name(&name) || value.is_sensitive() {
            return Err(ShellError::SensitiveResultField { name });
        }
        self.fields.insert(name, value);
        Ok(self)
    }

    /// Return this record's terminal status.
    #[must_use]
    pub const fn status(&self) -> ResultStatus {
        self.status
    }

    /// Return the stable result kind for scripts.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Return the concise human result summary.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Return one JSON-safe result field by exact name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&ResultValue> {
        self.fields.get(name)
    }

    /// Return all JSON-safe fields in stable name order.
    #[must_use]
    pub fn fields(&self) -> &BTreeMap<String, ResultValue> {
        &self.fields
    }

    pub(crate) fn enforce_bounds(
        &self,
        text_bytes: usize,
        field_count: usize,
    ) -> Result<(), ShellError> {
        if self.kind.len() > text_bytes || self.summary.len() > text_bytes {
            return Err(ShellError::Limit {
                what: "result text bytes",
                maximum: text_bytes,
            });
        }
        if self.fields.len() > field_count {
            return Err(ShellError::Limit {
                what: "result fields",
                maximum: field_count,
            });
        }
        for (name, value) in &self.fields {
            if name.len() > text_bytes {
                return Err(ShellError::Limit {
                    what: "result field bytes",
                    maximum: text_bytes,
                });
            }
            value.enforce_bounds(text_bytes, field_count)?;
        }
        Ok(())
    }
}

/// The command-output format selected by an app entry point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Readable terminal output.
    Human,
    /// One schema-stable JSON object and newline per command.
    JsonLines,
}

impl OutputFormat {
    /// Render one result without retaining another copy of it.
    ///
    /// # Errors
    ///
    /// Returns [`ShellError::Json`] only if serializing the bounded result fails.
    pub fn render(self, result: &CommandResult) -> Result<String, ShellError> {
        match self {
            Self::Human => {
                let mut output =
                    format!("[{:?}] {}: {}", result.status, result.kind, result.summary);
                for (name, value) in &result.fields {
                    output.push('\n');
                    output.push_str("  ");
                    output.push_str(name);
                    output.push('=');
                    output.push_str(&value.render_human()?);
                }
                output.push('\n');
                Ok(output)
            }
            Self::JsonLines => serde_json::to_string(result)
                .map(|line| format!("{line}\n"))
                .map_err(|error| ShellError::Json(error.to_string())),
        }
    }
}

pub(crate) fn sensitive_field_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "secret" | "password" | "token" | "private_key" | "nsec"
    )
}

fn contains_raw_hex_run(value: &str) -> bool {
    value
        .split(|character: char| !character.is_ascii_hexdigit())
        .any(|segment| segment.len() >= 64)
}

pub(crate) fn sensitive_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("nsec1")
        || lower.contains("-----begin")
        || lower.contains("secret=")
        || lower.contains("password=")
        || lower.contains("token=")
}
