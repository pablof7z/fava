//! Account, relay, capture, and dump commands owned by the common shell.

use std::sync::Arc;

use fava::RelayUrl;
use fava_signer_local::LocalSigner;
use nostr::key::Keys;

use crate::session::validate_alias;
use crate::{
    Account, CommandResult, E2eSession, InputMode, ResultValue, ShellError, parse_public_key,
};

impl E2eSession {
    pub(crate) fn account_command<P>(
        &mut self,
        action: &str,
        arguments: &[String],
        _mode: InputMode,
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        const USAGE: &str = "account <new|import|add-pubkey|list|switch|replace|remove|clear> ...";
        match action {
            "new" => {
                let alias = required_value(arguments, 0, "account-alias", USAGE, prompt)?;
                if arguments.len() > 1 {
                    return usage(USAGE);
                }
                self.new_account(&alias)
            }
            "import" => {
                let alias = required_value(arguments, 0, "account-alias", USAGE, prompt)?;
                let nsec = required_value(arguments, 1, "account-nsec", USAGE, prompt)?;
                if arguments.len() > 2 {
                    return usage(USAGE);
                }
                self.import_account(&alias, &nsec)
            }
            "add-pubkey" => {
                let alias = required_value(arguments, 0, "account-alias", USAGE, prompt)?;
                let public_key = required_value(arguments, 1, "account-pubkey", USAGE, prompt)?;
                if arguments.len() > 2 {
                    return usage(USAGE);
                }
                self.add_pubkey_account(&alias, &public_key)
            }
            "list" if arguments.is_empty() => self.list_accounts(),
            "switch" => {
                let alias = required_value(arguments, 0, "account-alias", USAGE, prompt)?;
                if arguments.len() > 1 {
                    return usage(USAGE);
                }
                self.switch_account(&alias)
            }
            "replace" => {
                let alias = required_value(arguments, 0, "account-alias", USAGE, prompt)?;
                let nsec = required_value(arguments, 1, "account-nsec", USAGE, prompt)?;
                if arguments.len() > 2 {
                    return usage(USAGE);
                }
                self.replace_account(&alias, &nsec)
            }
            "remove" => {
                let alias = required_value(arguments, 0, "account-alias", USAGE, prompt)?;
                if arguments.len() > 1 {
                    return usage(USAGE);
                }
                self.remove_account(&alias)
            }
            "clear" if arguments.is_empty() => self.clear_account(),
            _ => usage(USAGE),
        }
    }

    pub(crate) fn relay_command<P>(
        &mut self,
        action: &str,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        const USAGE: &str = "relay <add|list|remove> ...";
        match action {
            "add" => {
                let alias = required_value(arguments, 0, "relay-alias", USAGE, prompt)?;
                let url = required_value(arguments, 1, "relay-url", USAGE, prompt)?;
                if arguments.len() > 2 {
                    return usage(USAGE);
                }
                self.add_relay(&alias, &url)
            }
            "list" if arguments.is_empty() => self.list_relays(),
            "remove" => {
                let alias = required_value(arguments, 0, "relay-alias", USAGE, prompt)?;
                if arguments.len() > 1 {
                    return usage(USAGE);
                }
                self.remove_relay(&alias)
            }
            _ => usage(USAGE),
        }
    }

    pub(crate) fn capture_command<P>(
        &mut self,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        const USAGE: &str = "capture <name> <field>";
        let name = required_value(arguments, 0, "capture-name", USAGE, prompt)?;
        let field = required_value(arguments, 1, "result-field", USAGE, prompt)?;
        if arguments.len() > 2 {
            return usage(USAGE);
        }
        self.capture(&name, &field)
    }

    fn new_account(&mut self, alias: &str) -> Result<CommandResult, ShellError> {
        self.add_account(alias, Keys::generate(), "account-created")
    }

    fn import_account(&mut self, alias: &str, nsec: &str) -> Result<CommandResult, ShellError> {
        self.prepare_account_alias(alias)?;
        let keys = Keys::parse(nsec).map_err(|_| ShellError::InvalidImportedAccount)?;
        let public_key = keys.public_key();
        self.fava
            .add_signer(Arc::new(LocalSigner::new(keys)))
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        self.fava
            .select_account(public_key)
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        self.accounts
            .insert(alias.to_owned(), Account::new(alias, public_key));
        self.selected_account = Some(alias.to_owned());
        CommandResult::success(
            "account-imported",
            format!("account-imported and selected {alias}"),
        )
        .with_field("account", alias)
        .and_then(|result| result.with_field("public_key", public_key.to_hex()))
    }

    fn add_account(
        &mut self,
        alias: &str,
        keys: Keys,
        result_kind: &'static str,
    ) -> Result<CommandResult, ShellError> {
        self.prepare_account_alias(alias)?;
        let public_key = keys.public_key();
        self.fava
            .add_signer(Arc::new(LocalSigner::new(keys)))
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        self.fava
            .select_account(public_key)
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        self.accounts
            .insert(alias.to_owned(), Account::new(alias, public_key));
        self.selected_account = Some(alias.to_owned());
        CommandResult::success(result_kind, format!("{result_kind} and selected {alias}"))
            .with_field("account", alias)
            .and_then(|result| result.with_field("public_key", public_key.to_hex()))
    }

    fn add_pubkey_account(
        &mut self,
        alias: &str,
        public_key: &str,
    ) -> Result<CommandResult, ShellError> {
        self.prepare_account_alias(alias)?;
        let public_key = parse_public_key(public_key)?;
        self.fava
            .add_account(public_key)
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        self.fava
            .select_account(public_key)
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        self.accounts
            .insert(alias.to_owned(), Account::new(alias, public_key));
        self.selected_account = Some(alias.to_owned());
        CommandResult::success(
            "account-added",
            format!("account-added and selected {alias}"),
        )
        .with_field("account", alias)
        .and_then(|result| result.with_field("public_key", public_key.to_hex()))
    }

    fn replace_account(&mut self, alias: &str, nsec: &str) -> Result<CommandResult, ShellError> {
        let account = self
            .accounts
            .get(alias)
            .ok_or_else(|| ShellError::UnknownAccount {
                alias: alias.to_owned(),
            })?;
        let keys = Keys::parse(nsec).map_err(|_| ShellError::InvalidImportedAccount)?;
        let actual = keys.public_key();
        if actual != account.public_key() {
            return Err(ShellError::AccountKeyMismatch {
                expected: account.public_key().to_hex(),
                actual: actual.to_hex(),
            });
        }
        self.fava
            .replace_signer(Arc::new(LocalSigner::new(keys)))
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        CommandResult::success("account-replaced", format!("replaced signer for {alias}"))
            .with_field("account", alias)
            .and_then(|result| result.with_field("public_key", actual.to_hex()))
    }

    fn clear_account(&mut self) -> Result<CommandResult, ShellError> {
        self.fava
            .clear_current_account()
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        self.selected_account = None;
        Ok(CommandResult::success(
            "account-cleared",
            "cleared current account",
        ))
    }

    fn prepare_account_alias(&self, alias: &str) -> Result<(), ShellError> {
        validate_alias("account", alias, self.limits.alias_bytes())?;
        if self.accounts.contains_key(alias) {
            return Err(ShellError::DuplicateAccount {
                alias: alias.to_owned(),
            });
        }
        if self.accounts.len() == self.limits.accounts() {
            return Err(ShellError::Limit {
                what: "accounts",
                maximum: self.limits.accounts(),
            });
        }
        Ok(())
    }

    fn switch_account(&mut self, alias: &str) -> Result<CommandResult, ShellError> {
        let account = self
            .accounts
            .get(alias)
            .ok_or_else(|| ShellError::UnknownAccount {
                alias: alias.to_owned(),
            })?;
        self.fava
            .select_account(account.public_key())
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        self.selected_account = Some(alias.to_owned());
        CommandResult::success("account-selected", format!("selected {alias}"))
            .with_field("account", alias)
            .and_then(|result| result.with_field("public_key", account.public_key().to_hex()))
    }

    fn list_accounts(&self) -> Result<CommandResult, ShellError> {
        CommandResult::success("account-list", "known local accounts")
            .with_field(
                "accounts",
                ResultValue::array(self.accounts.keys().cloned().map(ResultValue::text)),
            )?
            .with_field(
                "selected_account",
                self.selected_account.as_deref().unwrap_or(""),
            )
    }

    fn remove_account(&mut self, alias: &str) -> Result<CommandResult, ShellError> {
        let account = self
            .accounts
            .get(alias)
            .ok_or_else(|| ShellError::UnknownAccount {
                alias: alias.to_owned(),
            })?;
        self.fava
            .remove_account(account.public_key())
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        self.accounts.remove(alias);
        if self.selected_account.as_deref() == Some(alias) {
            self.selected_account = None;
        }
        CommandResult::success("account-removed", format!("removed {alias}"))
            .with_field("account", alias)
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

    fn list_relays(&self) -> Result<CommandResult, ShellError> {
        let aliases = ResultValue::array(self.relays.keys().cloned().map(ResultValue::text));
        let urls = ResultValue::array(
            self.relays
                .values()
                .map(|relay| ResultValue::text(relay.to_string())),
        );
        CommandResult::success("relay-list", "known relay aliases")
            .with_field("relay_aliases", aliases)?
            .with_field("relay_urls", urls)
    }

    fn remove_relay(&mut self, alias: &str) -> Result<CommandResult, ShellError> {
        self.relays
            .remove(alias)
            .ok_or_else(|| ShellError::UnknownRelay {
                alias: alias.to_owned(),
            })?;
        CommandResult::success("relay-removed", format!("removed {alias}"))
            .with_field("alias", alias)
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
        let value = value
            .capture_text()
            .ok_or_else(|| ShellError::NonScalarResultField {
                name: field.to_owned(),
            })?;
        if value.len() > self.limits.capture_bytes() {
            return Err(ShellError::Limit {
                what: "capture bytes",
                maximum: self.limits.capture_bytes(),
            });
        }
        self.captures.insert(name.to_owned(), value.clone());
        CommandResult::success("capture-set", format!("captured {field} as {name}"))
            .with_field("capture", name)?
            .with_field("value", value)
    }

    pub(crate) fn dump(&self) -> Result<CommandResult, ShellError> {
        let accounts = ResultValue::array(self.accounts.keys().cloned().map(ResultValue::text));
        let relay_aliases = ResultValue::array(self.relays.keys().cloned().map(ResultValue::text));
        let relay_urls = ResultValue::array(
            self.relays
                .values()
                .map(|relay| ResultValue::text(relay.to_string())),
        );
        let captures = ResultValue::array(self.captures.keys().cloned().map(ResultValue::text));
        CommandResult::success("dump", "bounded shell state")
            .with_field(
                "selected_account",
                self.selected_account.as_deref().unwrap_or(""),
            )
            .and_then(|result| result.with_field("accounts", accounts))
            .and_then(|result| result.with_field("relay_aliases", relay_aliases))
            .and_then(|result| result.with_field("relay_urls", relay_urls))
            .and_then(|result| result.with_field("captures", captures))
    }
}

fn required_value<P>(
    arguments: &[String],
    index: usize,
    label: &str,
    usage: &'static str,
    prompt: &mut P,
) -> Result<String, ShellError>
where
    P: FnMut(&str) -> Result<Option<String>, ShellError>,
{
    match arguments.get(index) {
        Some(value) => Ok(value.clone()),
        None => prompt(label)?.ok_or(ShellError::Usage { usage }),
    }
}

fn usage<T>(usage: &'static str) -> Result<T, ShellError> {
    Err(ShellError::Usage { usage })
}
