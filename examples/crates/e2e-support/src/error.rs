//! Typed refusals at the private application-shell boundary.

use thiserror::Error;

use crate::result::CommandResult;

/// A bounded, attributed refusal while reading or executing one shell command.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ShellError {
    /// A domain command attempted work and supplied a safe typed terminal
    /// failure record which must be rendered before replay stops.
    #[error("command emitted a failed result")]
    CommandFailed {
        /// Safe, bounded public evidence for the attempted command.
        result: CommandResult,
    },
    /// An application-owned collection reached its declared maximum.
    #[error("{what} exceeds its limit of {maximum}")]
    Limit {
        /// Retained collection or input whose bound was reached.
        what: &'static str,
        /// Maximum accepted cardinality or byte length.
        maximum: usize,
    },
    /// A command's quoting is malformed before it reaches domain code.
    #[error("unterminated quoted argument")]
    UnterminatedQuote,
    /// A capture reference did not name a retained capture.
    #[error("unknown capture {name:?}")]
    UnknownCapture {
        /// Missing capture name.
        name: String,
    },
    /// The last result did not expose the requested capture-safe field.
    #[error("last result has no field {name:?}")]
    MissingResultField {
        /// Missing result field name.
        name: String,
    },
    /// A result field exists but is not one capture-safe scalar.
    #[error("last result field {name:?} is not a capture-safe scalar")]
    NonScalarResultField {
        /// Existing non-scalar field name.
        name: String,
    },
    /// A result array tried to become a generic nested data structure.
    #[error("nested result arrays are forbidden")]
    NestedResultArray,
    /// A capture reference was syntactically invalid.
    #[error("invalid capture reference {reference:?}")]
    InvalidCaptureReference {
        /// Full malformed interpolation text.
        reference: String,
    },
    /// A command requires a selected account before it can construct an event.
    #[error("no account is selected")]
    NoSelectedAccount,
    /// An account alias was unknown.
    #[error("unknown account alias {alias:?}")]
    UnknownAccount {
        /// Unknown account alias.
        alias: String,
    },
    /// A relay alias was unknown.
    #[error("unknown relay alias {alias:?}")]
    UnknownRelay {
        /// Unknown relay alias.
        alias: String,
    },
    /// An alias violates the bounded shell identifier grammar.
    #[error("invalid {kind} alias {alias:?}")]
    InvalidAlias {
        /// Alias family.
        kind: &'static str,
        /// Invalid alias.
        alias: String,
    },
    /// A relay URL could not be parsed through the public Fava type.
    #[error("invalid relay URL {input:?}: {reason}")]
    InvalidRelayUrl {
        /// Raw user input.
        input: String,
        /// Parser refusal.
        reason: String,
    },
    /// A selected account name is repeated.
    #[error("duplicate account alias {alias:?}")]
    DuplicateAccount {
        /// Repeated account alias.
        alias: String,
    },
    /// An imported key could not form a local signer.
    #[error("imported account key is not a valid Nostr secret key")]
    InvalidImportedAccount,
    /// A replacement key does not belong to the named account.
    #[error("replacement key belongs to {actual}, not account {expected}")]
    AccountKeyMismatch {
        /// Public key retained by the alias.
        expected: String,
        /// Public key derived from replacement input.
        actual: String,
    },
    /// Fava refused an account or signer lifecycle change.
    #[error("account operation failed: {0}")]
    AccountSigner(String),
    /// One built-in command has an invalid argument shape.
    #[error("usage: {usage}")]
    Usage {
        /// Exact usage string.
        usage: &'static str,
    },
    /// A domain command was not recognized by the real consumer.
    #[error("unknown command {command:?}")]
    UnknownCommand {
        /// Unknown command head.
        command: String,
    },
    /// JSONL must not share a stream with interactive prompts.
    #[error("interactive input cannot use JSONL output; use --script or human output")]
    InteractiveJsonLines,
    /// A domain-required ordinary value cannot be requested from a replay.
    #[error("interactive prompting is unavailable for script input")]
    NonInteractivePrompt,
    /// A public-key string was invalid.
    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),
    /// JSON rendering failed before output was emitted.
    #[error("cannot render JSONL: {0}")]
    Json(String),
    /// Output could not be written by the selected terminal mode.
    #[error("cannot write command output: {0}")]
    Output(String),
    /// A concrete domain command could not complete its own bounded workflow.
    #[error("{0}")]
    Domain(String),
}
