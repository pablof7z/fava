//! Executable ownership, dependency, subtraction, and line-bound gates.

use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const PUBLIC_ROOT: &str = include_str!("../src/lib.rs");
const README: &str = include_str!("../README.md");
const CATALOG: &str = include_str!("../../../.bg-shell/simple-groups-semantic-catalog.jsonl");
const CONSTRUCTOR_DECISION: &str =
    include_str!("../../../docs/issues/0027-simple-group-relay-input-boundary.md");
const VOCABULARY: &str = include_str!("../../../docs/internals/vocabulary.toml");

fn sources() -> [(&'static str, &'static str); 9] {
    [
        ("edit.rs", include_str!("../src/edit.rs")),
        ("lib.rs", include_str!("../src/lib.rs")),
        ("metadata.rs", include_str!("../src/metadata.rs")),
        ("people.rs", include_str!("../src/people.rs")),
        ("pins.rs", include_str!("../src/pins.rs")),
        ("query.rs", include_str!("../src/query.rs")),
        ("records.rs", include_str!("../src/records.rs")),
        ("saved.rs", include_str!("../src/saved.rs")),
        ("simple_group.rs", include_str!("../src/simple_group.rs")),
    ]
}

fn dependencies() -> BTreeSet<String> {
    let mut active = false;
    let mut dependencies = BTreeSet::new();
    for line in MANIFEST.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.starts_with('[') {
            active = line == "[dependencies]";
        } else if active && !line.is_empty() {
            let key = line
                .split_once('=')
                .expect("dependency assignment")
                .0
                .trim();
            dependencies.insert(key.strip_suffix(".workspace").unwrap_or(key).to_owned());
        }
    }
    dependencies
}

#[test]
fn normal_dependencies_are_exact_domain_and_composition_owners() {
    assert_eq!(
        dependencies(),
        BTreeSet::from([
            "fava-query".to_owned(),
            "fava-state".to_owned(),
            "fava-write".to_owned(),
            "nostr".to_owned(),
        ])
    );
}

#[test]
fn public_root_exports_only_the_current_nominal_surface() {
    for required in [
        "SimpleGroup",
        "SimpleGroupConstructionError",
        "SimpleGroupStateEventKind",
        "SimpleGroupMetadata",
        "SimpleGroupAdmins",
        "SimpleGroupMembers",
        "SimpleGroupRoles",
        "SimpleGroupLivekitParticipants",
        "SimpleGroupPins",
        "SimpleGroupDecodeError",
        "SavedSimpleGroup",
        "SavedGroupList",
        "SavedGroupListDecodeError",
    ] {
        assert!(
            PUBLIC_ROOT.contains(required),
            "missing current export {required}"
        );
    }
    for removed in [
        "SimpleGroups",
        "SimpleGroupRecords",
        "SimpleGroupSnapshot",
        "SimpleGroupParticipants",
        "RelaySequence",
        "RelaySequenceError",
        "PinnedItem",
        "SavedRelay",
    ] {
        assert!(
            !PUBLIC_ROOT.contains(removed),
            "obsolete export survived: {removed}"
        );
    }
}

#[test]
fn constructor_decision_and_vocabulary_describe_the_exported_boundary() {
    for required in [
        "from_relays(id, relays: Vec<RelayUrl>)",
        "Result<SimpleGroup, SimpleGroupConstructionError>",
        "`EmptyId` rejects exactly a zero-length id",
        "`EmptyRelays` rejects exactly an empty vector",
    ] {
        assert!(
            CONSTRUCTOR_DECISION.contains(required),
            "constructor decision lost {required}"
        );
    }

    let simple_group = vocabulary_term("SimpleGroup");
    assert!(simple_group.contains("source = \"nostr\""));
    assert!(simple_group.contains("One non-empty opaque simple-group id"));

    let construction_error = vocabulary_term("SimpleGroupConstructionError");
    assert!(construction_error.contains("source = \"fava\""));
    assert!(construction_error.contains("EmptyId"));
    assert!(construction_error.contains("EmptyRelays"));
}

fn vocabulary_term(name: &str) -> &'static str {
    let marker = format!("[[term]]\nname = \"{name}\"\n");
    let (_, rest) = VOCABULARY
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing vocabulary term {name}"));
    rest.split_once("\n[[term]]").map_or(rest, |(term, _)| term)
}

#[test]
fn production_has_no_duplicate_generic_owner_or_obsolete_policy_path() {
    let forbidden = [
        "verify_signature(",
        "QuerySnapshot",
        "SimpleGroupSnapshot",
        "metadata_differ",
        "TooManyRecord",
        "MAX_RECORD_",
        "MAX_SIMPLE_GROUP_",
        "RelaySequence",
        "RelaySequenceError",
        "edit_metadata",
        "set_pins",
        "simple_groups_where_",
        "simple_groups_saved_by",
    ];
    for (path, source) in sources() {
        for needle in forbidden {
            assert!(
                !source.contains(needle),
                "{path} retains forbidden owner/path {needle}"
            );
        }
    }
}

#[test]
fn production_files_respect_code_line_limits() {
    for (path, source) in sources() {
        let lines = source.lines().count();
        assert!(lines <= 800, "{path} has {lines} lines");
    }
}

#[test]
fn compiler_inventory_is_grouped_described_evidenced_and_catalogued() {
    let body = README
        .split_once("<!-- BEGIN crate-readme-api inventory -->")
        .and_then(|(_, rest)| rest.split_once("<!-- END crate-readme-api inventory -->"))
        .map(|(body, _)| body)
        .expect("managed README inventory");
    assert!(!body.contains("| Kind | Item | Description |"));
    let owner_sections = body
        .lines()
        .filter(|line| line.starts_with("### `"))
        .count();
    assert!(owner_sections > 0, "inventory has no public owners");
    assert_eq!(body.matches("| Item | Purpose |").count(), owner_sections);
    assert_eq!(body.matches("```rust,no_run").count(), owner_sections);
    assert!(!body.contains("Compiler-visible"));

    let metadata = body
        .lines()
        .filter(|line| line.contains("<!-- api-item "))
        .collect::<Vec<_>>();
    let catalog = CATALOG.lines().collect::<Vec<_>>();
    assert!(!metadata.is_empty());
    assert_eq!(metadata.len(), catalog.len());

    let mut identities = BTreeSet::new();
    for (readme, catalog) in metadata.into_iter().zip(catalog) {
        for required in ["\"kind\":", "\"item\":", "\"signature\":", "\"evidence\":"] {
            assert!(
                readme.contains(required),
                "incomplete API metadata: {readme}"
            );
        }
        let item = readme
            .split_once("\"item\":\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(item, _)| item)
            .expect("metadata item");
        assert!(identities.insert(item), "duplicate API identity: {item}");
        assert!(
            catalog.contains(&format!("\"item\": \"{item}\"")),
            "catalog disagrees for {item}"
        );
        assert!(catalog.contains("\"purpose\": ") && catalog.contains("\"evidence\": "));
    }
}
