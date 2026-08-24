use std::process::Command;

use super::croissant::{CroissantLimits, process_is_alive};
use super::croissant_simple_groups::{
    CroissantSimpleGroupsOptions, prepare_owned_supervisors, supervise_owned_pair,
};
use super::croissant_simple_groups_flow::execute_public_flow;

const FIXTURE_FAVA_REVISION: &str = "1111111111111111111111111111111111111111";
const FIXTURE_FAVA_TREE: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const FIXTURE_FAVA_BUILD_TREE: &str = "6666666666666666666666666666666666666666";
const FIXTURE_FAVA_BUILD_IMAGE: &str =
    "7777777777777777777777777777777777777777777777777777777777777777";
const FIXTURE_FAVA_RUST_BASE_IMAGE: &str =
    "8888888888888888888888888888888888888888888888888888888888888888";
const FIXTURE_FAVA_BUILD_COMMAND: &str =
    "8e010e7b68d708e96ebc25f34935b42d8e6198436a65cf41e27a60c7765bae08";
const FIXTURE_FAVA_SOURCE_MANIFEST: &str =
    "73b83ce204d2d4c69ec95a8750b0c2f25483f18d6235f357d7d50a950d9dde96";
const FIXTURE_FAVA_EXECUTABLE: &str = "dbe3d43cfad0cc9a73e99695aa9df9ba54a475ee38f6111b3dead5e55e08be78";
const FIXTURE_FAVA_SUBJECT_IMAGE: &str =
    "9999999999999999999999999999999999999999999999999999999999999999";
const FIXTURE_CROISSANT_REVISION: &str = "3333333333333333333333333333333333333333";
const FIXTURE_CROISSANT_EXECUTABLE: &str = "4444444444444444444444444444444444444444444444444444444444444444";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn croissant_simple_groups_public_flow() {
    let _fixture_guard = crate::environment::croissant_fixture_guard().await;
    let temporary = TempDir::new().expect("public-flow fixture root");
    let source = PathBuf::from("/Users/pablo/Work/croissant");
    let binary = build_croissant(&source, temporary.path());
    let seed = "controlled-simple-groups-public-flow";
    let options = CroissantSimpleGroupsOptions {
        relay_binary: binary,
        source_checkout: source,
        fava_build_attestation: temporary.path().join("unused-build-attestation.json"),
        fava_build_source_manifest: temporary.path().join("unused-build-source.manifest"),
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
    assert!(!facts.simple_group_id.is_empty());
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
        assert!(teardown.executable_removed);
        assert!(!process_is_alive(teardown.pid));
    }
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
