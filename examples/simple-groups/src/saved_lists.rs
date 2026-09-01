//! Kind-10009 saved-list grammar and explicit-relay edit publication.

use e2e_support::{CommandResult, E2eSession, ShellError};
use fava_simple_groups::{
    SavedGroupList, remove_saved_relay, remove_saved_simple_group, rename_saved_simple_group,
    save_relay, save_simple_group, saved_group_lists,
};

use crate::app::{App, domain_error, parse_read_limit, required_value, usage};

const SAVED_SHOW_USAGE: &str = "saved-list show <relay-alias> [limit]";
const SAVED_GROUP_ADD_USAGE: &str = "saved-list group add <relay-alias> [display-name]";
const SAVED_GROUP_RENAME_USAGE: &str = "saved-list group rename <relay-alias> <display-name>";
const SAVED_GROUP_REMOVE_USAGE: &str = "saved-list group remove <relay-alias>";
const SAVED_RELAY_ADD_USAGE: &str =
    "saved-list relay add <publication-relay-alias> <saved-relay-alias>";
const SAVED_RELAY_REMOVE_USAGE: &str =
    "saved-list relay remove <publication-relay-alias> <saved-relay-alias>";

impl App {
    pub(crate) fn saved_list_command<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        match arguments {
            [action, rest @ ..] if action == "show" => self.show_saved_list(session, rest, prompt),
            [group, action, rest @ ..] if group == "group" && action == "add" => {
                self.save_group(session, rest, prompt)
            }
            [group, action, rest @ ..] if group == "group" && action == "rename" => {
                self.rename_saved_group(session, rest, prompt)
            }
            [group, action, rest @ ..] if group == "group" && action == "remove" => {
                self.remove_saved_group(session, rest, prompt)
            }
            [relay, action, rest @ ..] if relay == "relay" && action == "add" => {
                self.save_list_relay(session, rest, prompt)
            }
            [relay, action, rest @ ..] if relay == "relay" && action == "remove" => {
                self.remove_list_relay(session, rest, prompt)
            }
            _ => usage("saved-list <show|group|relay> ..."),
        }
    }

    fn show_saved_list<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let alias = required_value(arguments, 0, "relay-alias", SAVED_SHOW_USAGE, prompt)?;
        if arguments.len() > 2 {
            return usage(SAVED_SHOW_USAGE);
        }
        let limit = parse_read_limit(arguments.get(1), SAVED_SHOW_USAGE)?;
        let relay = session.relay(&alias)?.clone();
        let author = session.selected_account()?.public_key();
        let query = saved_group_lists([author])
            .map_err(domain_error)?
            .only_from_relays([relay])
            .map_err(domain_error)?
            .limit(limit)
            .map_err(domain_error)?;
        let snapshot = self.read_limited_eose(query)?;
        let mut groups = 0;
        let mut relays = 0;
        let mut failures = 0;
        for event in snapshot.events.iter() {
            match SavedGroupList::from_event(event.event()) {
                Ok(list) => {
                    groups += list
                        .simple_groups()
                        .iter()
                        .filter(|entry| entry.is_ok())
                        .count();
                    relays += list.relays().iter().filter(|entry| entry.is_ok()).count();
                    failures += list
                        .simple_groups()
                        .iter()
                        .filter(|entry| entry.is_err())
                        .count();
                    failures += list.relays().iter().filter(|entry| entry.is_err()).count();
                }
                Err(_) => failures += 1,
            }
        }
        CommandResult::success("saved-list", "bounded decoded saved group list snapshot")
            .with_field("relay", alias)?
            .with_field("events", snapshot.events.len())?
            .with_field("groups", groups)?
            .with_field("saved_relays", relays)?
            .with_field("decode_failures", failures)?
            .with_field(
                "event_id",
                snapshot
                    .events
                    .first()
                    .map(|event| event.id().to_hex())
                    .unwrap_or_default(),
            )?
            .with_field("relay_eose", true)?
            .with_field("stored_events_complete", false)
    }

    fn save_group<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let relay_alias =
            required_value(arguments, 0, "relay-alias", SAVED_GROUP_ADD_USAGE, prompt)?;
        if arguments.len() > 2 {
            return usage(SAVED_GROUP_ADD_USAGE);
        }
        let group = self.selected_group()?;
        let edit =
            save_simple_group(group, arguments.get(1).map(String::as_str)).map_err(domain_error)?;
        self.publish_saved_edit(session, &relay_alias, edit, "saved-group-added", group.id())
    }

    fn rename_saved_group<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let relay_alias = required_value(
            arguments,
            0,
            "relay-alias",
            SAVED_GROUP_RENAME_USAGE,
            prompt,
        )?;
        let display_name = required_value(
            arguments,
            1,
            "display-name",
            SAVED_GROUP_RENAME_USAGE,
            prompt,
        )?;
        if arguments.len() > 2 {
            return usage(SAVED_GROUP_RENAME_USAGE);
        }
        let group = self.selected_group()?;
        let edit = rename_saved_simple_group(group, &display_name).map_err(domain_error)?;
        self.publish_saved_edit(
            session,
            &relay_alias,
            edit,
            "saved-group-renamed",
            group.id(),
        )
    }

    fn remove_saved_group<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let relay_alias = required_value(
            arguments,
            0,
            "relay-alias",
            SAVED_GROUP_REMOVE_USAGE,
            prompt,
        )?;
        if arguments.len() > 1 {
            return usage(SAVED_GROUP_REMOVE_USAGE);
        }
        let group = self.selected_group()?;
        let edit = remove_saved_simple_group(group).map_err(domain_error)?;
        self.publish_saved_edit(
            session,
            &relay_alias,
            edit,
            "saved-group-removed",
            group.id(),
        )
    }

    fn save_list_relay<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let publication_alias = required_value(
            arguments,
            0,
            "publication-relay-alias",
            SAVED_RELAY_ADD_USAGE,
            prompt,
        )?;
        let saved_alias = required_value(
            arguments,
            1,
            "saved-relay-alias",
            SAVED_RELAY_ADD_USAGE,
            prompt,
        )?;
        if arguments.len() > 2 {
            return usage(SAVED_RELAY_ADD_USAGE);
        }
        let edit = save_relay(session.relay(&saved_alias)?.clone()).map_err(domain_error)?;
        self.publish_saved_edit(session, &publication_alias, edit, "saved-relay-added", "")
    }

    fn remove_list_relay<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let publication_alias = required_value(
            arguments,
            0,
            "publication-relay-alias",
            SAVED_RELAY_REMOVE_USAGE,
            prompt,
        )?;
        let saved_alias = required_value(
            arguments,
            1,
            "saved-relay-alias",
            SAVED_RELAY_REMOVE_USAGE,
            prompt,
        )?;
        if arguments.len() > 2 {
            return usage(SAVED_RELAY_REMOVE_USAGE);
        }
        let edit =
            remove_saved_relay(session.relay(&saved_alias)?.clone()).map_err(domain_error)?;
        self.publish_saved_edit(session, &publication_alias, edit, "saved-relay-removed", "")
    }

    fn publish_saved_edit(
        &self,
        session: &E2eSession,
        relay_alias: &str,
        edit: fava::EventEdit,
        kind: &'static str,
        group: &str,
    ) -> Result<CommandResult, ShellError> {
        let relay = session.relay(relay_alias)?.clone();
        let author = session.selected_account()?.public_key();
        let write = self
            .fava
            .to(vec![relay])
            .map_err(domain_error)?
            .with_account(author)
            .publish(edit)
            .map_err(domain_error)?;
        Self::publication_result(
            kind,
            format!("published {kind} through {relay_alias}"),
            &write,
            (!group.is_empty()).then_some(group),
            None,
            Some(relay_alias),
        )
    }
}
