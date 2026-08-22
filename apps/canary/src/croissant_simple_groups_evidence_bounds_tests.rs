use std::fs::{self, File};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::croissant_simple_groups_evidence::verify_croissant_simple_groups_pair;

const MAX_FILE_BYTES: u64 = 2_097_152;
const MAX_AGGREGATE_BYTES: u64 = 8_388_608;
const MAX_MANIFEST_BYTES: u64 = 262_144;

fn refusal(root: &Path) -> String {
    verify_croissant_simple_groups_pair(root)
        .expect_err("hostile evidence must be refused")
        .to_string()
}

fn run_root(temporary: &TempDir) -> PathBuf {
    let run = temporary.path().join("run-a");
    fs::create_dir(&run).expect("run root");
    run
}

#[test]
fn evidence_walker_refuses_symlink_escape_and_cycle() {
    let escape = TempDir::new().expect("escape fixture");
    let outside_root = TempDir::new().expect("outside fixture");
    let outside = outside_root.path().join("outside");
    fs::write(&outside, b"outside").expect("outside file");
    let run = run_root(&escape);
    symlink(&outside, run.join("escape")).expect("escape symlink");
    let error = refusal(escape.path());
    assert!(error.contains("symlink"), "unexpected refusal: {error}");

    let cycle = TempDir::new().expect("cycle fixture");
    let run = run_root(&cycle);
    symlink(&run, run.join("cycle")).expect("cycle symlink");
    let error = refusal(cycle.path());
    assert!(error.contains("symlink"), "unexpected refusal: {error}");
}

#[test]
fn evidence_walker_refuses_depth_and_count_plus_one() {
    let depth = TempDir::new().expect("depth fixture");
    let mut directory = run_root(&depth);
    for index in 0..9 {
        directory = directory.join(format!("d{index}"));
        fs::create_dir(&directory).expect("nested directory");
    }
    fs::write(directory.join("leaf"), b"leaf").expect("deep leaf");
    assert!(refusal(depth.path()).contains("depth"));

    let count = TempDir::new().expect("count fixture");
    let run = run_root(&count);
    for index in 0..129 {
        fs::write(run.join(format!("file-{index}")), []).expect("counted file");
    }
    assert!(refusal(count.path()).contains("file count"));
}

#[test]
fn evidence_walker_refuses_file_and_aggregate_size_plus_one() {
    let file = TempDir::new().expect("file-size fixture");
    let run = run_root(&file);
    File::create(run.join("oversized"))
        .expect("oversized file")
        .set_len(MAX_FILE_BYTES + 1)
        .expect("size file");
    assert!(refusal(file.path()).contains("file bytes"));

    let aggregate = TempDir::new().expect("aggregate fixture");
    let run = run_root(&aggregate);
    for index in 0..5 {
        File::create(run.join(format!("large-{index}")))
            .expect("aggregate file")
            .set_len(MAX_AGGREGATE_BYTES / 4)
            .expect("size aggregate file");
    }
    assert!(refusal(aggregate.path()).contains("aggregate bytes"));
}

#[test]
fn evidence_verifier_refuses_manifest_size_plus_one_before_json_decode() {
    let temporary = TempDir::new().expect("manifest fixture");
    let run = run_root(&temporary);
    File::create(run.join("manifest.json"))
        .expect("manifest file")
        .set_len(MAX_MANIFEST_BYTES + 1)
        .expect("size manifest");
    let other = temporary.path().join("run-b");
    fs::create_dir(&other).expect("second run root");
    fs::write(other.join("manifest.json"), b"{}").expect("second manifest");
    let error = refusal(temporary.path());
    assert!(
        error.contains("manifest bytes"),
        "unexpected refusal: {error}"
    );
}
