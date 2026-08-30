//! Reedline prompt, highlighting, hints, and completion wiring.

use e2e_support::elide;
use std::borrow::Cow;

use nu_ansi_term::{Color, Style};
use reedline::{
    Completer, Highlighter, Hinter, History, Prompt, PromptEditMode, PromptHistorySearch,
    PromptHistorySearchStatus, Span, StyledText, Suggestion,
};

use crate::terminal::Theme;
use crate::terminal_completion::{is_command, suggestions};

pub(crate) struct ShellPrompt {
    left: String,
    theme: Theme,
}

impl ShellPrompt {
    pub(crate) fn new(
        account: Option<&str>,
        group: Option<&str>,
        relays: usize,
        width: usize,
        theme: Theme,
    ) -> Self {
        let account = account.unwrap_or("no-account");
        let group = group.unwrap_or("no-group");
        let relay_label = if relays == 1 { "relay" } else { "relays" };
        let relay = format!("{relays} {relay_label}");
        let left = prompt_left(account, group, &relay, width, theme);
        Self { left, theme }
    }
}

fn prompt_left(account: &str, group: &str, relay: &str, width: usize, theme: Theme) -> String {
    if width < 16 {
        return format!(
            "{} ",
            theme.muted(&elide(
                &format!("{account}/{group}/{relay}"),
                width.saturating_sub(4)
            ))
        );
    }
    // Reserve two cells for the indicator and one for the input cursor. The
    // remaining fixed presentation is two spaced separators and a trailing
    // gap, so only exact source values are elided.
    let variable = width.saturating_sub(10);
    let relay_width = relay.chars().count().min(variable.saturating_sub(2));
    let names_width = variable.saturating_sub(relay_width);
    let account = elide(account, names_width / 2);
    let group = elide(group, names_width.saturating_sub(names_width / 2));
    let relay = elide(relay, relay_width);
    format!(
        "{} {} {} {} {} ",
        theme.account(&account),
        theme.muted("·"),
        theme.group(&group),
        theme.muted("·"),
        theme.connected(&relay),
    )
}

impl Prompt for ShellPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.left)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        Cow::Owned(self.theme.marker("› "))
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("… ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        let label = match history_search.status {
            PromptHistorySearchStatus::Passing => "search",
            PromptHistorySearchStatus::Failing => "search?",
        };
        Cow::Owned(format!("{label}: {}", history_search.term))
    }
}

pub(crate) struct ValuePrompt {
    left: String,
    theme: Theme,
}

impl ValuePrompt {
    pub(crate) fn new(label: &str, theme: Theme) -> Self {
        Self {
            left: format!("{} ", theme.muted(label)),
            theme,
        }
    }
}

impl Prompt for ValuePrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.left)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        Cow::Owned(self.theme.value_marker("› "))
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("… ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        Cow::Owned(format!("search: {}", history_search.term))
    }
}

pub(crate) struct ShellHighlighter;

impl Highlighter for ShellHighlighter {
    fn highlight(&self, line: &str, _: usize) -> StyledText {
        let mut styled = StyledText::new();
        for token in line.split_inclusive(char::is_whitespace) {
            let word = token.trim();
            let style = if is_command(word) {
                Style::new().bold().fg(Color::LightCyan)
            } else if word.starts_with("--") {
                Style::new().fg(Color::LightPurple)
            } else if word.parse::<u16>().is_ok() {
                Style::new().fg(Color::Yellow)
            } else if word.starts_with('"') || word.starts_with('\'') {
                Style::new().fg(Color::LightGreen)
            } else {
                Style::new().fg(Color::White)
            };
            styled.push((style, token.to_owned()));
        }
        styled
    }
}

pub(crate) struct UsageHinter {
    pub(crate) color: bool,
}

impl Hinter for UsageHinter {
    fn handle(&mut self, line: &str, _: usize, _: &dyn History, _: bool, _: &str) -> String {
        let Some(hint) = usage_hint(line.trim_end()) else {
            return String::new();
        };
        if self.color {
            Style::new()
                .italic()
                .fg(Color::DarkGray)
                .paint(hint)
                .to_string()
        } else {
            hint.to_owned()
        }
    }

    fn complete_hint(&self) -> String {
        String::new()
    }

    fn next_hint_token(&self) -> String {
        String::new()
    }
}

pub(crate) struct ShellCompleter;

impl Completer for ShellCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let line = line.get(..pos).unwrap_or(line);
        let start = line
            .char_indices()
            .rev()
            .find_map(|(index, character)| character.is_whitespace().then_some(index + 1))
            .unwrap_or(0);
        let prefix = &line[start..];
        let context = line[..start].trim_end();
        suggestions(context)
            .iter()
            .filter(|entry| entry.name.starts_with(prefix))
            .map(|entry| Suggestion {
                value: entry.name.to_owned(),
                description: Some(entry.description.to_owned()),
                style: Some(Style::new().fg(Color::LightCyan)),
                span: Span::new(start, pos),
                append_whitespace: true,
                ..Suggestion::default()
            })
            .collect()
    }
}

fn usage_hint(line: &str) -> Option<&'static str> {
    match line {
        "account" => Some(" <new|import|list|switch|remove> ..."),
        "relay" => Some(" <add|list|remove> ..."),
        "group" => Some(
            " <create|open|list|switch|edit|invite|join|member|leave|delete|event|events|state> ...",
        ),
        "group create" | "group open" => Some(" <id> <relay-alias> [relay-alias ...]"),
        "group edit" => Some(" [--name <text>] [--about <text>] [--private|--public] ..."),
        "group event" => Some(" <publish|expect-rejection|delete> ..."),
        "group event publish" | "group event expect-rejection" => Some(" --kind <kind> [content]"),
        "group state" | "group events" => Some(" [limit]"),
        "saved-list" => Some(" <show|group|relay> ..."),
        "receipt" => Some(" <list|show> ..."),
        "receipt list" => Some("  # open publication obligations"),
        "receipt show" => Some(" <receipt-id>"),
        "capture" => Some(" <alias> <last-result-field>"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use reedline::Completer;

    use crate::terminal::Theme;

    use super::{ShellCompleter, ShellPrompt, usage_hint};

    #[test]
    fn completion_covers_group_actions_and_options() {
        let mut completer = ShellCompleter;
        assert!(
            completer
                .complete("group ev", 8)
                .iter()
                .any(|suggestion| suggestion.value == "event")
        );
        assert!(
            completer
                .complete("group edit --", 13)
                .iter()
                .any(|suggestion| suggestion.value == "--supported-kinds")
        );
    }

    #[test]
    fn usage_hints_describe_partial_group_commands() {
        assert_eq!(
            usage_hint("group event publish"),
            Some(" --kind <kind> [content]")
        );
    }

    #[test]
    fn receipt_completion_and_hints_cover_the_documented_subcommands() {
        let mut completer = ShellCompleter;
        let completions = completer.complete("receipt ", 8);
        assert!(
            completions
                .iter()
                .any(|suggestion| suggestion.value == "list")
        );
        assert!(
            completions
                .iter()
                .any(|suggestion| suggestion.value == "show")
        );
        assert_eq!(usage_hint("receipt show"), Some(" <receipt-id>"));
    }

    #[test]
    fn legal_names_fit_a_40_column_prompt_with_explicit_elision() {
        let account = "a".repeat(32);
        let group = "g".repeat(32);
        let prompt = ShellPrompt::new(Some(&account), Some(&group), 1, 40, Theme::new(false));
        assert_eq!(prompt.left, "aaaaaaaaaa… · ggggggggggg… · 1 relay ");
        assert_eq!(prompt.left.chars().count() + 2, 39);
        assert!(!prompt.left.contains(&account));
        assert!(!prompt.left.contains(&group));
    }

    #[test]
    fn narrow_prompt_elides_the_relay_context_too() {
        let prompt = ShellPrompt::new(
            Some("account"),
            Some("group"),
            123_456,
            20,
            Theme::new(false),
        );
        assert!(prompt.left.contains("123456 …"));
        assert!(prompt.left.chars().count() + 2 < 20);
    }
}
