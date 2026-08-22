use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use nostr::key::Keys;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::croissant::{CroissantLimits, CroissantSupervisor, process_is_alive};
use super::croissant_simple_groups::{
    CroissantSimpleGroupsOptions, prepare_owned_supervisors, supervise_owned_pair,
};
use super::croissant_simple_groups_flow::execute_public_flow;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn croissant_simple_groups_public_flow() {
    let temporary = TempDir::new().expect("public-flow fixture root");
    let source = PathBuf::from("/Users/pablo/Work/croissant");
    let binary = build_croissant(&source, temporary.path());
    let seed = "controlled-simple-groups-public-flow";
    let options = CroissantSimpleGroupsOptions {
        relay_binary: binary,
        source_checkout: source,
        scenario_seed: seed.to_owned(),
        runs_directory: temporary.path().join("unused-retained-root"),
    };
    let relay_keys = Keys::generate();
    let owner_a = Keys::generate().public_key().to_hex();
    let owner_b = Keys::generate().public_key().to_hex();
    assert_ne!(owner_a, owner_b);
    let run_root = temporary.path().join("run");
    fs::create_dir(&run_root).expect("run root");
    let supervisors = prepare_owned_supervisors(
        &options,
        &run_root,
        &relay_keys,
        [&owner_a, &owner_b],
        CroissantLimits::default(),
    )
    .expect("two exact Croissant supervisors");
    let flow_root = run_root.clone();
    let flow_seed = seed.to_owned();
    let completion = supervise_owned_pair(supervisors, move |ready| async move {
        execute_public_flow(&flow_root, &flow_seed, ready).await
    })
    .await
    .expect("controlled public flow completes");
    let facts = completion.flow;

    assert_eq!(facts.shared_evidence, facts.relay_urls);
    assert_ne!(facts.shared_event_id, facts.unique_event_ids[0]);
    assert_ne!(facts.shared_event_id, facts.unique_event_ids[1]);
    assert_ne!(facts.unique_event_ids[0], facts.unique_event_ids[1]);
    assert_eq!(facts.metadata_names, ["relay-A", "relay-B"]);
    assert_eq!(facts.metadata_authors[0], relay_keys.public_key().to_hex());
    assert_eq!(facts.metadata_authors[0], facts.metadata_authors[1]);
    assert_ne!(facts.admin_targets[0], facts.admin_targets[1]);
    assert_eq!(facts.admin_authors[0], relay_keys.public_key().to_hex());
    assert_eq!(facts.admin_authors[0], facts.admin_authors[1]);
    assert!(!facts.group_id.is_empty());
    assert!(!facts.custom_event_id.is_empty());
    assert_ne!(facts.write_id, 0);
    assert_ne!(facts.receipt_id, 0);
    assert_eq!(facts.custom_destinations, 2);
    assert_eq!(facts.custom_acknowledged, 2);
    assert_eq!(facts.handoffs, [1, 1]);
    assert_eq!(facts.signed_refusals, 3);
    assert!(facts.observation_closed);
    assert_pair_cleanup(&completion.ready, &completion.teardown);
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

fn build_croissant(source: &Path, root: &Path) -> PathBuf {
    let binary = root.join("croissant");
    let output = Command::new("go")
        .args(["build", "-mod=vendor", "-o"])
        .arg(&binary)
        .arg(".")
        .current_dir(source)
        .output()
        .expect("go build launches");
    assert!(
        output.status.success(),
        "controlled Croissant build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

fn owner(label: &str) -> String {
    hex::encode(Sha256::digest(label.as_bytes()))
}

fn seed_hash(seed: &[u8]) -> String {
    hex::encode(Sha256::digest(seed))
}
