use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use nostr::key::Keys;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::croissant::{CroissantLimits, CroissantSupervisor, process_is_alive};
use super::croissant_simple_groups::{
    CroissantSimpleGroupsOptions, prepare_owned_supervisors, supervise_owned_pair,
};
use super::croissant_simple_groups_evidence::{SCENARIO, verify_croissant_simple_groups_pair};
use super::croissant_simple_groups_evidence_support::{
    SECRET_SCAN_CLASSES, artifact_hashes, artifact_seal,
};
use super::croissant_simple_groups_flow::execute_public_flow;
use super::{CanaryError, repository_root};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_croissant_children_are_always_reaped() {
    let fixture = PairFixture::new();
    let completion = supervise_owned_pair(fixture.supervisors(), |_| async { Ok(()) })
        .await
        .expect("both exact children complete");
    let () = completion.flow;
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
    let completion = Box::pin(supervise_owned_pair(supervisors, move |ready| {
        Box::pin(async move { Box::pin(execute_public_flow(&flow_root, &flow_seed, ready)).await })
    }))
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

#[test]
fn pair_verifier_rejects_unsafe_evidence() {
    let control = PairEvidenceFixture::new();
    verify_croissant_simple_groups_pair(control.root()).expect("narrow safe pair verifies");

    for case in [
        UnsafePairCase::PersistentParentSecret,
        UnsafePairCase::IncompleteCleanup,
        UnsafePairCase::UnsignedClaim,
        UnsafePairCase::ReusedIdentity,
        UnsafePairCase::CrossRunData,
        UnsafePairCase::ExtraManifest,
        UnsafePairCase::MissingManifest,
        UnsafePairCase::StagingResidue,
    ] {
        let fixture = PairEvidenceFixture::new();
        fixture.apply(case);
        assert!(
            verify_croissant_simple_groups_pair(fixture.root()).is_err(),
            "pair verifier accepted unsafe fixture {case:?}"
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum UnsafePairCase {
    PersistentParentSecret,
    IncompleteCleanup,
    UnsignedClaim,
    ReusedIdentity,
    CrossRunData,
    ExtraManifest,
    MissingManifest,
    StagingResidue,
}

struct PairEvidenceFixture {
    temporary: TempDir,
    authors: [Keys; 2],
    roots: [PathBuf; 2],
}

impl PairEvidenceFixture {
    fn new() -> Self {
        let temporary = TempDir::new().expect("pair evidence root");
        let authors = [Keys::generate(), Keys::generate()];
        let roots = [
            temporary.path().join("run-a"),
            temporary.path().join("run-b"),
        ];
        for (index, root) in roots.iter().enumerate() {
            fs::create_dir(root).expect("run fixture root");
            fs::write(root.join("flow.json"), format!("run-{index}-own-data"))
                .expect("flow fixture");
            write_pair_manifest(root, index, &authors[index]);
        }
        Self {
            temporary,
            authors,
            roots,
        }
    }

    fn root(&self) -> &Path {
        self.temporary.path()
    }

    fn apply(&self, case: UnsafePairCase) {
        match case {
            UnsafePairCase::PersistentParentSecret => {
                fs::write(
                    self.root().join("scenario-seed.secret"),
                    "persistent-parent-secret",
                )
                .expect("parent residue");
            }
            UnsafePairCase::IncompleteCleanup => self.mutate(0, true, |manifest| {
                manifest["teardown"][1]["completed"] = json!(false);
            }),
            UnsafePairCase::UnsignedClaim => self.mutate(0, false, |manifest| {
                manifest["signed_refusals"] = json!(2);
            }),
            UnsafePairCase::ReusedIdentity => {
                let first = read_manifest(&self.roots[0]);
                let pid = first["ready"][0]["pid"].clone();
                self.mutate(1, true, |manifest| {
                    manifest["ready"][0]["pid"] = pid.clone();
                    manifest["teardown"][0]["pid"] = pid;
                });
            }
            UnsafePairCase::CrossRunData => {
                let second = read_manifest(&self.roots[1]);
                fs::write(
                    self.roots[0].join("cross-run.txt"),
                    second["group_id"].as_str().expect("group id"),
                )
                .expect("cross-run artifact");
                self.mutate(0, true, |_| {});
            }
            UnsafePairCase::ExtraManifest => {
                let extra = self.root().join("run-extra");
                fs::create_dir(&extra).expect("extra root");
                fs::write(extra.join("manifest.json"), "{}\n").expect("extra manifest");
            }
            UnsafePairCase::MissingManifest => {
                fs::remove_file(self.roots[1].join("manifest.json")).expect("missing manifest");
            }
            UnsafePairCase::StagingResidue => {
                fs::create_dir(self.root().join(".fava-canary-staging-residue"))
                    .expect("staging residue");
            }
        }
    }

    fn mutate(&self, index: usize, reseal: bool, update: impl FnOnce(&mut Value)) {
        let mut manifest = read_manifest(&self.roots[index]);
        update(&mut manifest);
        if reseal {
            manifest["artifact_sha256"] =
                serde_json::to_value(artifact_hashes(&self.roots[index]).expect("fixture hashes"))
                    .expect("hash value");
            let seal = artifact_seal(&self.authors[index], &manifest).expect("fixture reseal");
            manifest["artifact_seal"] = serde_json::to_value(seal).expect("seal value");
        }
        fs::write(
            self.roots[index].join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).expect("manifest bytes"),
        )
        .expect("mutated manifest");
    }
}

fn write_pair_manifest(root: &Path, index: usize, author: &Keys) {
    let base_pid = 4_000_100 + (index as u64 * 2);
    let port = 49_000 + (u16::try_from(index).expect("fixture index fits u16") * 10);
    let relay_urls = [
        format!("ws://127.0.0.1:{port}"),
        format!("ws://127.0.0.1:{}", port + 1),
    ];
    let relay_signer = Keys::generate().public_key().to_hex();
    let ready = [0_u64, 1].map(|child| {
        json!({
            "pid": base_pid + child,
            "endpoint": relay_urls[usize::try_from(child).expect("child index fits usize")].trim_start_matches("ws://"),
            "data_path": format!("/discarded/run-{index}/relay-{child}"),
            "stdout_path": format!("/discarded/run-{index}/relay-{child}.stdout"),
            "stderr_path": format!("/discarded/run-{index}/relay-{child}.stderr"),
            "executable": format!("/controlled/croissant-{index}"),
            "executable_sha256": format!("executable-{index}"),
            "source_checkout": format!("/controlled/source-{index}"),
            "source_head": format!("source-{index}"),
            "scenario_seed_sha256": format!("seed-{index}"),
        })
    });
    let teardown = [0_u64, 1].map(|child| {
        json!({
            "pid": base_pid + child,
            "endpoint": relay_urls[usize::try_from(child).expect("child index fits usize")].trim_start_matches("ws://"),
            "completed": true,
            "pid_alive_after": false,
            "port_open_after": false,
        })
    });
    let mut manifest = json!({
        "run_id": format!("run-{index}"),
        "scenario": SCENARIO,
        "scenario_seed_sha256": format!("seed-{index}"),
        "author_public_key": author.public_key().to_hex(),
        "relay_signer_public_key": relay_signer.clone(),
        "relay_owner_public_keys": [Keys::generate().public_key().to_hex(), Keys::generate().public_key().to_hex()],
        "group_id": format!("group-{index}"),
        "relay_urls": relay_urls,
        "shared_event_id": format!("shared-{index}"),
        "unique_event_ids": [format!("unique-a-{index}"), format!("unique-b-{index}")],
        "custom_event_id": format!("custom-{index}"),
        "shared_evidence": relay_urls,
        "metadata_names": [format!("metadata-a-{index}"), format!("metadata-b-{index}")],
        "metadata_authors": [relay_signer.clone(), relay_signer.clone()],
        "admin_targets": [format!("admin-a-{index}"), format!("admin-b-{index}")],
        "admin_authors": [relay_signer.clone(), relay_signer.clone()],
        "write_id": format!("run-{index}:1"),
        "receipt_id": format!("run-{index}:1"),
        "custom_destinations": 2,
        "custom_acknowledged": 2,
        "handoffs": [1, 1],
        "signed_refusals": 3,
        "observation_closed": true,
        "ready": ready,
        "teardown": teardown,
        "pre_seal_secret_scan_passed": true,
        "post_manifest_secret_scan_passed": true,
        "secret_scan_classes": SECRET_SCAN_CLASSES,
        "secret_scan_key_count": 6,
        "bounds": {"operation_ms": 30_000, "wire_bytes": 2_097_152, "wire_bytes_observed": 10,
            "log_bytes": 1_048_576, "readiness_ms": 10_000, "readiness_stability_ms": 100,
            "teardown_ms": 5000},
        "artifact_sha256": artifact_hashes(root).expect("fixture hashes"),
        "fava_revision": format!("revision-{index}"),
    });
    manifest["artifact_seal"] =
        serde_json::to_value(artifact_seal(author, &manifest).expect("fixture seal"))
            .expect("seal value");
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("manifest bytes"),
    )
    .expect("manifest fixture");
}

fn read_manifest(root: &Path) -> Value {
    serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("manifest read"))
        .expect("manifest json")
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
