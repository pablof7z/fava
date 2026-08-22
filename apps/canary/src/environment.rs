//! Fail-closed resolution of machine-local canary prerequisites.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::{CanaryError, CanaryResult};

const CROISSANT_BINARY: &str = "/Users/pablo/.local/bin/croissant";
const CROISSANT_SOURCE: &str = "/Users/pablo/Work/croissant";
const LOCAL_BAZELISK: &str = "/Users/pablo/.local/bin/bazelisk";

pub(crate) fn croissant_fixture_binary() -> CanaryResult<PathBuf> {
    require_executable(Path::new(CROISSANT_BINARY), "Croissant fixture")
}

pub(crate) fn croissant_fixture_source() -> CanaryResult<PathBuf> {
    let source = PathBuf::from(CROISSANT_SOURCE);
    if source.is_dir() && source.join(".git").exists() {
        return Ok(source);
    }
    Err(CanaryError::new(
        "Croissant source checkout is unavailable",
    ))
}

pub(crate) fn bazel_program() -> CanaryResult<PathBuf> {
    for name in ["bazel", "bazelisk"] {
        if let Some(path) = executable_on_path(name) {
            return Ok(path);
        }
    }
    require_executable(Path::new(LOCAL_BAZELISK), "Bazelisk").map_err(|_| {
        CanaryError::new("neither Bazel nor Bazelisk is available for the product graph probe")
    })
}

fn executable_on_path(name: &str) -> Option<PathBuf> {
    let search_path = env::var_os("PATH")?;
    env::split_paths(&search_path)
        .map(|directory| directory.join(name))
        .find_map(|candidate| require_executable(&candidate, name).ok())
}

fn require_executable(path: &Path, label: &str) -> CanaryResult<PathBuf> {
    let metadata = fs::metadata(path)
        .map_err(|_| CanaryError::new(format!("{label} executable is unavailable")))?;
    #[cfg(unix)]
    let executable = metadata.permissions().mode() & 0o111 != 0;
    #[cfg(not(unix))]
    let executable = true;
    if metadata.is_file() && executable {
        return Ok(path.to_path_buf());
    }
    Err(CanaryError::new(format!(
        "{label} path is not an executable file"
    )))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{bazel_program, croissant_fixture_binary, croissant_fixture_source};

    #[test]
    fn current_croissant_fixture_is_resolved_explicitly() {
        assert_eq!(
            croissant_fixture_binary().expect("Croissant fixture binary"),
            Path::new("/Users/pablo/.local/bin/croissant")
        );
        assert_eq!(
            croissant_fixture_source().expect("Croissant source checkout"),
            Path::new("/Users/pablo/Work/croissant")
        );
    }

    #[test]
    fn bazelisk_is_accepted_when_bazel_is_unavailable() {
        assert_eq!(
            bazel_program().expect("Bazel or Bazelisk prerequisite"),
            Path::new("/Users/pablo/.local/bin/bazelisk")
        );
    }
}
