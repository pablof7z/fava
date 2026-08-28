//! NIP-29 metadata edit option parsing.

use e2e_support::ShellError;
use fava::Kind;
use fava_simple_groups::{GroupAccess, GroupVisibility, MetadataEdit};

use crate::app::{required_value, usage};

const GROUP_EDIT_USAGE: &str = "group edit [--name <text>] [--about <text>] [--picture <url>] [--private|--public] [--closed|--open] [--supported-kinds <kind> ...]";

pub(crate) fn metadata_edit<P>(
    arguments: &[String],
    prompt: &mut P,
) -> Result<MetadataEdit, ShellError>
where
    P: FnMut(&str) -> Result<Option<String>, ShellError>,
{
    let mut edit = MetadataEdit::default();
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        match argument.as_str() {
            "--name" => {
                edit.name = Some(required_value(
                    arguments,
                    index + 1,
                    "name",
                    GROUP_EDIT_USAGE,
                    prompt,
                )?);
                index += 2;
            }
            "--about" => {
                edit.about = Some(required_value(
                    arguments,
                    index + 1,
                    "about",
                    GROUP_EDIT_USAGE,
                    prompt,
                )?);
                index += 2;
            }
            "--picture" => {
                edit.picture = Some(required_value(
                    arguments,
                    index + 1,
                    "picture",
                    GROUP_EDIT_USAGE,
                    prompt,
                )?);
                index += 2;
            }
            "--private" => {
                edit.visibility = Some(GroupVisibility::Private);
                index += 1;
            }
            "--public" => {
                edit.visibility = Some(GroupVisibility::Public);
                index += 1;
            }
            "--closed" => {
                edit.access = Some(GroupAccess::Closed);
                index += 1;
            }
            "--open" => {
                edit.access = Some(GroupAccess::Open);
                index += 1;
            }
            "--supported-kinds" => {
                let kinds = edit.supported_kinds.get_or_insert_with(Vec::new);
                index += 1;
                while let Some(value) = arguments.get(index) {
                    if value.starts_with("--") {
                        break;
                    }
                    let kind = value.parse::<u16>().map_err(|_| ShellError::Usage {
                        usage: GROUP_EDIT_USAGE,
                    })?;
                    kinds.push(Kind::from_u16(kind));
                    index += 1;
                }
            }
            _ => return usage(GROUP_EDIT_USAGE),
        }
    }
    Ok(edit)
}
