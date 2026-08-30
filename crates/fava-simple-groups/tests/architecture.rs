//! Executable ownership, dependency, subtraction, and line-bound gates.

use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const PUBLIC_ROOT: &str = include_str!("../src/lib.rs");
fn sources() -> [(&'static str, &'static str); 10] {
    [
        ("edit.rs", include_str!("../src/edit.rs")),
        ("lib.rs", include_str!("../src/lib.rs")),
        ("management.rs", include_str!("../src/management.rs")),
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
    assert!(PUBLIC_ROOT.contains("mod management;"));
    assert!(!PUBLIC_ROOT.contains("pub mod management;"));
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
        "SimpleGroups",
    ] {
        assert!(
            PUBLIC_ROOT.contains(required),
            "missing current export {required}"
        );
    }
    for removed in [
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
