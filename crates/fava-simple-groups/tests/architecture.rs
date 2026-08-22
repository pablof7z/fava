//! Executable exact dependency boundary for the simple-groups capability.

use std::collections::BTreeSet;

const CARGO_MANIFEST: &str = include_str!("../Cargo.toml");
const BAZEL_MANIFEST: &str = include_str!("../BUILD.bazel");

fn toml_table_keys(manifest: &str, table: &str) -> BTreeSet<String> {
    let header = format!("[{table}]");
    let mut in_table = false;
    let mut keys = BTreeSet::new();

    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_table = line == header;
            continue;
        }
        if in_table && !line.is_empty() {
            let (key, _) = line
                .split_once('=')
                .expect("dependency entries are TOML key/value assignments");
            keys.insert(
                key.trim()
                    .strip_suffix(".workspace")
                    .unwrap_or(key.trim())
                    .to_owned(),
            );
        }
    }

    keys
}

fn starlark_call<'a>(source: &'a str, function: &str) -> &'a str {
    let start = source
        .find(&format!("{function}("))
        .expect("Starlark call exists");
    let source = &source[start..];
    let mut depth = 0_u32;
    for (index, byte) in source.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return &source[..=index];
                }
            }
            _ => {}
        }
    }
    panic!("Starlark call is balanced");
}

fn first_party_library_deps(target: &str) -> BTreeSet<String> {
    target
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            line.strip_prefix('"')
                .and_then(|line| line.strip_suffix("\","))
        })
        .filter(|label| label.starts_with("//crates/") && label.ends_with(":lib"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn normal_dependencies_are_exact() {
    let cargo_expected = BTreeSet::from([
        "fava-query".to_owned(),
        "fava-state".to_owned(),
        "fava-write".to_owned(),
    ]);
    let bazel_expected = BTreeSet::from([
        "//crates/fava-query:lib".to_owned(),
        "//crates/fava-state:lib".to_owned(),
        "//crates/fava-write:lib".to_owned(),
    ]);

    let cargo_actual = toml_table_keys(CARGO_MANIFEST, "dependencies");
    let bazel_actual = first_party_library_deps(starlark_call(BAZEL_MANIFEST, "rust_library"));

    assert_eq!(
        (cargo_actual, bazel_actual),
        (cargo_expected, bazel_expected),
        "Cargo and Bazel normal dependencies must equal the approved neutral owners"
    );
}
