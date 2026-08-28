//! Shared interactive and script command execution for concrete E2E examples.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, Write};

use fava::{Fava, RelayUrl};

use crate::ingress::{looks_secret, reject_unsafe_words};
use crate::result::sensitive_value;
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
    pub(crate) limits: Limits,
    pub(crate) fava: Fava,
    pub(crate) accounts: BTreeMap<String, Account>,
    pub(crate) selected_account: Option<String>,
    pub(crate) relays: BTreeMap<String, RelayUrl>,
    pub(crate) captures: BTreeMap<String, String>,
    pub(crate) history: VecDeque<String>,
    pub(crate) last_result: Option<CommandResult>,
    quitting: bool,
}

impl E2eSession {
    /// Construct a shell with no accounts or relay aliases.
    ///
    /// Accounts enter only through the protected shared commands. Their local
    /// signers are attached through this public Fava handle; the shell never
    /// reaches into Fava session state.
    #[must_use]
    pub fn new(limits: Limits, fava: Fava) -> Self {
        Self {
            limits,
            fava,
            accounts: BTreeMap::new(),
            selected_account: None,
            relays: BTreeMap::new(),
            captures: BTreeMap::new(),
            history: VecDeque::new(),
            last_result: None,
            quitting: false,
        }
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
        if matches!(mode, InputMode::Interactive) && matches!(format, OutputFormat::JsonLines) {
            return Err(ShellError::InteractiveJsonLines);
        }
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
            let limits = self.limits;
            let streams = RefCell::new((&mut *input, &mut *output));
            let execution = self.execute_line_with_prompt(
                line.trim_end(),
                mode,
                |label| {
                    let mut streams = streams.borrow_mut();
                    let (input, output) = &mut *streams;
                    prompt_value(limits, *input, *output, mode, label)
                },
                |session, words| {
                    let mut streams = streams.borrow_mut();
                    let (input, output) = &mut *streams;
                    domain(session, words, *input, *output, mode)
                },
            );
            let (rendered, failure) = match execution {
                Ok(result) => (format.render(&result)?, None),
                Err(ShellError::CommandFailed { result }) => {
                    let rendered = format.render(&result)?;
                    (rendered, Some(ShellError::CommandFailed { result }))
                }
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
    pub fn execute_line<F>(&mut self, line: &str, domain: F) -> Result<CommandResult, ShellError>
    where
        F: FnMut(&mut Self, &[String]) -> Result<CommandResult, ShellError>,
    {
        self.execute_line_with_prompt(
            line,
            InputMode::Script,
            |_| Err(ShellError::NonInteractivePrompt),
            domain,
        )
    }

    /// Execute one line through the shared parser and dispatcher with an
    /// application-supplied ordinary-value prompt.
    ///
    /// The runtime uses this for interactive and script streams alike. The
    /// prompt decides whether an omitted required value can be supplied; it
    /// must return [`ShellError::NonInteractivePrompt`] for a replay.
    ///
    /// # Errors
    ///
    /// Refuses oversized, secret-looking, malformed, or unknown retained input
    /// before it reaches the real domain command implementation.
    pub fn execute_line_with_prompt<F, P>(
        &mut self,
        line: &str,
        mode: InputMode,
        mut prompt: P,
        mut domain: F,
    ) -> Result<CommandResult, ShellError>
    where
        F: FnMut(&mut Self, &[String]) -> Result<CommandResult, ShellError>,
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        self.execute_line_with_domain_prompt(line, mode, &mut prompt, |session, words, _| {
            domain(session, words)
        })
    }

    /// Execute one line while giving the domain dispatcher the ordinary-value
    /// prompt used by the shared shell commands.
    ///
    /// This keeps command parsing, secret refusal, bounded history, and
    /// result retention in this shell while allowing a concrete interactive
    /// frontend to own its terminal line editor for both command and value
    /// entry.
    ///
    /// # Errors
    ///
    /// Refuses oversized, secret-looking, malformed, or unknown retained input
    /// before it reaches the real domain command implementation.
    pub fn execute_line_with_domain_prompt<F, P>(
        &mut self,
        line: &str,
        mode: InputMode,
        prompt: &mut P,
        mut domain: F,
    ) -> Result<CommandResult, ShellError>
    where
        F: FnMut(&mut Self, &[String], &mut P) -> Result<CommandResult, ShellError>,
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
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
        reject_unsafe_words(&words)?;
        self.record_history(&expanded, &words)?;
        let result = match words.as_slice() {
            [command, action, arguments @ ..] if command == "account" => {
                self.account_command(action, arguments, mode, prompt)
            }
            [command] if command == "account" => Err(ShellError::Usage {
                usage: "account <new|import|list|switch|remove> ...",
            }),
            [command, action, arguments @ ..] if command == "relay" => {
                self.relay_command(action, arguments, prompt)
            }
            [command] if command == "relay" => Err(ShellError::Usage {
                usage: "relay <add|list|remove> ...",
            }),
            [command, arguments @ ..] if command == "capture" => {
                self.capture_command(arguments, prompt)
            }
            [command] if command == "dump" => self.dump(),
            [command] if command == "quit" || command == "exit" => {
                self.quitting = true;
                Ok(CommandResult::success("quit", "session closed"))
            }
            _ => domain(self, &words, prompt),
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

    /// Return the selected account alias without requiring one to exist.
    #[must_use]
    pub fn selected_account_alias(&self) -> Option<&str> {
        self.selected_account.as_deref()
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

    /// Return the currently retained public relay-alias count.
    #[must_use]
    pub fn relay_count(&self) -> usize {
        self.relays.len()
    }

    /// Refuse a value that cannot safely become one capture-safe result field.
    ///
    /// Domain commands call this before publishing content they promise to
    /// expose exactly in their terminal result, so a renderer bound never
    /// turns an already-accepted write into an unreportable command outcome.
    ///
    /// # Errors
    ///
    /// Returns a typed limit refusal before the caller can accept work whose
    /// exact value would exceed the one result-field bound.
    pub fn validate_result_value(&self, value: &str) -> Result<(), ShellError> {
        if sensitive_value(value) {
            Err(ShellError::SecretOnCommandLine)
        } else if value.len() > self.limits.capture_bytes() {
            Err(ShellError::Limit {
                what: "result field bytes",
                maximum: self.limits.capture_bytes(),
            })
        } else {
            Ok(())
        }
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
        prompt_value(self.limits, input, output, mode, label)
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

fn prompt_value<R, W>(
    limits: Limits,
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
    limits.validate_prompt_value(label, &value)?;
    Ok(Some(value))
}

pub(crate) fn validate_alias(
    kind: &'static str,
    alias: &str,
    maximum: usize,
) -> Result<(), ShellError> {
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
