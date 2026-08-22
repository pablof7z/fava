//! Shared sealed-evidence fixture for every simple-groups verifier mutation test.
//!
//! This file stays above the 500-line soft limit because all review-iteration includes must mutate
//! one byte-identical valid fixture instead of duplicating or weakening the verifier oracle.

use super::croissant_simple_groups_evidence::{SCENARIO, verify_croissant_simple_groups_pair};
use super::croissant_simple_groups_evidence_support::{
    SECRET_SCAN_CLASSES, artifact_hashes, artifact_seal,
};
use crate::CanaryResult;
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;
use nostr::types::Timestamp;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
include!("croissant_simple_groups_tests/public_flow.rs");
include!("croissant_simple_groups_tests/review_iteration_one.rs");
include!("croissant_simple_groups_tests/review_iteration_two.rs");
include!("croissant_simple_groups_tests/review_iteration_three.rs");
include!("croissant_simple_groups_tests/review_iteration_four.rs");
include!("croissant_simple_groups_tests/review_iteration_five.rs");
include!("croissant_simple_groups_tests/review_iteration_six.rs");

struct PairEvidenceFixture {
    temporary: TempDir,
    authors: [Keys; 2],
    relays: [Keys; 2],
    roots: [PathBuf; 2],
}

impl PairEvidenceFixture {
    fn new() -> Self {
        let temporary = TempDir::new().expect("pair evidence root");
        let authors = [Keys::generate(), Keys::generate()];
        let relays = [Keys::generate(), Keys::generate()];
        let roots = [
            temporary.path().join("run-0"),
            temporary.path().join("run-1"),
        ];
        for (index, root) in roots.iter().enumerate() {
            fs::create_dir(root).expect("run fixture root");
            write_pair_manifest(root, index, &authors[index], &relays[index]);
        }
        Self {
            temporary,
            authors,
            relays,
            roots,
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive hostile-fixture dispatcher keeps every refusal mutation explicit"
    )]
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
            UnsafePairCase::ReusedUniqueIdentity => {
                let first = read_manifest(&self.roots[0]);
                let unique = first["unique_event_ids"][1].clone();
                self.mutate(1, true, |manifest| {
                    manifest["unique_event_ids"][0] = unique;
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
            UnsafePairCase::CrossRunUniqueData => {
                let second = read_manifest(&self.roots[1]);
                fs::write(
                    self.roots[0].join("cross-run-unique.txt"),
                    second["unique_event_ids"][0]
                        .as_str()
                        .expect("unique event id"),
                )
                .expect("cross-run unique artifact");
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
            UnsafePairCase::ExecutableResidue => {
                let directory = self.roots[0].join("relays/a/executable");
                fs::create_dir_all(&directory).expect("executable residue directory");
                fs::write(directory.join("croissant"), b"retained executable")
                    .expect("executable residue");
            }
            UnsafePairCase::UnderivedFlowClaim => self.mutate(0, true, |manifest| {
                manifest["metadata_names"][0] = json!("manifest-only-name");
            }),
            UnsafePairCase::UnderivedProcessClaim => {
                let path = self.roots[0].join("children/processes.json");
                let mut processes: Value =
                    serde_json::from_slice(&fs::read(&path).expect("process fixture read"))
                        .expect("process fixture json");
                processes["teardown"][0]["completed"] = json!(false);
                fs::write(
                    &path,
                    serde_json::to_vec_pretty(&processes).expect("process bytes"),
                )
                .expect("process fixture mutation");
                self.mutate(0, true, |_| {});
            }
            UnsafePairCase::MissingExactClose => {
                self.rewrite_wire(0, "a", |wire| {
                    wire.lines()
                        .filter(|line| !(line.contains("CLOSE") && line.contains("content-0")))
                        .collect::<Vec<_>>()
                        .join("\n")
                        + "\n"
                });
            }
            UnsafePairCase::ExtraSignedHandoff => {
                self.rewrite_wire(0, "a", |wire| {
                    let event = wire
                        .lines()
                        .find(|line| line.contains("client_to_relay") && line.contains("EVENT"))
                        .expect("client EVENT line");
                    format!("{wire}{event}\n")
                });
            }
            UnsafePairCase::RetainedSecretMarker => {
                fs::write(self.roots[0].join("retained.txt"), "nsec1forbidden")
                    .expect("secret marker fixture");
                self.mutate(0, true, |_| {});
            }
        }
    }

    fn rewrite_wire(&self, index: usize, label: &str, update: impl FnOnce(&str) -> String) {
        let path = self.roots[index].join(format!("wire/{label}.jsonl"));
        let wire = fs::read_to_string(&path).expect("wire fixture read");
        fs::write(&path, update(&wire)).expect("wire fixture mutation");
        self.mutate(index, true, |_| {});
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "one fixture builder keeps causal wire, flow, process, hash, and seal facts aligned"
)]
fn write_pair_manifest(root: &Path, index: usize, author: &Keys, relay_keys: &Keys) {
    let source_manifest = format!(
        "format=fava-pinned-source-v1\nrevision={FIXTURE_FAVA_REVISION}\ntree={FIXTURE_FAVA_BUILD_TREE}\nfile_count=1\ntotal_bytes=27\nfile=100644\t{FIXTURE_FAVA_TREE}\t27\tapps/canary/src/main.rs\n"
    );
    let source_manifest_sha256 = hex::encode(Sha256::digest(source_manifest.as_bytes()));
    let base_pid = 4_000_100 + (index as u64 * 2);
    let port = 49_000 + (u16::try_from(index).expect("fixture index fits u16") * 10);
    let relay_urls = [
        format!("ws://127.0.0.1:{port}"),
        format!("ws://127.0.0.1:{}", port + 1),
    ];
    let relay_signer = relay_keys.public_key().to_hex();
    let group_id = format!("group-{index}");
    let shared = signed_fixture_event(author, 9, &group_id, "shared");
    let unique_events = [
        signed_fixture_event(author, 9, &group_id, &format!("unique-a-{index}")),
        signed_fixture_event(author, 9, &group_id, &format!("unique-b-{index}")),
    ];
    let custom = signed_fixture_event(author, 50_029, &group_id, "custom");
    let metadata_names = [format!("metadata-a-{index}"), format!("metadata-b-{index}")];
    let admin_targets = [
        Keys::generate().public_key().to_hex(),
        Keys::generate().public_key().to_hex(),
    ];
    let ready = [0_u64, 1].map(|child| {
        json!({
            "pid": base_pid + child,
            "endpoint": relay_urls[usize::try_from(child).expect("child index fits usize")].trim_start_matches("ws://"),
            "data_path": format!("/discarded/run-{index}/relay-{child}"),
            "stdout_path": format!("/discarded/run-{index}/relay-{child}.stdout"),
            "stderr_path": format!("/discarded/run-{index}/relay-{child}.stderr"),
            "executable": format!("/controlled/croissant-{index}"),
            "executable_sha256": FIXTURE_CROISSANT_EXECUTABLE,
            "executable_device": 42,
            "executable_inode": base_pid + child,
            "source_checkout": format!("/controlled/source-{index}"),
            "source_head": FIXTURE_CROISSANT_REVISION,
            "scenario_seed_sha256": format!("seed-{index}"),
            "readiness_completed": true,
            "execution_platform": "linux-sealed-memfd-proc-fd",
            "limits": {"log_bytes": 1_048_576, "readiness_ms": 10_000,
                "readiness_stability_ms": 100, "teardown_ms": 5000},
        })
    });
    let teardown = [0_u64, 1].map(|child| {
        json!({
            "pid": base_pid + child,
            "endpoint": relay_urls[usize::try_from(child).expect("child index fits usize")].trim_start_matches("ws://"),
            "completed": true,
            "pid_alive_after": false,
            "port_open_after": false,
            "executable_removed": true,
            "stdout_bytes": 0,
            "stderr_bytes": 0,
        })
    });
    let flow = json!({
        "group_id": group_id,
        "relay_urls": relay_urls,
        "shared_event_id": shared.id.to_hex(),
        "unique_event_ids": [unique_events[0].id.to_hex(), unique_events[1].id.to_hex()],
        "custom_event_id": custom.id.to_hex(),
        "shared_evidence": relay_urls,
        "metadata_names": metadata_names,
        "metadata_authors": [relay_signer.clone(), relay_signer.clone()],
        "admin_targets": admin_targets,
        "admin_authors": [relay_signer.clone(), relay_signer.clone()],
        "write_id": 1,
        "receipt_id": 1,
        "custom_destinations": 2,
        "custom_acknowledged": 2,
        "handoffs": [1, 1],
        "signed_refusals": 3,
        "observation_closed": true,
    });
    let processes = json!({"ready": ready, "teardown": teardown});
    fs::create_dir_all(root.join("children")).expect("children fixture directory");
    fs::create_dir_all(root.join("source")).expect("source fixture directory");
    fs::write(
        root.join("source/fava-canary"),
        b"fixture pinned fava canary\n",
    )
    .expect("pinned Fava fixture");
    fs::create_dir_all(root.join("wire")).expect("wire fixture directory");
    for label in ["a", "b"] {
        fs::create_dir_all(root.join(format!("relays/{label}"))).expect("relay fixture directory");
        fs::write(root.join(format!("relays/{label}/stdout.log")), []).expect("stdout fixture");
        fs::write(root.join(format!("relays/{label}/stderr.log")), []).expect("stderr fixture");
    }
    fs::write(
        root.join("flow.json"),
        serde_json::to_vec_pretty(&flow).expect("flow fixture bytes"),
    )
    .expect("flow fixture");
    fs::write(
        root.join("children/processes.json"),
        serde_json::to_vec_pretty(&processes).expect("process fixture bytes"),
    )
    .expect("process fixture");
    fs::write(
        root.join("source/fava.json"),
        serde_json::to_vec_pretty(&json!({
            "fava_revision": FIXTURE_FAVA_REVISION,
            "fava_source_tree_sha256": FIXTURE_FAVA_TREE,
            "fava_build_revision": FIXTURE_FAVA_REVISION,
            "fava_build_tree": "6666666666666666666666666666666666666666",
            "fava_build_source_tree_sha256": FIXTURE_FAVA_TREE,
            "fava_build_source_manifest_sha256": source_manifest_sha256,
            "fava_build_source_image_sha256": FIXTURE_FAVA_BUILD_IMAGE,
            "fava_build_source_immutable": true,
            "fava_source_clean": true,
            "fava_canary_executable_sha256": FIXTURE_FAVA_EXECUTABLE,
            "fava_canary_executable_bytes": 27,
            "fava_canary_executable_pinned": true,
            "fava_execution_platform": "linux-sealed-memfd-proc-fd",
        }))
        .expect("source fixture bytes"),
    )
    .expect("source fixture");
    fs::write(
        root.join("source/fava-build.json"),
        serde_json::to_vec_pretty(&json!({
            "schema": "fava-pinned-build-v1",
            "fava_revision": FIXTURE_FAVA_REVISION,
            "fava_build_tree": FIXTURE_FAVA_BUILD_TREE,
            "fava_build_source_tree_sha256": FIXTURE_FAVA_TREE,
            "fava_build_source_manifest_sha256": source_manifest_sha256,
            "fava_build_source_image_sha256": FIXTURE_FAVA_BUILD_IMAGE,
            "rust_base_image_sha256": FIXTURE_FAVA_RUST_BASE_IMAGE,
            "build_command_sha256": FIXTURE_FAVA_BUILD_COMMAND,
            "fava_canary_executable_sha256": FIXTURE_FAVA_EXECUTABLE,
            "source_file_count": 1,
            "source_total_bytes": 27,
            "toctou_read_only_attempt": "EROFS",
            "toctou_deliberate_break": "compiled-hostile-bytes",
            "source_root": "/source",
            "target_root": "/target",
            "network": "none",
            "root_filesystem": "read-only",
            "capabilities": "none",
        }))
        .expect("build attestation fixture bytes"),
    )
    .expect("build attestation fixture");
    fs::write(
        root.join("source/fava-build-source.manifest"),
        source_manifest,
    )
    .expect("source manifest fixture");
    for label in 0..2 {
        write_wire_fixture(
            root,
            label,
            author,
            relay_keys,
            &group_id,
            &shared,
            &unique_events[label],
            &custom,
            &metadata_names[label],
            &admin_targets[label],
        );
    }
    let wire_bytes_observed = ["a", "b"]
        .into_iter()
        .map(|label| {
            fs::metadata(root.join(format!("wire/{label}.jsonl")))
                .expect("wire metadata")
                .len()
        })
        .sum::<u64>();
    let mut manifest = json!({
        "run_id": format!("run-{index}"),
        "scenario": SCENARIO,
        "scenario_seed_sha256": format!("seed-{index}"),
        "author_public_key": author.public_key().to_hex(),
        "relay_signer_public_key": relay_signer.clone(),
        "relay_owner_public_keys": [Keys::generate().public_key().to_hex(), Keys::generate().public_key().to_hex()],
        "group_id": group_id,
        "relay_urls": relay_urls,
        "shared_event_id": shared.id.to_hex(),
        "unique_event_ids": [unique_events[0].id.to_hex(), unique_events[1].id.to_hex()],
        "custom_event_id": custom.id.to_hex(),
        "shared_evidence": relay_urls,
        "metadata_names": metadata_names,
        "metadata_authors": [relay_signer.clone(), relay_signer.clone()],
        "admin_targets": admin_targets,
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
        "bounds": {"operation_ms": 30_000, "wire_bytes": 2_097_152, "wire_bytes_observed": wire_bytes_observed,
            "log_bytes": 1_048_576, "readiness_ms": 10_000, "readiness_stability_ms": 100,
            "teardown_ms": 5000},
        "artifact_sha256": artifact_hashes(root).expect("fixture hashes"),
        "fava_revision": FIXTURE_FAVA_REVISION,
        "fava_source_tree_sha256": FIXTURE_FAVA_TREE,
        "fava_build_revision": FIXTURE_FAVA_REVISION,
        "fava_build_tree": "6666666666666666666666666666666666666666",
        "fava_build_source_tree_sha256": FIXTURE_FAVA_TREE,
        "fava_build_source_manifest_sha256": source_manifest_sha256,
        "fava_build_source_image_sha256": FIXTURE_FAVA_BUILD_IMAGE,
        "fava_build_rust_base_image_sha256": FIXTURE_FAVA_RUST_BASE_IMAGE,
        "fava_build_command_sha256": FIXTURE_FAVA_BUILD_COMMAND,
        "fava_build_source_immutable": true,
        "fava_source_clean": true,
        "fava_canary_executable_sha256": FIXTURE_FAVA_EXECUTABLE,
        "fava_canary_executable_bytes": 27,
        "fava_canary_executable_pinned": true,
        "fava_execution_platform": "linux-sealed-memfd-proc-fd",
        "execution_platform": "linux-sealed-memfd-container",
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

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "fixture writes one complete causal relay transcript"
)]
fn write_wire_fixture(
    root: &Path,
    index: usize,
    author: &Keys,
    relay: &Keys,
    group: &str,
    shared: &Event,
    unique: &Event,
    custom: &Event,
    metadata_name: &str,
    admin_target: &str,
) {
    let metadata_seed = EventBuilder::new(Kind::from(9002), "")
        .tags([
            Tag::parse(["name", metadata_name]).expect("name tag"),
            Tag::parse([
                "about",
                if index == 0 {
                    "A-only metadata"
                } else {
                    "B-only metadata"
                },
            ])
            .expect("about tag"),
            Tag::parse(["h", group]).expect("h tag"),
        ])
        .custom_created_at(Timestamp::from(9_003))
        .finalize(author)
        .expect("metadata command");
    let admin_seed = EventBuilder::new(Kind::from(9000), "")
        .tags([
            Tag::parse(["p", admin_target, "admin"]).expect("admin tag"),
            Tag::parse(["h", group]).expect("h tag"),
        ])
        .custom_created_at(Timestamp::from(9_001))
        .finalize(author)
        .expect("admin command");
    let bootstrap = signed_fixture_event(author, 9007, group, "controlled group bootstrap");
    let metadata = EventBuilder::new(Kind::from(39000), "")
        .tags([
            Tag::parse(["d", group]).expect("d tag"),
            Tag::parse(["name", metadata_name]).expect("name tag"),
        ])
        .custom_created_at(Timestamp::from(39_001))
        .finalize(relay)
        .expect("metadata fixture event");
    let admin = EventBuilder::new(Kind::from(39001), "")
        .tags([
            Tag::parse(["d", group]).expect("d tag"),
            Tag::parse(["p", admin_target, "admin"]).expect("admin tag"),
        ])
        .custom_created_at(Timestamp::from(39_002))
        .finalize(relay)
        .expect("admin fixture event");
    let content_subscription = format!("content-{index}");
    let records_subscription = format!("records-{index}");
    let bootstrap_subscription = format!("bootstrap-{index}");
    let exchanges = [
        (1, "client_to_relay", json!(["EVENT", bootstrap])),
        (
            1,
            "relay_to_client",
            json!(["OK", bootstrap.id.to_hex(), true, ""]),
        ),
        (
            2,
            "client_to_relay",
            json!(["REQ", bootstrap_subscription, {"ids": [bootstrap.id.to_hex()]}]),
        ),
        (
            2,
            "relay_to_client",
            json!(["EVENT", bootstrap_subscription, bootstrap]),
        ),
        (
            2,
            "relay_to_client",
            json!(["EOSE", bootstrap_subscription]),
        ),
        (
            2,
            "client_to_relay",
            json!(["CLOSE", bootstrap_subscription]),
        ),
        (3, "client_to_relay", json!(["EVENT", metadata_seed])),
        (
            3,
            "relay_to_client",
            json!(["OK", metadata_seed.id.to_hex(), true, ""]),
        ),
        (4, "client_to_relay", json!(["EVENT", admin_seed])),
        (
            4,
            "relay_to_client",
            json!(["OK", admin_seed.id.to_hex(), true, ""]),
        ),
        (5, "client_to_relay", json!(["EVENT", shared])),
        (
            5,
            "relay_to_client",
            json!(["OK", shared.id.to_hex(), true, ""]),
        ),
        (6, "client_to_relay", json!(["EVENT", unique])),
        (
            6,
            "relay_to_client",
            json!(["OK", unique.id.to_hex(), true, ""]),
        ),
        (9, "client_to_relay", json!(["EVENT", custom])),
        (
            9,
            "relay_to_client",
            json!(["OK", custom.id.to_hex(), true, ""]),
        ),
        (
            7,
            "client_to_relay",
            json!(["REQ", content_subscription, {"kinds": [9], "limit": 16, "#h": [group]}]),
        ),
        (
            7,
            "relay_to_client",
            json!(["EVENT", content_subscription, unique]),
        ),
        (
            7,
            "relay_to_client",
            json!(["EVENT", content_subscription, shared]),
        ),
        (7, "relay_to_client", json!(["EOSE", content_subscription])),
        (7, "client_to_relay", json!(["CLOSE", content_subscription])),
        (
            8,
            "client_to_relay",
            json!(["REQ", records_subscription, {"kinds": [39000,39001,39002,39003,39004,39005], "limit": 4096, "#d": [group]}]),
        ),
        (
            8,
            "relay_to_client",
            json!(["EVENT", records_subscription, metadata]),
        ),
        (
            8,
            "relay_to_client",
            json!(["EVENT", records_subscription, admin]),
        ),
        (8, "relay_to_client", json!(["EOSE", records_subscription])),
        (8, "client_to_relay", json!(["CLOSE", records_subscription])),
    ];
    let lines = exchanges
        .into_iter()
        .enumerate()
        .map(|(sequence, (connection, direction, payload))| {
            wire_line(
                u64::try_from(sequence + 1).expect("sequence fits"),
                connection,
                direction,
                &payload,
            )
        })
        .collect::<Vec<_>>();
    fs::write(
        root.join(format!("wire/{}.jsonl", if index == 0 { "a" } else { "b" })),
        format!("{}\n", lines.join("\n")),
    )
    .expect("wire fixture");
}
