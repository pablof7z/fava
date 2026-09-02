//! One authentication lifecycle vocabulary exists, and it lives here.
//!
//! `fava-auth` owns the lifecycle that produces these values; `fava-relay`
//! owns the values themselves, beside the `Authority` they describe. A
//! second state, outcome, or verdict enum anywhere in the workspace would be
//! an alternate representation of an existing noun.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate sits two levels below the workspace root")
        .to_owned()
}

fn rust_sources(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name == "target" || name == ".git" {
                continue;
            }
            rust_sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn one_authentication_lifecycle_enum_exists_in_the_workspace() {
    let root = workspace_root();
    let mut sources = Vec::new();
    for area in ["crates", "apps", "examples", "falsifiers"] {
        rust_sources(&root.join(area), &mut sources);
    }
    assert!(
        sources.len() > 100,
        "expected to scan the whole workspace, saw {} files",
        sources.len()
    );

    let mut definitions = Vec::new();
    for source in &sources {
        let Ok(text) = fs::read_to_string(source) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim_start();
            for noun in [
                "enum Progress",
                "enum AuthenticationOutcome",
                "enum AuthenticationVerdict",
                "enum AuthState",
                "enum AuthOutcome",
            ] {
                if line.starts_with(noun) || line.starts_with(&format!("pub {noun}")) {
                    definitions.push(format!("{}: {line}", source.display()));
                }
            }
        }
    }

    assert_eq!(
        definitions.len(),
        1,
        "exactly one authentication lifecycle enum may exist; found: {definitions:#?}"
    );
    assert!(
        definitions[0].contains("fava-relay"),
        "the lifecycle enum belongs beside Authority in fava-relay, found at {}",
        definitions[0]
    );
}
