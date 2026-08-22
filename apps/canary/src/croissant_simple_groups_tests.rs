use std::fs;
use std::path::{Path, PathBuf};

use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;
use nostr::types::Timestamp;
use serde_json::{Value, json};
use tempfile::TempDir;

use super::croissant_simple_groups_evidence::{SCENARIO, verify_croissant_simple_groups_pair};
use super::croissant_simple_groups_evidence_support::{
    SECRET_SCAN_CLASSES, artifact_hashes, artifact_seal,
};
include!("croissant_simple_groups_tests/public_flow.rs");

#[test]
fn pair_verifier_rejects_unsafe_evidence() {
    let control = PairEvidenceFixture::new();
    verify_croissant_simple_groups_pair(control.root()).expect("narrow safe pair verifies");

    for case in [
        UnsafePairCase::PersistentParentSecret,
        UnsafePairCase::IncompleteCleanup,
        UnsafePairCase::UnsignedClaim,
        UnsafePairCase::ReusedIdentity,
        UnsafePairCase::ReusedUniqueIdentity,
        UnsafePairCase::CrossRunData,
        UnsafePairCase::CrossRunUniqueData,
        UnsafePairCase::ExtraManifest,
        UnsafePairCase::MissingManifest,
        UnsafePairCase::StagingResidue,
        UnsafePairCase::UnderivedFlowClaim,
        UnsafePairCase::UnderivedProcessClaim,
        UnsafePairCase::MissingExactClose,
        UnsafePairCase::ExtraSignedHandoff,
        UnsafePairCase::RetainedSecretMarker,
    ] {
        let fixture = PairEvidenceFixture::new();
        fixture.apply(case);
        assert!(
            verify_croissant_simple_groups_pair(fixture.root()).is_err(),
            "pair verifier accepted unsafe fixture {case:?}"
        );
    }
}

#[test]
fn pair_verifier_treats_shared_evidence_as_unordered_hosts() {
    let fixture = PairEvidenceFixture::new();
    fixture.reverse_shared_evidence(0);
    verify_croissant_simple_groups_pair(fixture.root())
        .expect("shared host evidence has no canonical order");
}

#[derive(Clone, Copy, Debug)]
enum UnsafePairCase {
    PersistentParentSecret,
    IncompleteCleanup,
    UnsignedClaim,
    ReusedIdentity,
    ReusedUniqueIdentity,
    CrossRunData,
    CrossRunUniqueData,
    ExtraManifest,
    MissingManifest,
    StagingResidue,
    UnderivedFlowClaim,
    UnderivedProcessClaim,
    MissingExactClose,
    ExtraSignedHandoff,
    RetainedSecretMarker,
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

    fn reverse_shared_evidence(&self, index: usize) {
        let flow_path = self.roots[index].join("flow.json");
        let mut flow: Value =
            serde_json::from_slice(&fs::read(&flow_path).expect("flow read")).expect("flow json");
        flow["shared_evidence"]
            .as_array_mut()
            .expect("shared evidence array")
            .reverse();
        fs::write(
            &flow_path,
            serde_json::to_vec_pretty(&flow).expect("flow bytes"),
        )
        .expect("flow rewrite");
        self.mutate(index, true, |manifest| {
            manifest["shared_evidence"]
                .as_array_mut()
                .expect("shared evidence array")
                .reverse();
        });
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

#[allow(
    clippy::too_many_lines,
    reason = "one fixture builder keeps causal wire, flow, process, hash, and seal facts aligned"
)]
fn write_pair_manifest(root: &Path, index: usize, author: &Keys) {
    let base_pid = 4_000_100 + (index as u64 * 2);
    let port = 49_000 + (u16::try_from(index).expect("fixture index fits u16") * 10);
    let relay_urls = [
        format!("ws://127.0.0.1:{port}"),
        format!("ws://127.0.0.1:{}", port + 1),
    ];
    let relay_keys = Keys::generate();
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
    fs::create_dir_all(root.join("wire")).expect("wire fixture directory");
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
    for label in 0..2 {
        write_wire_fixture(
            root,
            label,
            author,
            &relay_keys,
            &group_id,
            &shared,
            &unique_events[label],
            &custom,
            &metadata_names[label],
            &admin_targets[label],
        );
    }
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

fn signed_fixture_event(keys: &Keys, kind: u16, group: &str, content: &str) -> Event {
    EventBuilder::new(Kind::from(kind), content)
        .tags([Tag::parse(["h", group]).expect("h tag")])
        .custom_created_at(Timestamp::from(u64::from(kind) + 1))
        .finalize(keys)
        .expect("signed fixture event")
}

#[allow(
    clippy::too_many_arguments,
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
    let metadata_seed = signed_fixture_event(author, 9002, group, metadata_name);
    let admin_seed = signed_fixture_event(author, 9000, group, admin_target);
    let bootstrap = signed_fixture_event(author, 9007, group, "bootstrap");
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
    let payloads = [
        json!(["EVENT", bootstrap]),
        json!(["EVENT", metadata_seed]),
        json!(["EVENT", admin_seed]),
        json!(["EVENT", shared]),
        json!(["EVENT", unique]),
        json!(["REQ", content_subscription, {"kinds": [9], "limit": 16, "#h": [group]}]),
        json!(["REQ", records_subscription, {"kinds": [39000,39001,39002,39003,39004,39005], "limit": 4096, "#d": [group]}]),
        json!(["EVENT", custom]),
        json!(["CLOSE", content_subscription]),
        json!(["CLOSE", records_subscription]),
    ];
    let responses = [
        json!(["EVENT", content_subscription, unique]),
        json!(["EVENT", content_subscription, shared]),
        json!(["EVENT", records_subscription, metadata]),
        json!(["EVENT", records_subscription, admin]),
        json!(["OK", custom.id.to_hex(), true, ""]),
    ];
    let mut lines = Vec::new();
    for payload in payloads {
        lines.push(wire_line("client_to_relay", &payload));
    }
    for payload in responses {
        lines.push(wire_line("relay_to_client", &payload));
    }
    fs::write(
        root.join(format!("wire/{}.jsonl", if index == 0 { "a" } else { "b" })),
        format!("{}\n", lines.join("\n")),
    )
    .expect("wire fixture");
}

fn wire_line(direction: &str, payload: &Value) -> String {
    serde_json::to_string(&json!({
        "direction": direction,
        "frame_type": "text",
        "payload": serde_json::to_string(&payload).expect("payload json"),
    }))
    .expect("wire line")
}

fn read_manifest(root: &Path) -> Value {
    serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("manifest read"))
        .expect("manifest json")
}
