use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::net::TcpStream;

use crate::croissant_test_support::committed_source_checkout;
use super::{CroissantError, CroissantLimits, CroissantSupervisor, process_is_alive};

fn executable(directory: &Path, name: &str, body: &str) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).expect("write fake relay");
    let mut permissions = fs::metadata(&path).expect("fake metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&path, permissions).expect("make fake executable");
    path
}

fn owner() -> &'static str {
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
}

fn seed_hash(seed: &[u8]) -> String {
    hex::encode(Sha256::digest(seed))
}

#[tokio::test]
async fn supervisor_attributes_exit_before_readiness() {
    let fixture = TempDir::new().expect("fixture");
    let binary = executable(fixture.path(), "early-exit", "echo early >&2\nexit 23");
    let supervisor = CroissantSupervisor::prepare(
        &binary,
        &committed_source_checkout(fixture.path()),
        &fixture.path().join("run"),
        owner(),
        &seed_hash(b"early-exit-seed"),
        CroissantLimits::test(),
    )
    .expect("prepare");
    let error = supervisor.start().await.expect_err("early exit must fail");
    assert!(matches!(error, CroissantError::EarlyExit { .. }));
    assert!(error.to_string().contains("before readiness"));
}

#[tokio::test]
async fn supervisor_refuses_port_that_opens_then_exits_during_readiness() {
    let fixture = TempDir::new().expect("fixture");
    let binary = executable(
        fixture.path(),
        "open-then-exit",
        "exec python3 -c 'import os,socket; s=socket.socket(); s.bind((\"127.0.0.1\",int(os.environ[\"PORT\"]))); s.listen(); c,_=s.accept(); c.close(); s.close()'",
    );
    let supervisor = CroissantSupervisor::prepare(
        &binary,
        &committed_source_checkout(fixture.path()),
        &fixture.path().join("run"),
        owner(),
        &seed_hash(b"open-then-exit-seed"),
        CroissantLimits::test(),
    )
    .expect("prepare");
    let error = supervisor
        .start()
        .await
        .expect_err("transient port acceptance must not publish readiness");
    assert!(matches!(error, CroissantError::EarlyExit { .. }));
}

#[tokio::test]
async fn supervisor_refuses_log_overflow_without_echoing_secret_environment() {
    let fixture = TempDir::new().expect("fixture");
    let sentinel = "sentinel-private-seed-never-retain";
    let binary = executable(
        fixture.path(),
        "overflow",
        "printf '%s' \"${FAVA_TEST_SECRET-unset}\"\nyes x | head -c 16384",
    );
    let supervisor = CroissantSupervisor::prepare(
        &binary,
        &committed_source_checkout(fixture.path()),
        &fixture.path().join("run"),
        owner(),
        &seed_hash(sentinel.as_bytes()),
        CroissantLimits::test(),
    )
    .expect("prepare");
    let error = supervisor.start().await.expect_err("overflow must fail");
    assert!(matches!(error, CroissantError::LogOverflow { .. }));
    assert!(!error.to_string().contains(sentinel));
    for path in [supervisor.stdout_path(), supervisor.stderr_path()] {
        let bytes = fs::read(path).expect("bounded log exists");
        assert!(bytes.len() <= CroissantLimits::test().log_bytes);
        assert!(
            !bytes
                .windows(sentinel.len())
                .any(|window| window == sentinel.as_bytes())
        );
    }
}

#[tokio::test]
async fn supervisor_records_provenance_and_completes_bounded_teardown() {
    let fixture = TempDir::new().expect("fixture");
    let binary = executable(
        fixture.path(),
        "ready",
        "exec python3 -c 'import os,socket,time; s=socket.socket(); s.bind((\"127.0.0.1\",int(os.environ[\"PORT\"]))); s.listen(); s.accept(); time.sleep(30)'",
    );
    let expected_binary_hash = hex::encode(Sha256::digest(fs::read(&binary).expect("binary")));
    let supervisor = CroissantSupervisor::prepare(
        &binary,
        &committed_source_checkout(fixture.path()),
        &fixture.path().join("run"),
        owner(),
        &seed_hash(b"bounded-teardown-seed"),
        CroissantLimits::test(),
    )
    .expect("prepare");
    let process = supervisor.start().await.expect("ready process");
    let ready = process.ready_fact();
    assert_eq!(ready.executable_sha256, expected_binary_hash);
    assert_eq!(
        ready.scenario_seed_sha256,
        seed_hash(b"bounded-teardown-seed")
    );
    assert_eq!(ready.endpoint.ip().to_string(), "127.0.0.1");
    assert!(process_is_alive(ready.pid));
    assert!(TcpStream::connect(ready.endpoint).await.is_ok());
    let teardown = process.stop().await.expect("bounded stop");
    assert!(teardown.completed);
    assert!(!process_is_alive(teardown.pid));
    assert!(
        tokio::time::timeout(
            Duration::from_millis(100),
            TcpStream::connect(teardown.endpoint)
        )
        .await
        .expect("connection attempt bounded")
        .is_err()
    );
    assert!(teardown.stdout_bytes <= CroissantLimits::test().log_bytes);
    assert!(teardown.stderr_bytes <= CroissantLimits::test().log_bytes);
}

#[tokio::test]
async fn dropping_a_live_process_starts_kill_on_drop() {
    let fixture = TempDir::new().expect("fixture");
    let binary = executable(
        fixture.path(),
        "ready-drop",
        "exec python3 -c 'import os,socket,time; s=socket.socket(); s.bind((\"127.0.0.1\",int(os.environ[\"PORT\"]))); s.listen(); time.sleep(30)'",
    );
    let supervisor = CroissantSupervisor::prepare(
        &binary,
        &committed_source_checkout(fixture.path()),
        &fixture.path().join("run"),
        owner(),
        &seed_hash(b"kill-on-drop-seed"),
        CroissantLimits::test(),
    )
    .expect("prepare");
    let process = supervisor.start().await.expect("ready process");
    let pid = process.ready_fact().pid;
    drop(process);
    tokio::time::timeout(Duration::from_secs(2), async {
        while process_is_alive(pid) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("kill-on-drop reaps child");
}

#[tokio::test]
async fn prepared_executable_is_immune_to_caller_path_replacement() {
    let fixture = TempDir::new().expect("fixture");
    let marker = fixture.path().join("replacement-executed");
    let binary = executable(
        fixture.path(),
        "replaceable",
        "exec python3 -c 'import os,socket,time; s=socket.socket(); s.bind((\"127.0.0.1\",int(os.environ[\"PORT\"]))); s.listen(); time.sleep(30)'",
    );
    let original_hash = hex::encode(Sha256::digest(fs::read(&binary).expect("original")));
    let supervisor = CroissantSupervisor::prepare(
        &binary,
        &committed_source_checkout(fixture.path()),
        &fixture.path().join("run"),
        owner(),
        &seed_hash(b"replacement-seed"),
        CroissantLimits::test(),
    )
    .expect("prepare stages exact bytes");
    fs::write(&binary, format!("#!/bin/sh\ntouch {}\nexit 72\n", marker.display()))
        .expect("replace caller path");
    let process = supervisor
        .start()
        .await
        .expect("staged original reaches readiness");
    let ready = process.ready_fact();
    assert_ne!(ready.executable, binary);
    assert_eq!(ready.executable_sha256, original_hash);
    assert_eq!(
        fs::metadata(&ready.executable).unwrap().permissions().mode() & 0o222,
        0
    );
    assert!(!marker.exists(), "replacement bytes executed");
    process.stop().await.expect("staged child cleanup");
    drop(supervisor);
    assert!(!ready.executable.exists(), "staged executable was retained");
}
