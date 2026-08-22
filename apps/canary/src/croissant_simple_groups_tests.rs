use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::croissant::{CroissantLimits, CroissantSupervisor, process_is_alive};
use super::croissant_simple_groups::supervise_owned_pair;
use super::{CanaryError, repository_root};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_croissant_children_are_always_reaped() {
    let fixture = PairFixture::new();
    let completion = supervise_owned_pair(fixture.supervisors(), |_| async { Ok(()) })
        .await
        .expect("both exact children complete");
    assert_eq!(completion.flow, ());
    assert_pair_cleanup(&completion.ready, &completion.teardown);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_child_failure_still_reaps_first() {
    let fixture = PairFixture::new();
    let failure = supervise_owned_pair(fixture.supervisors(), |_| async {
        Err::<(), _>(CanaryError::new("injected second-child flow failure"))
    })
    .await
    .expect_err("injected flow failure remains attributed");
    assert_eq!(
        failure.ready.len(),
        2,
        "both exact children reached readiness"
    );
    assert_eq!(
        failure.teardown.len(),
        2,
        "both cleanup results were captured"
    );
    let teardown = failure
        .teardown
        .iter()
        .map(|result| result.as_ref().expect("both exact cleanups complete"))
        .collect::<Vec<_>>();
    for (ready, teardown) in failure.ready.iter().zip(teardown) {
        assert_eq!(ready.pid, teardown.pid);
        assert_ne!(teardown.pid, 75_649, "forbidden unowned PID was touched");
        assert!(!process_is_alive(teardown.pid));
        assert!(!teardown.port_open_after);
    }

    let startup_fixture = PairFixture::new();
    let startup_failure =
        supervise_owned_pair(startup_fixture.supervisors_with_failing_b(), |_| async {
            Ok(())
        })
        .await
        .expect_err("second-child startup failure remains attributed");
    assert_eq!(startup_failure.ready.len(), 1);
    assert_eq!(startup_failure.teardown.len(), 1);
    let first_cleanup = startup_failure.teardown[0]
        .as_ref()
        .expect("first child cleanup completes after B startup failure");
    assert_eq!(startup_failure.ready[0].pid, first_cleanup.pid);
    assert!(!process_is_alive(first_cleanup.pid));
}

fn assert_pair_cleanup(
    ready: &[super::croissant::CroissantReadyFact; 2],
    teardown: &[super::croissant::CroissantTeardown; 2],
) {
    assert_ne!(ready[0].pid, ready[1].pid);
    assert_ne!(ready[0].endpoint, ready[1].endpoint);
    assert_ne!(ready[0].data_path, ready[1].data_path);
    assert_ne!(ready[0].stdout_path, ready[1].stdout_path);
    assert_ne!(ready[0].stderr_path, ready[1].stderr_path);
    for (ready, teardown) in ready.iter().zip(teardown) {
        assert_eq!(ready.pid, teardown.pid);
        assert_ne!(teardown.pid, 75_649, "forbidden unowned PID was touched");
        assert!(teardown.completed);
        assert!(!teardown.pid_alive_after);
        assert!(!teardown.port_open_after);
        assert!(!process_is_alive(teardown.pid));
    }
}

struct PairFixture {
    temporary: TempDir,
    binary: PathBuf,
}

impl PairFixture {
    fn new() -> Self {
        let temporary = TempDir::new().expect("pair fixture root");
        let binary = executable(temporary.path());
        Self { temporary, binary }
    }

    fn supervisors(&self) -> [CroissantSupervisor; 2] {
        let source = repository_root().expect("source checkout");
        [
            supervisor(
                &self.binary,
                &source,
                &self.temporary.path().join("relay-a"),
                &owner("relay-a"),
                &seed_hash(b"pair-fixture-a"),
            ),
            supervisor(
                &self.binary,
                &source,
                &self.temporary.path().join("relay-b"),
                &owner("relay-b"),
                &seed_hash(b"pair-fixture-b"),
            ),
        ]
    }

    fn supervisors_with_failing_b(&self) -> [CroissantSupervisor; 2] {
        let source = repository_root().expect("source checkout");
        let failing = failing_executable(self.temporary.path());
        [
            supervisor(
                &self.binary,
                &source,
                &self.temporary.path().join("startup-relay-a"),
                &owner("startup-relay-a"),
                &seed_hash(b"startup-fixture-a"),
            ),
            supervisor(
                &failing,
                &source,
                &self.temporary.path().join("startup-relay-b"),
                &owner("startup-relay-b"),
                &seed_hash(b"startup-fixture-b"),
            ),
        ]
    }
}

fn supervisor(
    binary: &Path,
    source: &Path,
    root: &Path,
    owner: &str,
    seed: &str,
) -> CroissantSupervisor {
    CroissantSupervisor::prepare(binary, source, root, owner, seed, CroissantLimits::test())
        .expect("exact child supervisor")
}

fn executable(root: &Path) -> PathBuf {
    let path = root.join("controlled-croissant-fixture");
    fs::write(
        &path,
        "#!/bin/sh\nexec python3 -c 'import os,socket,time; s=socket.socket(); s.bind((\"127.0.0.1\",int(os.environ[\"PORT\"]))); s.listen(); time.sleep(30)'\n",
    )
    .expect("fixture executable");
    let mut permissions = fs::metadata(&path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("fixture permissions");
    path
}

fn failing_executable(root: &Path) -> PathBuf {
    let path = root.join("failing-croissant-fixture");
    fs::write(&path, "#!/bin/sh\nexit 71\n").expect("failing fixture executable");
    let mut permissions = fs::metadata(&path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("fixture permissions");
    path
}

fn owner(label: &str) -> String {
    hex::encode(Sha256::digest(label.as_bytes()))
}

fn seed_hash(seed: &[u8]) -> String {
    hex::encode(Sha256::digest(seed))
}
