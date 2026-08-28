//! Explicit-kind group event publication and deletion grammar.

use std::io::BufRead;

use e2e_support::{CommandResult, E2eSession, InputMode, ShellError};
use fava::{EventBuilder, Kind};
use fava_simple_groups::{SimpleGroupEventBuilder, delete_event};
use nostr::event::EventId;

use crate::app::{App, domain_error, required_value, usage};

const EVENT_PUBLISH_USAGE: &str = "group event publish --kind <kind> [content]";
const EVENT_EXPECT_REJECTION_USAGE: &str = "group event expect-rejection --kind <kind> [content]";
const EVENT_DELETE_USAGE: &str = "group event delete <event-id>";

impl App {
    pub(crate) fn event_command<R, W>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        input: &mut R,
        output: &mut W,
        mode: InputMode,
    ) -> Result<CommandResult, ShellError>
    where
        R: BufRead,
        W: std::io::Write,
    {
        match arguments {
            [action, rest @ ..] if action == "publish" => {
                self.publish_group_event(session, rest, input, output, mode, false)
            }
            [action, rest @ ..] if action == "expect-rejection" => {
                self.publish_group_event(session, rest, input, output, mode, true)
            }
            [action, rest @ ..] if action == "delete" => {
                self.delete_group_event(session, rest, input, output, mode)
            }
            _ => usage("group event <publish|expect-rejection|delete> ..."),
        }
    }

    fn publish_group_event<R, W>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        input: &mut R,
        output: &mut W,
        mode: InputMode,
        expect_rejection: bool,
    ) -> Result<CommandResult, ShellError>
    where
        R: BufRead,
        W: std::io::Write,
    {
        let usage_text = if expect_rejection {
            EVENT_EXPECT_REJECTION_USAGE
        } else {
            EVENT_PUBLISH_USAGE
        };
        let (kind, content) = match arguments {
            [] => (
                session
                    .prompt_value(input, output, mode, "kind")?
                    .ok_or(ShellError::Usage { usage: usage_text })?,
                None,
            ),
            [flag] if flag == "--kind" => (
                session
                    .prompt_value(input, output, mode, "kind")?
                    .ok_or(ShellError::Usage { usage: usage_text })?,
                None,
            ),
            [flag, kind] if flag == "--kind" => (kind.clone(), None),
            [flag, kind, content] if flag == "--kind" => (kind.clone(), Some(content.as_str())),
            _ => return usage(usage_text),
        };
        let kind = kind
            .parse::<u16>()
            .map(Kind::from_u16)
            .map_err(|_| ShellError::Usage { usage: usage_text })?;
        if let Some(content) = content {
            session.validate_result_value(content)?;
        }
        let group = self.selected_group()?.clone();
        let builder = EventBuilder::new(session.selected_account()?.public_key(), kind);
        let builder = match content {
            Some(content) => builder.content(content),
            None => builder,
        };
        let builder = builder.simple_group(&group).map_err(domain_error)?;
        let write = self.fava.publish(builder).map_err(domain_error)?;
        if expect_rejection {
            Self::expected_rejection_result(
                "group-event-rejected",
                format!(
                    "observed expected rejection of kind {} to {}",
                    kind.as_u16(),
                    group.id()
                ),
                &write,
                Some(group.id()),
                Some(content.unwrap_or("")),
            )
        } else {
            Self::publication_result(
                "group-event-published",
                format!("published kind {} to {}", kind.as_u16(), group.id()),
                &write,
                Some(group.id()),
                Some(content.unwrap_or("")),
                None,
            )
        }
    }

    fn delete_group_event<R, W>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        input: &mut R,
        output: &mut W,
        mode: InputMode,
    ) -> Result<CommandResult, ShellError>
    where
        R: BufRead,
        W: std::io::Write,
    {
        let event_id = required_value(
            session,
            arguments,
            0,
            "event-id",
            EVENT_DELETE_USAGE,
            input,
            output,
            mode,
        )?;
        if arguments.len() > 1 {
            return usage(EVENT_DELETE_USAGE);
        }
        let event_id = EventId::parse(&event_id).map_err(|_| ShellError::Usage {
            usage: EVENT_DELETE_USAGE,
        })?;
        let group = self.selected_group()?.clone();
        let builder = delete_event(session.selected_account()?.public_key(), &group, &event_id)
            .map_err(domain_error)?;
        let write = self.fava.publish(builder).map_err(domain_error)?;
        Self::publication_result(
            "group-event-deleted",
            format!("published event deletion for {}", group.id()),
            &write,
            Some(group.id()),
            None,
            None,
        )
    }
}
