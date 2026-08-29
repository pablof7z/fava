//! Interactive-only rendering and safe editor assembly.

use std::io::Write;
use std::time::Duration;

use e2e_support::{CommandResult, Limits, ResultStatus, ResultValue, ShellError};
use nu_ansi_term::{Color, Style};
use reedline::{
    ColumnarMenu, Emacs, FileBackedHistory, KeyCode, KeyModifiers, MenuBuilder, Reedline,
    ReedlineEvent, ReedlineMenu, Signal, default_emacs_keybindings,
};

use crate::terminal_editor::{
    ShellCompleter, ShellHighlighter, ShellPrompt, UsageHinter, ValuePrompt,
};

const HISTORY_LIMIT: usize = 16;
const MENU_NAME: &str = "simple_groups_completion";

pub(crate) struct Terminal {
    command_editor: Reedline,
    value_editor: Reedline,
    limits: Limits,
    theme: Theme,
}

impl Terminal {
    pub(crate) fn new(limits: Limits, color: bool) -> Result<Self, reedline::ReedlineError> {
        let theme = Theme::new(color);
        let mut keybindings = default_emacs_keybindings();
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu(MENU_NAME.to_owned()),
                ReedlineEvent::MenuNext,
            ]),
        );
        let completion_menu = Box::new(ColumnarMenu::default().with_name(MENU_NAME));
        let command_editor = Reedline::create()
            .with_ansi_colors(color)
            .with_history(Box::new(FileBackedHistory::new(HISTORY_LIMIT)?))
            .with_hinter(Box::new(UsageHinter { color }))
            .with_completer(Box::new(ShellCompleter))
            .with_highlighter(Box::new(ShellHighlighter))
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
            .with_edit_mode(Box::new(Emacs::new(keybindings)));
        let value_editor = Reedline::create()
            .with_ansi_colors(color)
            .with_history(Box::new(FileBackedHistory::new(0)?))
            .with_highlighter(Box::new(ShellHighlighter));
        Ok(Self {
            command_editor,
            value_editor,
            limits,
            theme,
        })
    }

    pub(crate) fn write_intro(&self, output: &mut impl Write) -> std::io::Result<()> {
        writeln!(
            output,
            "{}  {}",
            self.theme.brand("fava simple-groups"),
            self.theme.muted("interactive NIP-29 shell")
        )?;
        writeln!(
            output,
            "{}",
            self.theme
                .muted("Tab complete · ↑↓ history · Ctrl-R search · Ctrl-D close · --help grammar")
        )
    }

    pub(crate) fn read_command(
        &mut self,
        account: Option<&str>,
        group: Option<&str>,
        relays: usize,
    ) -> std::io::Result<Signal> {
        let prompt = ShellPrompt::new(account, group, relays, terminal_width(), self.theme);
        self.command_editor.read_line(&prompt)
    }

    pub(crate) fn read_value(&mut self, label: &str) -> Result<Option<String>, ShellError> {
        let prompt = ValuePrompt::new(label, self.theme);
        match self
            .value_editor
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
            "{}  {}{}",
            self.theme.paint(style, symbol),
            self.theme.paint(style, &result_heading(result)),
            self.theme.muted(&receipt_emphasis(result, elapsed))
        )?;
        if !matches!(result.status(), ResultStatus::Ok) {
            writeln!(output, "   {}", self.theme.muted(result.summary()))?;
        }
        self.render_fields(output, result)
    }

    pub(crate) fn render_cancelled(&self, output: &mut impl Write) -> std::io::Result<()> {
        writeln!(output, "{}", self.theme.muted("input cancelled"))
    }

    fn render_fields(
        &self,
        output: &mut impl Write,
        result: &CommandResult,
    ) -> std::io::Result<()> {
        let paired = paired_table(result);
        for (name, value) in result.fields() {
            if paired.is_some_and(|(left, right)| name == left || name == right)
                || receipt_field(name)
            {
                continue;
            }
            self.render_field(output, name, value)?;
        }
        if let Some((left, right)) = paired {
            self.render_table(output, left, right, result)?;
        }
        self.render_routes(output, result)
    }

    fn render_field(
        &self,
        output: &mut impl Write,
        name: &str,
        value: &ResultValue,
    ) -> std::io::Result<()> {
        let width = terminal_width();
        let label = if width < 52 {
            abbreviate(field_label(name), width.saturating_sub(6).max(1))
        } else {
            field_label(name).to_owned()
        };
        let value = value_text(value, field_value_width(&label, width));
        if width < 52 {
            writeln!(
                output,
                "   {}: {}",
                self.theme.muted(&label),
                self.theme.field_value(name, &value)
            )
        } else {
            writeln!(
                output,
                "   {:<14} {}",
                self.theme.muted(&label),
                self.theme.field_value(name, &value)
            )
        }
    }

    fn render_table(
        &self,
        output: &mut impl Write,
        left: &str,
        right: &str,
        result: &CommandResult,
    ) -> std::io::Result<()> {
        let (Some(ResultValue::Array(left_values)), Some(ResultValue::Array(right_values))) =
            (result.field(left), result.field(right))
        else {
            return Ok(());
        };
        if left_values.is_empty() {
            return Ok(());
        }
        if terminal_width() < 52 {
            let width = terminal_width().saturating_sub(16).max(1) / 2;
            for (left_value, right_value) in left_values.iter().zip(right_values) {
                writeln!(
                    output,
                    "   {}: {} · {}",
                    self.theme.muted(field_label(left)),
                    self.theme.field_value(left, &value_text(left_value, width)),
                    self.theme
                        .field_value(right, &value_text(right_value, width))
                )?;
            }
            return Ok(());
        }
        writeln!(
            output,
            "   {}  {}",
            self.theme.muted(field_label(left)),
            self.theme.muted(field_label(right))
        )?;
        for (left_value, right_value) in left_values.iter().zip(right_values) {
            writeln!(
                output,
                "   {:<14} {}",
                self.theme.field_value(left, &value_text(left_value, 24)),
                self.theme.field_value(right, &value_text(right_value, 16))
            )?;
        }
        Ok(())
    }

    fn render_routes(
        &self,
        output: &mut impl Write,
        result: &CommandResult,
    ) -> std::io::Result<()> {
        let (
            Some(ResultValue::Array(relays)),
            Some(ResultValue::Array(outcomes)),
            Some(ResultValue::Array(reasons)),
        ) = (
            result.field("destination_relays"),
            result.field("delivery_outcomes"),
            result.field("delivery_reasons"),
        )
        else {
            return Ok(());
        };
        let narrow = terminal_width() < 52;
        for ((relay, outcome), reason) in relays.iter().zip(outcomes).zip(reasons) {
            let relay = value_text(
                relay,
                terminal_width().saturating_sub(if narrow { 19 } else { 32 }),
            );
            let outcome = value_text(outcome, 24);
            let reason_width = terminal_width()
                .saturating_sub(relay.chars().count() + outcome.chars().count() + 12);
            let reason = value_text(reason, if narrow { reason_width } else { 48 });
            let suffix = (!reason.is_empty()).then(|| format!(" · {reason}"));
            if narrow {
                writeln!(
                    output,
                    "   {}: {} {}{}",
                    self.theme.muted("route"),
                    self.theme.relay(&relay),
                    self.theme.outcome(&outcome),
                    self.theme.muted(suffix.as_deref().unwrap_or_default())
                )?;
            } else {
                writeln!(
                    output,
                    "   {:<14} {} {}{}",
                    self.theme.muted("route"),
                    self.theme.relay(&relay),
                    self.theme.outcome(&outcome),
                    self.theme.muted(suffix.as_deref().unwrap_or_default())
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Theme {
    color: bool,
}

impl Theme {
    pub(crate) const fn new(color: bool) -> Self {
        Self { color }
    }

    pub(crate) fn paint(self, style: Style, value: &str) -> String {
        if self.color {
            style.paint(value).to_string()
        } else {
            value.to_owned()
        }
    }

    pub(crate) fn muted(self, value: &str) -> String {
        self.paint(Style::new().fg(Color::DarkGray), value)
    }

    pub(crate) fn account(self, value: &str) -> String {
        self.paint(Style::new().bold().fg(Color::LightCyan), value)
    }

    pub(crate) fn group(self, value: &str) -> String {
        self.paint(Style::new().bold().fg(Color::LightPurple), value)
    }

    pub(crate) fn connected(self, value: &str) -> String {
        self.paint(Style::new().fg(Color::LightGreen), value)
    }

    pub(crate) fn marker(self, value: &str) -> String {
        self.paint(Style::new().bold().fg(Color::LightGreen), value)
    }

    pub(crate) fn value_marker(self, value: &str) -> String {
        self.paint(Style::new().fg(Color::LightCyan), value)
    }

    fn brand(self, value: &str) -> String {
        self.account(value)
    }

    fn relay(self, value: &str) -> String {
        self.paint(Style::new().fg(Color::LightCyan), value)
    }

    fn outcome(self, value: &str) -> String {
        let color = match value {
            "acknowledged" | "complete" => Color::LightGreen,
            "rejected" | "given-up" | "authentication-denied" => Color::LightRed,
            _ => Color::Yellow,
        };
        self.paint(Style::new().fg(color), value)
    }

    fn field_value(self, name: &str, value: &str) -> String {
        let style = match name {
            "event_id" | "public_key" => Style::new().fg(Color::DarkGray),
            "relay" | "destination_relays" => Style::new().fg(Color::LightCyan),
            "kind" | "count" | "acknowledged" | "desired" => Style::new().fg(Color::Yellow),
            "group" | "account" | "active_account" | "active_group" => {
                Style::new().bold().fg(Color::LightPurple)
            }
            _ => Style::new().fg(Color::White),
        };
        self.paint(style, value)
    }
}

fn result_heading(result: &CommandResult) -> String {
    match result.kind() {
        "group-event-published" => "event acknowledged".to_owned(),
        "group-state" => "relay state".to_owned(),
        "group-events" => "group events".to_owned(),
        "shell-refused" => "command refused".to_owned(),
        "domain-failed" => "command failed".to_owned(),
        kind => kind.replace('-', " "),
    }
}

fn receipt_emphasis(result: &CommandResult, elapsed: Duration) -> String {
    let timing = format!("{} ms", elapsed.as_millis());
    let acknowledgement = result.field("acknowledged").and_then(number);
    let desired = result.field("desired").and_then(number);
    let outcome = result.field("outcome").and_then(text);
    match (acknowledgement, desired, outcome) {
        (Some(acknowledgement), Some(desired), Some(outcome)) => {
            format!("  {acknowledgement}/{desired} relays · {outcome} · {timing}")
        }
        _ => format!("  {timing}"),
    }
}

fn paired_table(result: &CommandResult) -> Option<(&'static str, &'static str)> {
    [("decoded", "decoded_counts"), ("kinds", "kind_counts")]
        .into_iter()
        .find(|(left, right)| result.field(left).is_some() && result.field(right).is_some())
}

fn receipt_field(name: &str) -> bool {
    matches!(
        name,
        "acknowledged"
            | "desired"
            | "outcome"
            | "rejected"
            | "destination_relays"
            | "delivery_outcomes"
            | "delivery_reasons"
            | "delivery_shortfall"
    )
}

fn field_label(name: &str) -> &str {
    match name {
        "event_id" => "event",
        "public_key" => "public key",
        "relay_aliases" => "relay aliases",
        "relay_urls" => "relay URLs",
        "active_account" => "account",
        "active_group" => "group",
        "stored_events_complete" => "EOSE",
        "decoded_counts" | "kind_counts" => "count",
        other => other,
    }
}

fn value_text(value: &ResultValue, width: usize) -> String {
    let value = match value {
        ResultValue::Text(value) if value.is_empty() => "—".to_owned(),
        ResultValue::Text(value) => value.clone(),
        ResultValue::Integer(value) => value.to_string(),
        ResultValue::Boolean(value) => value.to_string(),
        ResultValue::Array(values) => values
            .iter()
            .map(|value| value_text(value, width))
            .collect::<Vec<_>>()
            .join(", "),
    };
    abbreviate(&value, width)
}

fn abbreviate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        value.to_owned()
    } else {
        format!(
            "{}…",
            value
                .chars()
                .take(width.saturating_sub(1))
                .collect::<String>()
        )
    }
}

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width > 0)
        .unwrap_or(88)
}

fn field_value_width(label: &str, width: usize) -> usize {
    let used = if width < 52 {
        5 + label.chars().count()
    } else {
        18
    };
    width.saturating_sub(used).max(1)
}

fn text(value: &ResultValue) -> Option<&str> {
    match value {
        ResultValue::Text(value) => Some(value),
        ResultValue::Integer(_) | ResultValue::Boolean(_) | ResultValue::Array(_) => None,
    }
}

fn number(value: &ResultValue) -> Option<u64> {
    match value {
        ResultValue::Integer(value) => Some(*value),
        ResultValue::Text(_) | ResultValue::Boolean(_) | ResultValue::Array(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{abbreviate, field_value_width};

    #[test]
    fn narrow_values_are_ellipsized_to_the_available_width() {
        let rendered = abbreviate("weekend-builders", 8);
        assert_eq!(rendered, "weekend…");
        assert_eq!(rendered.chars().count(), 8);
    }

    #[test]
    fn narrow_fields_fit_the_terminal_width() {
        let width = 40;
        let label = abbreviate("destination relays", width - 6);
        let value = abbreviate("ws://relay.example", field_value_width(&label, width));
        assert!(3 + label.chars().count() + 2 + value.chars().count() <= width);
    }
}
