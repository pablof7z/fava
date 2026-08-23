//! Fail-closed resolution of machine-local canary prerequisites.

#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::OnceLock;

#[cfg(test)]
use tokio::sync::{Mutex, MutexGuard};

#[cfg(all(unix, test))]
use std::os::unix::fs::PermissionsExt;

use crate::{CanaryError, CanaryResult};

#[cfg(test)]
const CROISSANT_BINARY: &str = "/Users/pablo/.local/bin/croissant";
const CROISSANT_SOURCE: &str = "/Users/pablo/Work/croissant";

#[cfg(test)]
static LIVE_CROISSANT_FIXTURE: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn croissant_fixture_binary() -> CanaryResult<PathBuf> {
    require_executable(Path::new(CROISSANT_BINARY), "Croissant fixture")
}

#[cfg(test)]
pub(crate) async fn croissant_fixture_guard() -> MutexGuard<'static, ()> {
    LIVE_CROISSANT_FIXTURE
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await
}

pub(crate) fn croissant_fixture_source() -> CanaryResult<PathBuf> {
    let source = PathBuf::from(CROISSANT_SOURCE);
    if source.is_dir() && source.join(".git").exists() {
        return Ok(source);
    }
    Err(CanaryError::new("Croissant source checkout is unavailable"))
}

#[cfg(test)]
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

    use std::time::Duration;

    use super::{croissant_fixture_binary, croissant_fixture_guard, croissant_fixture_source};

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn live_croissant_fixture_has_one_suite_owner() {
        let first = croissant_fixture_guard().await;
        let blocked = tokio::time::timeout(Duration::from_millis(50), async {
            let _second = croissant_fixture_guard().await;
        })
        .await;
        assert!(blocked.is_err(), "a second live fixture owner entered");
        drop(first);
        let _released = tokio::time::timeout(Duration::from_secs(1), croissant_fixture_guard())
            .await
            .expect("fixture owner releases within deadline");
    }
}
