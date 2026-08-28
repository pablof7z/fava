//! Shared interactive and script command execution for concrete E2E examples.

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, Write};

use fava::RelayUrl;

use crate::{Account, CommandResult, Limits, OutputFormat, Secret, ShellError};

/// Whether the same command executor is fed by a terminal or a command file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    /// Show a prompt before every line read.
    Interactive,
    /// Consume lines without prompts, suitable for piped stdin and CI.
    Script,
}

/// Bounded, application-owned terminal state shared by one concrete E2E app.
pub struct E2eSession {
    limits: Limits,
    accounts: BTreeMap<String, Account>,
    selected_account: Option<String>,
    relays: BTreeMap<String, RelayUrl>,
    captures: BTreeMap<String, String>,
    history: VecDeque<String>,
    last_result: Option<CommandResult>,
    quitting: bool,
}

impl E2eSession {
    /// Construct a shell with application-provided accounts and no relay aliases.
    ///
    /// # Errors
    ///
    /// Refuses invalid or duplicated account aliases before retaining them.
    pub fn new(
        limits: Limits,
        accounts: impl IntoIterator<Item = Account>,
    ) -> Result<Self, ShellError> {
        let mut retained = BTreeMap::new();
        for account in accounts {
            if retained.len() == limits.accounts() {
                return Err(ShellError::Limit {
                    what: "accounts",
                    maximum: limits.accounts(),
                });
            }
            validate_alias("account", account.alias(), limits.alias_bytes())?;
            if retained
                .insert(account.alias().to_owned(), account.clone())
                .is_some()
            {
                return Err(ShellError::DuplicateAccount {
                    alias: account.alias().to_owned(),
                });
            }
        }
        Ok(Self {
            limits,
            accounts: retained,
            selected_account: None,
            relays: BTreeMap::new(),
            captures: BTreeMap::new(),
            history: VecDeque::new(),
            last_result: None,
            quitting: false,
        })
    }

    /// Run terminal and piped scripts through exactly the same line executor.
    ///
    /// The domain closure receives only commands the shell does not own, plus
    /// the concrete input/output stream for a required interactive value.
    ///
    /// # Errors
    ///
    /// Returns the exact shell refusal, domain refusal, render failure, or
    /// input/output error that prevents the current command stream progressing.
    pub fn run<R, W, F>(
        &mut self,
        input: &mut R,
        output: &mut W,
        mode: InputMode,
        format: OutputFormat,
        mut domain: F,
    ) -> Result<(), ShellError>
    where
        R: BufRead,
        W: Write,
        F: FnMut(
            &mut Self,
            &[String],
            &mut R,
            &mut W,
            InputMode,
        ) -> Result<CommandResult, ShellError>,
    {
        let mut line = String::new();
        loop {
            if matches!(mode, InputMode::Interactive) {
                output
                    .write_all(b"e2e> ")
                    .map_err(|error| output_error(&error))?;
                output.flush().map_err(|error| output_error(&error))?;
            }
            line.clear();
            if input
                .read_line(&mut line)
                .map_err(|error| output_error(&error))?
                == 0
            {
                return Ok(());
            }
            let execution = self.execute_line(line.trim_end(), |session, words| {
                domain(session, words, input, output, mode)
            });
            let (rendered, failure) = match execution {
                Ok(result) => (format.render(&result)?, None),
                Err(error @ ShellError::Domain(_)) => {
                    let summary = bounded_summary(&error.to_string(), self.limits.capture_bytes());
                    (
                        format.render(&CommandResult::failed("domain-failed", summary))?,
                        Some(error),
                    )
                }
                Err(error) => {
                    let summary = bounded_summary(&error.to_string(), self.limits.capture_bytes());
                    (
                        format.render(&CommandResult::refused("shell-refused", summary))?,
                        Some(error),
                    )
                }
            };
            output
                .write_all(rendered.as_bytes())
                .map_err(|error| output_error(&error))?;
            if matches!(mode, InputMode::Script)
                && let Some(error) = failure
            {
                return Err(error);
            }
            if self.quitting {
                return Ok(());
            }
        }
    }

    /// Execute one parsed line through built-ins or one concrete domain closure.
    ///
    /// # Errors
    ///
    /// Refuses oversized, secret-looking, malformed, or unknown retained input
    /// before it reaches the real domain command implementation.
    pub fn execute_line<F>(
        &mut self,
        line: &str,
        mut domain: F,
    ) -> Result<CommandResult, ShellError>
    where
        F: FnMut(&mut Self, &[String]) -> Result<CommandResult, ShellError>,
    {
        if line.len() > self.limits.line_bytes() {
            return Err(ShellError::Limit {
                what: "command line bytes",
                maximum: self.limits.line_bytes(),
            });
        }
        let expanded = self.interpolate(line)?;
        let words = split_command(&expanded)?;
        if words.is_empty() {
            return Ok(CommandResult::success("empty", "no command"));
        }
        if words.len() > self.limits.arguments() {
            return Err(ShellError::Limit {
                what: "command arguments",
                maximum: self.limits.arguments(),
            });
        }
        if words.iter().any(|word| looks_secret(word)) {
            return Err(ShellError::SecretOnCommandLine);
        }
        self.record_history(&expanded, &words)?;
        let result = match words.as_slice() {
            [command, action, alias] if command == "account" && action == "use" => {
                self.use_account(alias)
            }
            [command, action, alias, url] if command == "relay" && action == "add" => {
                self.add_relay(alias, url)
            }
            [command, name, field] if command == "capture" => self.capture(name, field),
            [command] if command == "dump" => self.dump(),
            [command] if command == "quit" || command == "exit" => {
                self.quitting = true;
                Ok(CommandResult::success("quit", "session closed"))
            }
            _ => domain(self, &words),
        }?;
        result.enforce_bounds(self.limits.capture_bytes(), self.limits.result_fields())?;
        self.last_result = Some(result.clone());
        Ok(result)
    }

    /// Return the currently selected account, if any.
    ///
    /// # Errors
    ///
    /// Returns [`ShellError::NoSelectedAccount`] before a domain command can
    /// construct an event with an implicit or stale author.
    pub fn selected_account(&self) -> Result<&Account, ShellError> {
        self.selected_account
            .as_ref()
            .and_then(|alias| self.accounts.get(alias))
            .ok_or(ShellError::NoSelectedAccount)
    }

    /// Resolve one application-owned relay alias.
    ///
    /// # Errors
    ///
    /// Returns [`ShellError::UnknownRelay`] without attempting network work.
    pub fn relay(&self, alias: &str) -> Result<&RelayUrl, ShellError> {
        self.relays
            .get(alias)
            .ok_or_else(|| ShellError::UnknownRelay {
                alias: alias.to_owned(),
            })
    }

    /// Return bounded history entries; protected prompts are absent by construction.
    #[must_use]
    pub fn history(&self) -> Vec<&str> {
        self.history.iter().map(String::as_str).collect()
    }

    /// Read one protected secret without passing it through command parsing or history.
    ///
    /// # Errors
    ///
    /// Refuses non-terminal input instead of falling back to a script, argv, or
    /// history-bearing reader.
    pub fn prompt_secret(&self, label: &str) -> Result<Secret, ShellError> {
        Secret::prompt(label)
    }

    /// Prompt for one non-secret domain value without recording it as a command.
    ///
    /// # Errors
    ///
    /// Refuses script input so a replay never consumes a following command as
    /// an omitted required value. The value is bounded before it reaches the
    /// domain grammar and is never added to shell history.
    pub fn prompt_value<R, W>(
        &self,
        input: &mut R,
        output: &mut W,
        mode: InputMode,
        label: &str,
    ) -> Result<Option<String>, ShellError>
    where
        R: BufRead,
        W: Write,
    {
        if matches!(mode, InputMode::Script) {
            return Err(ShellError::NonInteractivePrompt);
        }
        output
            .write_all(format!("{label}> ").as_bytes())
            .map_err(|error| output_error(&error))?;
        output.flush().map_err(|error| output_error(&error))?;

        let mut value = String::new();
        if input
            .read_line(&mut value)
            .map_err(|error| output_error(&error))?
            == 0
        {
            return Ok(None);
        }
        let value = value.trim_end().to_owned();
        if value.len() > self.limits.line_bytes() {
            return Err(ShellError::Limit {
                what: "prompt value bytes",
                maximum: self.limits.line_bytes(),
            });
        }
        Ok(Some(value))
    }

    fn use_account(&mut self, alias: &str) -> Result<CommandResult, ShellError> {
        let account = self
            .accounts
            .get(alias)
            .ok_or_else(|| ShellError::UnknownAccount {
                alias: alias.to_owned(),
            })?;
        self.selected_account = Some(alias.to_owned());
        CommandResult::success("account-selected", format!("selected {alias}"))
            .with_field("account", alias)
            .and_then(|result| result.with_field("public_key", account.public_key().to_hex()))
    }

    fn add_relay(&mut self, alias: &str, url: &str) -> Result<CommandResult, ShellError> {
        validate_alias("relay", alias, self.limits.alias_bytes())?;
        if !self.relays.contains_key(alias) && self.relays.len() == self.limits.relays() {
            return Err(ShellError::Limit {
                what: "relay aliases",
                maximum: self.limits.relays(),
            });
        }
        let relay = RelayUrl::parse(url).map_err(|error| ShellError::InvalidRelayUrl {
            input: url.to_owned(),
            reason: error.to_string(),
        })?;
        self.relays.insert(alias.to_owned(), relay.clone());
        CommandResult::success("relay-added", format!("{alias} -> {relay}"))
            .with_field("alias", alias)
            .and_then(|result| result.with_field("relay", relay.to_string()))
    }

    fn capture(&mut self, name: &str, field: &str) -> Result<CommandResult, ShellError> {
        validate_alias("capture", name, self.limits.alias_bytes())?;
        if !self.captures.contains_key(name) && self.captures.len() == self.limits.captures() {
            return Err(ShellError::Limit {
                what: "captures",
                maximum: self.limits.captures(),
            });
        }
        let value = self
            .last_result
            .as_ref()
            .and_then(|result| result.field(field))
            .ok_or_else(|| ShellError::MissingResultField {
                name: field.to_owned(),
            })?;
        if value.len() > self.limits.capture_bytes() {
            return Err(ShellError::Limit {
                what: "capture bytes",
                maximum: self.limits.capture_bytes(),
            });
        }
        self.captures.insert(name.to_owned(), value.to_owned());
        CommandResult::success("capture-set", format!("captured {field} as {name}"))
            .with_field("capture", name)
    }

    fn dump(&self) -> Result<CommandResult, ShellError> {
        let accounts = self.accounts.keys().cloned().collect::<Vec<_>>().join(",");
        let relays = self
            .relays
            .iter()
            .map(|(alias, relay)| format!("{alias}={relay}"))
            .collect::<Vec<_>>()
            .join(",");
        let captures = self.captures.keys().cloned().collect::<Vec<_>>().join(",");
        CommandResult::success("dump", "bounded shell state")
            .with_field(
                "selected_account",
                self.selected_account.as_deref().unwrap_or(""),
            )
            .and_then(|result| result.with_field("accounts", accounts))
            .and_then(|result| result.with_field("relays", relays))
            .and_then(|result| result.with_field("captures", captures))
    }

    fn interpolate(&self, line: &str) -> Result<String, ShellError> {
        let mut expanded = String::with_capacity(line.len());
        let mut remaining = line;
        while let Some(start) = remaining.find("${") {
            expanded.push_str(&remaining[..start]);
            let after_start = &remaining[start + 2..];
            let Some(end) = after_start.find('}') else {
                return Err(ShellError::InvalidCaptureReference {
                    reference: remaining[start..].to_owned(),
                });
            };
            let name = &after_start[..end];
            validate_alias("capture", name, self.limits.alias_bytes())?;
            let value = self
                .captures
                .get(name)
                .ok_or_else(|| ShellError::UnknownCapture {
                    name: name.to_owned(),
                })?;
            expanded.push_str(value);
            if expanded.len() > self.limits.line_bytes() {
                return Err(ShellError::Limit {
                    what: "expanded command line bytes",
                    maximum: self.limits.line_bytes(),
                });
            }
            remaining = &after_start[end + 1..];
        }
        expanded.push_str(remaining);
        if expanded.len() > self.limits.line_bytes() {
            return Err(ShellError::Limit {
                what: "expanded command line bytes",
                maximum: self.limits.line_bytes(),
            });
        }
        Ok(expanded)
    }

    fn record_history(&mut self, line: &str, words: &[String]) -> Result<(), ShellError> {
        if words.iter().any(|word| looks_secret(word)) {
            return Err(ShellError::SecretOnCommandLine);
        }
        if self.history.len() == self.limits.history() {
            self.history.pop_front();
        }
        self.history.push_back(line.to_owned());
        Ok(())
    }
}

fn validate_alias(kind: &'static str, alias: &str, maximum: usize) -> Result<(), ShellError> {
    let valid = !alias.is_empty()
        && alias.len() <= maximum
        && alias.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && alias
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(ShellError::InvalidAlias {
            kind,
            alias: alias.to_owned(),
        })
    }
}

fn split_command(line: &str) -> Result<Vec<String>, ShellError> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for character in line.chars() {
        match (quote, character) {
            (Some(active), character) if character == active => quote = None,
            (None, '\'' | '"') => quote = Some(character),
            (None, character) if character.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if quote.is_some() {
        return Err(ShellError::UnterminatedQuote);
    }
    if !current.is_empty() {
        words.push(current);
    }
    Ok(words)
}

fn looks_secret(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    lower.starts_with("nsec1")
        || lower.starts_with("-----begin")
        || lower.starts_with("secret=")
        || lower.starts_with("password=")
        || lower.starts_with("token=")
}

fn output_error(error: &std::io::Error) -> ShellError {
    ShellError::Output(error.to_string())
}

fn bounded_summary(text: &str, maximum: usize) -> String {
    let mut result = String::new();
    for character in text.chars() {
        if result.len().saturating_add(character.len_utf8()) > maximum {
            break;
        }
        result.push(character);
    }
    result
}
