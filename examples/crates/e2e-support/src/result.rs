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

/// One bounded public-facing result retained only until the next command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CommandResult {
    status: ResultStatus,
    kind: String,
    summary: String,
    fields: BTreeMap<String, String>,
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

    /// Add one safe scalar field available to captures and JSONL output.
    ///
    /// # Errors
    ///
    /// Refuses reserved secret-shaped field names before they can enter a
    /// renderer, capture, history-adjacent dump, or script transcript.
    pub fn with_field(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, ShellError> {
        let name = name.into();
        if sensitive_field_name(&name) {
            return Err(ShellError::SensitiveResultField { name });
        }
        self.fields.insert(name, value.into());
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

    /// Return one capture-safe result field by exact name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).map(String::as_str)
    }

    /// Return all scalar capture-safe fields in stable name order.
    #[must_use]
    pub fn fields(&self) -> &BTreeMap<String, String> {
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
        if self
            .fields
            .iter()
            .any(|(name, value)| name.len() > text_bytes || value.len() > text_bytes)
        {
            return Err(ShellError::Limit {
                what: "result field bytes",
                maximum: text_bytes,
            });
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
                    output.push_str(value);
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
