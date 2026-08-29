//! Static completion catalog for the fixed simple-groups grammar.

#[derive(Clone, Copy)]
pub(crate) struct Completion {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
}

pub(crate) fn suggestions(context: &str) -> &'static [Completion] {
    match context {
        "" => ROOT,
        "account" => ACCOUNT,
        "relay" => RELAY,
        "group" => GROUP,
        "group event" => GROUP_EVENT,
        "group member" => MEMBER,
        "group edit" => EDIT_OPTIONS,
        "receipt" => RECEIPT,
        "saved-list" => SAVED_LIST,
        "saved-list group" => SAVED_GROUP,
        "saved-list relay" => SAVED_RELAY,
        "group event publish" | "group event expect-rejection" => EVENT_OPTIONS,
        _ => &[],
    }
}

pub(crate) fn is_command(word: &str) -> bool {
    ROOT.iter()
        .chain(GROUP)
        .chain(ACCOUNT)
        .chain(RELAY)
        .chain(RECEIPT)
        .any(|entry| entry.name == word)
}

const fn item(name: &'static str, description: &'static str) -> Completion {
    Completion { name, description }
}

const ROOT: &[Completion] = &[
    item("account", "local signer selection"),
    item("relay", "public relay aliases"),
    item("group", "NIP-29 group workflow"),
    item("saved-list", "kind-10009 saved lists"),
    item("status", "current app selection"),
    item("routes", "read route preview"),
    item("receipt", "publication obligations"),
    item("diagnostics", "bounded Fava counts"),
    item("capture", "capture the last scalar field"),
    item("dump", "bounded shell state"),
    item("quit", "close this session"),
];

const ACCOUNT: &[Completion] = &[
    item("new", "create and select a local account"),
    item("import", "import a local signer"),
    item("list", "list local accounts"),
    item("switch", "select an account"),
    item("remove", "remove an attached signer"),
];

const RELAY: &[Completion] = &[
    item("add", "add a public ws relay alias"),
    item("list", "list relay aliases"),
    item("remove", "remove one relay alias"),
];

const GROUP: &[Completion] = &[
    item("create", "create and select a group"),
    item("open", "open and select a known group"),
    item("list", "list known groups"),
    item("switch", "select a known group"),
    item("edit", "publish metadata and policy"),
    item("invite", "publish an invite"),
    item("join", "publish a join request"),
    item("member", "add or remove members"),
    item("leave", "publish a leave request"),
    item("delete", "delete a selected group"),
    item("event", "publish, reject-check, or delete"),
    item("events", "read bounded group events"),
    item("state", "decode bounded relay state"),
];

const GROUP_EVENT: &[Completion] = &[
    item("publish", "publish an explicit kind"),
    item("expect-rejection", "assert all relays reject"),
    item("delete", "publish an event deletion"),
];

const MEMBER: &[Completion] = &[
    item("add", "add one public key"),
    item("remove", "remove one public key"),
];

const EDIT_OPTIONS: &[Completion] = &[
    item("--name", "metadata name"),
    item("--about", "metadata description"),
    item("--picture", "metadata picture URL"),
    item("--private", "private visibility"),
    item("--public", "public visibility"),
    item("--closed", "closed membership"),
    item("--open", "open membership"),
    item("--supported-kinds", "ordered supported kinds"),
];

const EVENT_OPTIONS: &[Completion] = &[item("--kind", "u16 Nostr kind")];

const RECEIPT: &[Completion] = &[
    item("list", "list open publication obligations"),
    item("show", "show one retained receipt by id"),
];

const SAVED_LIST: &[Completion] = &[
    item("show", "read a bounded saved list"),
    item("group", "save or remove a group"),
    item("relay", "save or remove a relay"),
];

const SAVED_GROUP: &[Completion] = &[
    item("add", "save the selected group"),
    item("rename", "set saved display name"),
    item("remove", "remove the selected group"),
];

const SAVED_RELAY: &[Completion] = &[
    item("add", "save a relay in a list"),
    item("remove", "remove a relay from a list"),
];
