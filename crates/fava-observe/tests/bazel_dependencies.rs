//! Executable mapping from production Rust imports to Cargo and Bazel edges.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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

fn bazel_first_party_deps(target: &str) -> BTreeSet<String> {
    target
        .split('"')
        .filter_map(|label| {
            label
                .strip_prefix("//crates/")
                .and_then(|label| label.strip_suffix(":lib"))
        })
        .map(ToOwned::to_owned)
        .collect()
}

fn crate_root() -> PathBuf {
    if let Some(workspace) = std::env::var_os("BUILD_WORKSPACE_DIRECTORY") {
        return PathBuf::from(workspace).join("crates/fava-observe");
    }
    if let (Some(runfiles), Some(workspace)) = (
        std::env::var_os("TEST_SRCDIR"),
        std::env::var_os("TEST_WORKSPACE"),
    ) {
        return PathBuf::from(runfiles)
            .join(workspace)
            .join("crates/fava-observe");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources(root: &Path, sources: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("source directory exists") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

fn referenced_first_party_crates(source: &str) -> BTreeSet<String> {
    let production = source.split("#[cfg(test)]").next().unwrap_or(source);
    let mut crates = BTreeSet::new();
    for line in production.lines() {
        let code = line.split("//").next().unwrap_or_default();
        for (start, _) in code.match_indices("fava_") {
            let name: String = code[start..]
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect();
            if code[start + name.len()..].starts_with("::") {
                crates.insert(name.replace('_', "-"));
            }
        }
    }
    crates
}

#[test]
fn production_imports_map_exactly_to_cargo_and_bazel_dependencies() {
    let cargo: BTreeSet<_> = toml_table_keys(CARGO_MANIFEST, "dependencies")
        .into_iter()
        .filter(|dependency| dependency.starts_with("fava-"))
        .collect();
    let bazel = bazel_first_party_deps(starlark_call(BAZEL_MANIFEST, "rust_library"));

    let mut paths = Vec::new();
    rust_sources(&crate_root().join("src"), &mut paths);
    assert!(!paths.is_empty(), "production source set must be non-empty");
    let mut imports = BTreeSet::new();
    for path in paths {
        let source = fs::read_to_string(path).expect("Rust source is readable");
        imports.extend(referenced_first_party_crates(&source));
    }

    assert_eq!(
        imports, cargo,
        "Cargo must match production crate references"
    );
    assert_eq!(bazel, cargo, "Bazel must map every Cargo first-party edge");
}
