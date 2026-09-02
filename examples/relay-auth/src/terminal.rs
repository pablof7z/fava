//! Interactive Reedline presentation for the relay-auth application.

use std::borrow::Cow;
use std::io::Write;
use std::time::Duration;

use e2e_support::{CommandResult, Limits, ResultStatus, ShellError, elide};
use nu_ansi_term::{Color, Style};
use reedline::{
    ColumnarMenu, Completer, Emacs, FileBackedHistory, Hinter, History, KeyCode, KeyModifiers,
    MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus, Reedline,
    ReedlineEvent, ReedlineMenu, Signal, Span, Suggestion, default_emacs_keybindings,
};

const MENU: &str = "relay-auth-completion";

pub(crate) struct Terminal {
    command: Reedline,
    value: Reedline,
    limits: Limits,
    color: bool,
}

impl Terminal {
    pub(crate) fn new(limits: Limits, color: bool) -> Result<Self, reedline::ReedlineError> {
        let mut keys = default_emacs_keybindings();
        keys.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu(MENU.to_owned()),
                ReedlineEvent::MenuNext,
            ]),
        );
        let menu = Box::new(ColumnarMenu::default().with_name(MENU));
        let command = Reedline::create()
            .with_ansi_colors(color)
            .with_history(Box::new(FileBackedHistory::new(32)?))
            .with_hinter(Box::new(RelayAuthHinter { color }))
            .with_completer(Box::new(RelayAuthCompleter))
            .with_menu(ReedlineMenu::EngineCompleter(menu))
            .with_edit_mode(Box::new(Emacs::new(keys)));
        let value = Reedline::create()
            .with_ansi_colors(color)
            .with_history(Box::new(FileBackedHistory::new(0)?));
        Ok(Self {
            command,
            value,
            limits,
            color,
        })
    }

    pub(crate) fn write_intro(&self, output: &mut impl Write) -> std::io::Result<()> {
        writeln!(
            output,
            "{}  {}",
            paint(
                self.color,
                Style::new().bold().fg(Color::LightCyan),
                "fava relay-auth"
            ),
            paint(
                self.color,
                Style::new().fg(Color::DarkGray),
                "NIP-42 relay-authentication shell"
            )
        )?;
        writeln!(
            output,
            "{}",
            paint(
                self.color,
                Style::new().fg(Color::DarkGray),
                "Tab complete · ↑↓ history · Ctrl-D close · help for grammar"
            )
        )
    }

    pub(crate) fn read_command(
        &mut self,
        account: Option<&str>,
        relays: usize,
        queries: usize,
    ) -> std::io::Result<Signal> {
        let prompt = RelayAuthPrompt::new(account, relays, queries, width(), self.color);
        self.command.read_line(&prompt)
    }

    pub(crate) fn read_value(&mut self, label: &str) -> Result<Option<String>, ShellError> {
        let prompt = ValuePrompt {
            label: label.to_owned(),
            color: self.color,
        };
        match self
            .value
            .read_line(&prompt)
            .map_err(|error| ShellError::Output(error.to_string()))?
        {
            Signal::Success(value) => {
                self.limits.validate_prompt_value(&value)?;
                Ok(Some(value))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn render_result(
        &self,
        output: &mut impl Write,
        result: &CommandResult,
        elapsed: Duration,
    ) -> std::io::Result<()> {
        let (symbol, style) = match result.status() {
            ResultStatus::Ok => ("✓", Style::new().bold().fg(Color::LightGreen)),
            ResultStatus::Refused => ("!", Style::new().bold().fg(Color::Yellow)),
            ResultStatus::Failed => ("×", Style::new().bold().fg(Color::LightRed)),
        };
        writeln!(
            output,
            "{}  {}  {}ms",
            paint(self.color, style, symbol),
            paint(self.color, style, result.kind()),
            elapsed.as_millis()
        )?;
        if !result.summary().is_empty() {
            writeln!(output, "   {}", result.summary())?;
        }
        for (name, value) in result.fields() {
            writeln!(output, "   {name:<22} {value:?}")?;
        }
        Ok(())
    }

    pub(crate) fn render_cancelled(&self, output: &mut impl Write) -> std::io::Result<()> {
        writeln!(
            output,
            "{}",
            paint(self.color, Style::new().fg(Color::DarkGray), "cancelled")
        )
    }
}

struct RelayAuthPrompt {
    left: String,
    color: bool,
}

impl RelayAuthPrompt {
    fn new(
        account: Option<&str>,
        relays: usize,
        queries: usize,
        width: usize,
        color: bool,
    ) -> Self {
        let account = account.unwrap_or("no-account");
        let plain = format!("{account} · {relays}r · {queries}q");
        let available = width.saturating_sub(4);
        let plain = elide(&plain, available);
        Self { left: plain, color }
    }
}

impl Prompt for RelayAuthPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Owned(paint(
            self.color,
            Style::new().bold().fg(Color::LightCyan),
            &self.left,
        ))
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(" › ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(" … ")
    }

    fn render_prompt_history_search_indicator(&self, search: PromptHistorySearch) -> Cow<'_, str> {
        let state = match search.status {
            PromptHistorySearchStatus::Passing => "search",
            PromptHistorySearchStatus::Failing => "search?",
        };
        Cow::Owned(format!("{state}: {}", search.term))
    }
}

struct ValuePrompt {
    label: String,
    color: bool,
}

impl Prompt for ValuePrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Owned(paint(
            self.color,
            Style::new().fg(Color::DarkGray),
            &self.label,
        ))
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(" › ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(" … ")
    }

    fn render_prompt_history_search_indicator(&self, _: PromptHistorySearch) -> Cow<'_, str> {
        Cow::Borrowed("search: ")
    }
}

struct RelayAuthCompleter;

impl Completer for RelayAuthCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let line = line.get(..pos).unwrap_or(line);
        let start = line
            .char_indices()
            .rev()
            .find_map(|(index, character)| character.is_whitespace().then_some(index + 1))
            .unwrap_or(0);
        let prefix = &line[start..];
        completions(line[..start].trim_end())
            .iter()
            .filter(|value| value.starts_with(prefix))
            .map(|value| Suggestion {
                value: (*value).to_owned(),
                description: Some(description(value).to_owned()),
                style: Some(Style::new().fg(Color::LightCyan)),
                span: Span::new(start, pos),
                append_whitespace: true,
                ..Suggestion::default()
            })
            .collect()
    }
}

struct RelayAuthHinter {
    color: bool,
}

impl Hinter for RelayAuthHinter {
    fn handle(&mut self, line: &str, _: usize, _: &dyn History, _: bool, _: &str) -> String {
        let Some(hint) = hint(line.trim_end()) else {
            return String::new();
        };
        paint(self.color, Style::new().italic().fg(Color::DarkGray), hint)
    }

    fn complete_hint(&self) -> String {
        String::new()
    }

    fn next_hint_token(&self) -> String {
        String::new()
    }
}

fn completions(context: &str) -> &'static [&'static str] {
    match context {
        "account" => &[
            "new",
            "import",
            "add-pubkey",
            "list",
            "switch",
            "replace",
            "remove",
            "clear",
        ],
        "relay" => &["add", "list", "remove"],
        "policy" => &["set"],
        "policy set" => &["authenticate", "decline", "defer"],
        "auth" => &["pending", "answer", "state"],
        "query" => &["open", "snapshot", "wait", "close"],
        "auth state" | "query open" | "publish" => &["public"],
        "receipt" => &["list", "show", "wait"],
        _ => &[
            "account",
            "relay",
            "policy",
            "auth",
            "query",
            "publish",
            "receipt",
            "diagnostics",
            "routes",
            "capture",
            "dump",
            "help",
            "quit",
        ],
    }
}

fn description(value: &str) -> &'static str {
    match value {
        "public" => "the unauthenticated relay lane",
        "policy" => "the runtime NIP-42 decision for future challenges",
        "auth" => "inspect and answer NIP-42 demands",
        "publish" => "publish an explicit-kind event, public or authenticated",
        "query" => "open or inspect a query under a public or authenticated lane",
        "diagnostics" => "show public ownership facts",
        "routes" => "show active query demand states, including auth demand",
        _ => "command",
    }
}

fn hint(line: &str) -> Option<&'static str> {
    match line {
        "account" => Some(" <new|import|add-pubkey|list|switch|replace|remove|clear> ..."),
        "relay" => Some(" <add|list|remove> ..."),
        "policy" => Some(" set <authenticate|decline|defer>"),
        "policy set" => Some(" <authenticate|decline|defer>"),
        "auth" => Some(" <pending|answer|state> ..."),
        "auth answer" => Some(" <relay-url> <connection> <authenticate|decline>"),
        "auth state" => Some(" <relay-alias> <public|as:<account>>"),
        "query" => Some(" <open|snapshot|wait|close> ..."),
        "query open" => Some(" <name> <public|as:<account>> <kind> <relay> [relay ...]"),
        "publish" => {
            Some(" <public|as:<account>> [for <author>] <kind> <content> <relay> [relay ...]")
        }
        "receipt" => Some(" <list|show|wait> ..."),
        "capture" => Some(" <name> <last-result-field>"),
        _ => None,
    }
}

fn paint(color: bool, style: Style, value: &str) -> String {
    if color {
        style.paint(value).to_string()
    } else {
        value.to_owned()
    }
}

fn width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(80)
}

#[cfg(test)]
mod tests {
    use reedline::{Completer, Prompt};

    use super::{RelayAuthCompleter, RelayAuthPrompt, completions, hint};

    #[test]
    fn completion_and_hints_cover_the_access_grammar() {
        assert!(completions("query").contains(&"open"));
        assert_eq!(completions("query open"), &["public"]);
        assert!(hint("query open").expect("hint").contains("as:<account>"));
        let mut completer = RelayAuthCompleter;
        assert!(
            completer
                .complete("auth st", 7)
                .iter()
                .any(|item| item.value == "state")
        );
    }

    #[test]
    fn narrow_prompt_elides_context_within_width() {
        let prompt = RelayAuthPrompt::new(Some("a-very-long-account-name"), 8, 4, 24, false);
        let rendered = format!("{} › ", prompt.render_prompt_left());
        assert!(rendered.chars().count() <= 24, "{rendered:?}");
        assert!(rendered.contains('…'));
    }
}
