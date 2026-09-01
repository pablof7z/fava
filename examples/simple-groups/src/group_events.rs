//! Explicit-kind group event publication and deletion grammar.

use e2e_support::{CommandResult, E2eSession, ShellError};
use fava::{EventBuilder, Kind};
use fava_simple_groups::{SimpleGroupEventBuilder, delete_event};
use nostr::event::EventId;

use crate::app::{App, domain_error, required_value, usage};

const EVENT_PUBLISH_USAGE: &str = "group event publish --kind <kind> [content]";
const EVENT_EXPECT_REJECTION_USAGE: &str = "group event expect-rejection --kind <kind> [content]";
const EVENT_DELETE_USAGE: &str = "group event delete <event-id>";

impl App {
    pub(crate) fn event_command<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        match arguments {
            [action, rest @ ..] if action == "publish" => {
                self.publish_group_event(session, rest, prompt, false)
            }
            [action, rest @ ..] if action == "expect-rejection" => {
                self.publish_group_event(session, rest, prompt, true)
            }
            [action, rest @ ..] if action == "delete" => {
                self.delete_group_event(session, rest, prompt)
            }
            _ => usage("group event <publish|expect-rejection|delete> ..."),
        }
    }

    fn publish_group_event<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
        expect_rejection: bool,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let usage_text = if expect_rejection {
            EVENT_EXPECT_REJECTION_USAGE
        } else {
            EVENT_PUBLISH_USAGE
        };
        let (kind, content) = match arguments {
            [] => (
                prompt("kind")?.ok_or(ShellError::Usage { usage: usage_text })?,
                None,
            ),
            [flag] if flag == "--kind" => (
                prompt("kind")?.ok_or(ShellError::Usage { usage: usage_text })?,
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
        let author = session.selected_account()?.public_key();
        let builder = EventBuilder::new(kind);
        let builder = match content {
            Some(content) => builder.content(content),
            None => builder,
        };
        let builder = builder.simple_group(&group).map_err(domain_error)?;
        let write = self
            .fava
            .with_account(author)
            .publish(builder)
            .map_err(domain_error)?;
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

    fn delete_group_event<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let event_id = required_value(arguments, 0, "event-id", EVENT_DELETE_USAGE, prompt)?;
        if arguments.len() > 1 {
            return usage(EVENT_DELETE_USAGE);
        }
        let event_id = EventId::parse(&event_id).map_err(|_| ShellError::Usage {
            usage: EVENT_DELETE_USAGE,
        })?;
        let group = self.selected_group()?.clone();
        let author = session.selected_account()?.public_key();
        let builder = delete_event(&group, &event_id).map_err(domain_error)?;
        let write = self
            .fava
            .with_account(author)
            .publish(builder)
            .map_err(domain_error)?;
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
