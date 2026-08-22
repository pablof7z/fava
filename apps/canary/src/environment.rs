//! Fail-closed resolution of machine-local canary prerequisites.

use std::path::PathBuf;

use crate::{CanaryError, CanaryResult};

const CROISSANT_BINARY: &str = "/Users/pablofernandez/Work/croissant/target/release/croissant";
const CROISSANT_SOURCE: &str = "/Users/pablofernandez/Work/croissant";

pub(crate) fn croissant_fixture_binary() -> CanaryResult<PathBuf> {
    Ok(PathBuf::from(CROISSANT_BINARY))
}

pub(crate) fn croissant_fixture_source() -> CanaryResult<PathBuf> {
    Ok(PathBuf::from(CROISSANT_SOURCE))
}

pub(crate) fn bazel_program() -> CanaryResult<PathBuf> {
    Err(CanaryError::new("Bazel executable is unavailable"))
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
