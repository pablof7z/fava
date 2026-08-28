//! NIP-29 grammar and self-routed management/publication workflows.

use e2e_support::{CommandResult, E2eSession, ResultValue, ShellError, parse_public_key};
use fava_simple_groups::{
    SimpleGroup, create_group, delete_group, edit_metadata, invite, join_request, leave_group,
    put_user, remove_user,
};

use crate::app::{App, domain_error, required_value, usage};
use crate::group_metadata::metadata_edit;

const GROUP_CREATE_USAGE: &str = "group create <id> <relay-alias> [relay-alias ...]";
const GROUP_OPEN_USAGE: &str = "group open <id> <relay-alias> [relay-alias ...]";
const GROUP_SWITCH_USAGE: &str = "group switch <id>";
const GROUP_INVITE_USAGE: &str = "group invite <code>";
const GROUP_JOIN_USAGE: &str = "group join [code] [reason]";
const MEMBER_ADD_USAGE: &str = "group member add <public-key> [role ...]";
const MEMBER_REMOVE_USAGE: &str = "group member remove <public-key>";

impl App {
    pub(crate) fn group_command<P>(
        &mut self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        match arguments {
            [action, rest @ ..] if action == "create" => {
                self.create_group_command(session, rest, prompt)
            }
            [action, rest @ ..] if action == "open" => {
                self.open_group_command(session, rest, prompt)
            }
            [action] if action == "list" => self.list_groups(),
            [action, rest @ ..] if action == "switch" => self.switch_group_command(rest, prompt),
            [action, rest @ ..] if action == "edit" => {
                self.edit_group_command(session, rest, prompt)
            }
            [action, rest @ ..] if action == "invite" => self.invite_command(session, rest, prompt),
            [action, rest @ ..] if action == "join" => self.join_command(session, rest),
            [action, rest @ ..] if action == "member" => self.member_command(session, rest, prompt),
            [action] if action == "leave" => self.leave_command(session),
            [action, rest @ ..] if action == "delete" => self.delete_group_command(session, rest),
            [action, rest @ ..] if action == "event" => self.event_command(session, rest, prompt),
            [action, rest @ ..] if action == "events" => self.events_command(session, rest),
            [action, rest @ ..] if action == "state" => self.state_command(session, rest),
            _ => usage(
                "group <create|open|list|switch|edit|invite|join|member|leave|delete|event|events|state> ...",
            ),
        }
    }

    fn create_group_command<P>(
        &mut self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let (id, group) =
            group_from_arguments(session, arguments, "group-id", GROUP_CREATE_USAGE, prompt)?;
        if self.groups.contains_key(&id) {
            return Err(ShellError::Domain(format!(
                "simple group {id:?} is already known"
            )));
        }
        self.reserve_group(&id)?;
        let author = session.selected_account()?.public_key();
        let builder = create_group(author, &group).map_err(domain_error)?;
        let write = self.fava.publish(builder).map_err(domain_error)?;
        let result = Self::publication_result(
            "group-created",
            format!("created and selected {id}"),
            &write,
            Some(&id),
            None,
            None,
        )?;
        self.groups.insert(id.clone(), group);
        self.selected_group = Some(id);
        Ok(result)
    }

    fn open_group_command<P>(
        &mut self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let (id, group) =
            group_from_arguments(session, arguments, "group-id", GROUP_OPEN_USAGE, prompt)?;
        if let Some(existing) = self.groups.get(&id) {
            if existing != &group {
                return Err(ShellError::Domain(format!(
                    "simple group {id:?} is already open with a different relay set"
                )));
            }
        } else {
            self.reserve_group(&id)?;
            self.groups.insert(id.clone(), group);
        }
        self.selected_group = Some(id.clone());
        CommandResult::success("group-opened", format!("opened and selected {id}"))
            .with_field("group", id)
    }

    fn list_groups(&self) -> Result<CommandResult, ShellError> {
        CommandResult::success("group-list", "known simple groups")
            .with_field(
                "groups",
                ResultValue::array(self.groups.keys().cloned().map(ResultValue::text)),
            )?
            .with_field(
                "selected_group",
                self.selected_group.as_deref().unwrap_or(""),
            )
    }

    fn switch_group_command<P>(
        &mut self,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let id = required_value(arguments, 0, "group-id", GROUP_SWITCH_USAGE, prompt)?;
        if arguments.len() > 1 {
            return usage(GROUP_SWITCH_USAGE);
        }
        if !self.groups.contains_key(&id) {
            return Err(ShellError::Domain(format!("unknown simple group {id:?}")));
        }
        self.selected_group = Some(id.clone());
        CommandResult::success("group-selected", format!("selected {id}")).with_field("group", id)
    }

    fn edit_group_command<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let group = self.selected_group()?.clone();
        let edit = metadata_edit(arguments, prompt)?;
        let builder = edit_metadata(session.selected_account()?.public_key(), &group, &edit)
            .map_err(domain_error)?;
        let write = self.fava.publish(builder).map_err(domain_error)?;
        Self::publication_result(
            "group-edited",
            format!("published metadata edit for {}", group.id()),
            &write,
            Some(group.id()),
            None,
            None,
        )
    }

    fn invite_command<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        let code = required_value(arguments, 0, "invite-code", GROUP_INVITE_USAGE, prompt)?;
        if arguments.len() > 1 {
            return usage(GROUP_INVITE_USAGE);
        }
        let group = self.selected_group()?.clone();
        let builder = invite(session.selected_account()?.public_key(), &group, &code)
            .map_err(domain_error)?;
        let write = self.fava.publish(builder).map_err(domain_error)?;
        Self::publication_result(
            "group-invited",
            format!("published invite for {}", group.id()),
            &write,
            Some(group.id()),
            None,
            None,
        )
    }

    fn join_command(
        &self,
        session: &E2eSession,
        arguments: &[String],
    ) -> Result<CommandResult, ShellError> {
        if arguments.len() > 2 {
            return usage(GROUP_JOIN_USAGE);
        }
        if let Some(reason) = arguments.get(1) {
            session.validate_result_value(reason)?;
        }
        let group = self.selected_group()?.clone();
        let builder = join_request(
            session.selected_account()?.public_key(),
            &group,
            arguments.first().map(String::as_str),
        )
        .map_err(domain_error)?;
        let builder = match arguments.get(1) {
            Some(reason) => builder.content(reason),
            None => builder,
        };
        let write = self.fava.publish(builder).map_err(domain_error)?;
        Self::publication_result(
            "group-join-requested",
            format!("published join request for {}", group.id()),
            &write,
            Some(group.id()),
            arguments.get(1).map(String::as_str),
            None,
        )
    }

    fn member_command<P>(
        &self,
        session: &E2eSession,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        match arguments {
            [action, rest @ ..] if action == "add" => {
                let public_key = required_value(rest, 0, "public-key", MEMBER_ADD_USAGE, prompt)?;
                let public_key = parse_public_key(&public_key)?;
                let roles = rest.iter().skip(1).map(String::as_str).collect::<Vec<_>>();
                let group = self.selected_group()?.clone();
                let builder = put_user(
                    session.selected_account()?.public_key(),
                    &group,
                    &[public_key],
                    &roles,
                )
                .map_err(domain_error)?;
                let write = self.fava.publish(builder).map_err(domain_error)?;
                Self::publication_result(
                    "group-member-added",
                    format!("published member addition for {}", group.id()),
                    &write,
                    Some(group.id()),
                    None,
                    None,
                )
            }
            [action, rest @ ..] if action == "remove" => {
                let public_key =
                    required_value(rest, 0, "public-key", MEMBER_REMOVE_USAGE, prompt)?;
                if rest.len() > 1 {
                    return usage(MEMBER_REMOVE_USAGE);
                }
                let public_key = parse_public_key(&public_key)?;
                let group = self.selected_group()?.clone();
                let builder = remove_user(
                    session.selected_account()?.public_key(),
                    &group,
                    &[public_key],
                )
                .map_err(domain_error)?;
                let write = self.fava.publish(builder).map_err(domain_error)?;
                Self::publication_result(
                    "group-member-removed",
                    format!("published member removal for {}", group.id()),
                    &write,
                    Some(group.id()),
                    None,
                    None,
                )
            }
            _ => usage("group member <add|remove> ..."),
        }
    }

    fn leave_command(&self, session: &E2eSession) -> Result<CommandResult, ShellError> {
        let group = self.selected_group()?.clone();
        let builder =
            leave_group(session.selected_account()?.public_key(), &group).map_err(domain_error)?;
        let write = self.fava.publish(builder).map_err(domain_error)?;
        Self::publication_result(
            "group-left",
            format!("published leave request for {}", group.id()),
            &write,
            Some(group.id()),
            None,
            None,
        )
    }

    fn delete_group_command(
        &mut self,
        session: &E2eSession,
        arguments: &[String],
    ) -> Result<CommandResult, ShellError> {
        let id = match arguments {
            [] => self
                .selected_group
                .clone()
                .ok_or_else(|| ShellError::Domain("no simple group is selected".to_owned()))?,
            [id] => id.clone(),
            _ => return usage("group delete [id]"),
        };
        let group = self
            .groups
            .get(&id)
            .cloned()
            .ok_or_else(|| ShellError::UnknownCommand {
                command: format!("unknown simple group {id:?}"),
            })?;
        let builder =
            delete_group(session.selected_account()?.public_key(), &group).map_err(domain_error)?;
        let write = self.fava.publish(builder).map_err(domain_error)?;
        let result = Self::publication_result(
            "group-deleted",
            format!("deleted {id}"),
            &write,
            Some(&id),
            None,
            None,
        )?;
        self.groups.remove(&id);
        if self.selected_group.as_deref() == Some(&id) {
            self.selected_group = None;
        }
        Ok(result)
    }
}

fn group_from_arguments<P>(
    session: &E2eSession,
    arguments: &[String],
    id_label: &str,
    usage_text: &'static str,
    prompt: &mut P,
) -> Result<(String, SimpleGroup), ShellError>
where
    P: FnMut(&str) -> Result<Option<String>, ShellError>,
{
    let id = required_value(arguments, 0, id_label, usage_text, prompt)?;
    let relay_aliases = if arguments.len() > 1 {
        arguments[1..].to_vec()
    } else {
        vec![required_value(
            arguments,
            1,
            "relay-alias",
            usage_text,
            prompt,
        )?]
    };
    let relays = relay_aliases
        .iter()
        .map(|alias| session.relay(alias).cloned())
        .collect::<Result<Vec<_>, _>>()?;
    let group = SimpleGroup::new(id.clone(), relays).map_err(domain_error)?;
    Ok((id, group))
}
