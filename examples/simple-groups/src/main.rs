//! Interactive and scripted real-relay simple-group E2E application.

mod support;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, IsTerminal as _, Write as _, stdin, stdout};
use std::path::PathBuf;

use e2e_support::{
    Account, CommandResult, E2eSession, InputMode, Limits, OutputFormat, ShellError,
};
use fava::{EventBuilder, Fava, Kind};
use fava_simple_groups::{SimpleGroup, SimpleGroupEventBuilder, create_group, delete_group};
use nostr::key::Keys;

use support::{OPERATION_TIMEOUT, assemble, settle_acknowledged};

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    let options = Options::parse()?;
    let alice = Keys::generate();
    let bob = Keys::generate();
    let fava = assemble(&alice, &bob)?;
    let accounts = [
        Account::new("alice", alice.public_key()),
        Account::new("bob", bob.public_key()),
    ];
    let mut session = E2eSession::new(Limits::standard(), accounts)?;
    let mut groups = BTreeMap::new();
    let mut selected_group = None;
    let mut commands = GroupCommands {
        groups: &mut groups,
        selected_group: &mut selected_group,
        fava: &fava,
    };
    let mut output = stdout().lock();

    if let Some(path) = options.script {
        let mut input = BufReader::new(File::open(path)?);
        run(
            &mut session,
            &mut commands,
            &mut input,
            &mut output,
            InputMode::Script,
            options.format,
        )?;
    } else {
        let input_mode = if stdin().is_terminal() {
            InputMode::Interactive
        } else {
            InputMode::Script
        };
        let mut input = BufReader::new(stdin().lock());
        run(
            &mut session,
            &mut commands,
            &mut input,
            &mut output,
            input_mode,
            options.format,
        )?;
    }
    Ok(())
}

fn run<R: std::io::BufRead, W: std::io::Write>(
    session: &mut E2eSession,
    commands: &mut GroupCommands<'_>,
    input: &mut R,
    output: &mut W,
    mode: InputMode,
    format: OutputFormat,
) -> AppResult<()> {
    session.run(
        input,
        output,
        mode,
        format,
        |session, words, input, output, mode| commands.execute(session, words, input, output, mode),
    )?;
    Ok(())
}

struct GroupCommands<'a> {
    groups: &'a mut BTreeMap<String, SimpleGroup>,
    selected_group: &'a mut Option<String>,
    fava: &'a Fava,
}

impl GroupCommands<'_> {
    fn execute<R, W>(
        &mut self,
        session: &mut E2eSession,
        words: &[String],
        input: &mut R,
        output: &mut W,
        mode: InputMode,
    ) -> Result<CommandResult, ShellError>
    where
        R: std::io::BufRead,
        W: std::io::Write,
    {
        match words {
            [group, action, id, relay_alias] if group == "group" && action == "create" => {
                let author = session.selected_account()?.public_key();
                let relay = session.relay(relay_alias)?.clone();
                let simple_group = SimpleGroup::new(id, vec![relay.clone()])
                    .map_err(|error| ShellError::Domain(error.to_string()))?;
                let event = create_group(author, &simple_group)
                    .map_err(|error| ShellError::Domain(error.to_string()))?;
                let write = self
                    .fava
                    .publish(event)
                    .map_err(|error| ShellError::Domain(error.to_string()))?;
                let receipt = settle_acknowledged(&write).map_err(ShellError::Domain)?;
                self.groups.insert(id.clone(), simple_group);
                *self.selected_group = Some(id.clone());
                CommandResult::success(
                    "group-created",
                    format!("created and selected {id} on {relay}"),
                )
                .with_field("group", id)
                .and_then(|result| result.with_field("author", author.to_hex()))
                .and_then(|result| result.with_field("event_id", receipt.current.id().to_hex()))
                .and_then(|result| {
                    result.with_field("write_id", write.write_id().as_u64().to_string())
                })
            }
            [group, action, id] if group == "group" && action == "use" => {
                if !self.groups.contains_key(id) {
                    return Err(ShellError::UnknownCommand {
                        command: format!("unknown group {id:?}"),
                    });
                }
                *self.selected_group = Some(id.clone());
                CommandResult::success("group-selected", format!("selected {id}"))
                    .with_field("group", id)
            }
            [group, event, publish, rest @ ..]
                if group == "group" && event == "event" && publish == "publish" =>
            {
                self.publish_command(session, rest, input, output, mode)
            }
            [group, action, id] if group == "group" && action == "delete" => {
                let author = session.selected_account()?.public_key();
                let simple_group =
                    self.groups
                        .get(id)
                        .ok_or_else(|| ShellError::UnknownCommand {
                            command: format!("unknown group {id:?}"),
                        })?;
                let event = delete_group(author, simple_group)
                    .map_err(|error| ShellError::Domain(error.to_string()))?;
                let write = self
                    .fava
                    .publish(event)
                    .map_err(|error| ShellError::Domain(error.to_string()))?;
                let receipt = settle_acknowledged(&write).map_err(ShellError::Domain)?;
                self.groups.remove(id);
                if self.selected_group.as_deref() == Some(id) {
                    *self.selected_group = None;
                }
                CommandResult::success("group-deleted", format!("deleted {id}"))
                    .with_field("group", id)
                    .and_then(|result| result.with_field("author", author.to_hex()))
                    .and_then(|result| result.with_field("event_id", receipt.current.id().to_hex()))
                    .and_then(|result| {
                        result.with_field("write_id", write.write_id().as_u64().to_string())
                    })
            }
            [group, action] if group == "group" && action == "list" => {
                CommandResult::success("group-list", "known simple groups")
                    .with_field(
                        "groups",
                        self.groups.keys().cloned().collect::<Vec<_>>().join(","),
                    )?
                    .with_field(
                        "selected_group",
                        self.selected_group.as_deref().unwrap_or(""),
                    )
            }
            _ => Err(ShellError::UnknownCommand {
                command: words.join(" "),
            }),
        }
    }

    fn publish_command<R, W>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        input: &mut R,
        output: &mut W,
        mode: InputMode,
    ) -> Result<CommandResult, ShellError>
    where
        R: std::io::BufRead,
        W: std::io::Write,
    {
        match arguments {
            [] => {
                let kind = prompted_kind(session, input, output, mode)?;
                self.publish_group_event(session, &kind, None)
            }
            [flag] if flag == "--kind" => {
                let kind = prompted_kind(session, input, output, mode)?;
                self.publish_group_event(session, &kind, None)
            }
            [content] => {
                let kind = prompted_kind(session, input, output, mode)?;
                self.publish_group_event(session, &kind, Some(content))
            }
            [flag, kind] if flag == "--kind" => self.publish_group_event(session, kind, None),
            [flag, kind, content] if flag == "--kind" => {
                self.publish_group_event(session, kind, Some(content))
            }
            _ => Err(ShellError::Usage {
                usage: EVENT_PUBLISH_USAGE,
            }),
        }
    }

    fn publish_group_event(
        &self,
        session: &E2eSession,
        kind: &str,
        content: Option<&str>,
    ) -> Result<CommandResult, ShellError> {
        let kind = parse_event_kind(kind)?;
        let id = self
            .selected_group
            .as_deref()
            .ok_or_else(|| ShellError::Domain("no simple group is selected".to_owned()))?;
        let simple_group = self
            .groups
            .get(id)
            .ok_or_else(|| ShellError::UnknownCommand {
                command: format!("unknown group {id:?}"),
            })?;
        let author = session.selected_account()?.public_key();
        let event = match content {
            Some(content) => EventBuilder::new(author, kind).content(content),
            None => EventBuilder::new(author, kind),
        }
        .simple_group(simple_group)
        .map_err(|error| ShellError::Domain(error.to_string()))?;
        let write = self
            .fava
            .publish(event)
            .map_err(|error| ShellError::Domain(error.to_string()))?;
        let receipt = settle_acknowledged(&write).map_err(ShellError::Domain)?;
        CommandResult::success(
            "group-event-published",
            format!("published kind {} to {id}", kind.as_u16()),
        )
        .with_field("group", id)
        .and_then(|result| result.with_field("kind", kind.as_u16().to_string()))
        .and_then(|result| result.with_field("author", author.to_hex()))
        .and_then(|result| result.with_field("event_id", receipt.current.id().to_hex()))
        .and_then(|result| result.with_field("write_id", write.write_id().as_u64().to_string()))
    }
}

const EVENT_PUBLISH_USAGE: &str = "group event publish --kind <kind> [content]";

fn parse_event_kind(kind: &str) -> Result<Kind, ShellError> {
    kind.parse()
        .map(Kind::from_u16)
        .map_err(|_| ShellError::Usage {
            usage: EVENT_PUBLISH_USAGE,
        })
}

fn prompted_kind<R, W>(
    session: &E2eSession,
    input: &mut R,
    output: &mut W,
    mode: InputMode,
) -> Result<String, ShellError>
where
    R: std::io::BufRead,
    W: std::io::Write,
{
    session
        .prompt_value(input, output, mode, "kind")?
        .ok_or(ShellError::Usage {
            usage: EVENT_PUBLISH_USAGE,
        })
}

struct Options {
    script: Option<PathBuf>,
    format: OutputFormat,
}

impl Options {
    fn parse() -> AppResult<Self> {
        let mut script = None;
        let mut format = OutputFormat::Human;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--script" => {
                    script = Some(PathBuf::from(
                        arguments.next().ok_or("--script requires a command file")?,
                    ));
                }
                "--jsonl" => format = OutputFormat::JsonLines,
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => return Err(format!("unknown option {argument:?}").into()),
            }
        }
        Ok(Self { script, format })
    }
}

fn print_help() {
    let _ = writeln!(
        stdout(),
        "Usage: simple-groups [--script commands.txt] [--jsonl]\n\nShared commands:\n  relay add <alias> <ws-url>\n  account use <alice|bob>\n  capture <alias> <result-field>\n  dump\n  quit\n\nSimple-group commands:\n  group create <id> <relay-alias>\n  group use <id>\n  group event publish --kind <kind> [content]\n  group delete <id>\n  group list\n\nInteractive kind omission prompts; scripts render the refusal and fail. Every publication requires acknowledgement within {OPERATION_TIMEOUT:?}. Scripts use ordinary stdin/file lines; no PTY is required."
    );
}

#[cfg(test)]
mod tests {
    use super::parse_event_kind;

    #[test]
    fn event_publish_accepts_the_full_u16_kind_space() {
        for kind in [0, 1, 12_345, u16::MAX] {
            assert_eq!(parse_event_kind(&kind.to_string()).unwrap().as_u16(), kind);
        }
    }
}
