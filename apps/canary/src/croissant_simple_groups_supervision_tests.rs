use std::fs;
use std::future::pending;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::time::{Instant, timeout};

use super::croissant::{
    CroissantLimits, CroissantReadyFact, CroissantSupervisor, process_is_alive,
};
use super::croissant_simple_groups::supervise_owned_pair;
use super::{CanaryError, CanaryResult, repository_root};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn normal_and_flow_failure_reap_both_exact_children() {
    let fixture = PairFixture::new();
    let completion = supervise_owned_pair(fixture.supervisors(), |_| async { Ok(()) })
        .await
        .expect("both exact children complete");
    assert_pair_absent(&completion.ready).await;

    let error_fixture = PairFixture::new();
    let failure = supervise_owned_pair(error_fixture.supervisors(), |_| async {
        Err::<(), _>(CanaryError::new("injected pair-flow failure"))
    })
    .await
    .expect_err("flow failure remains attributed");
    assert_eq!(failure.ready.len(), 2);
    assert_eq!(failure.teardown.len(), 2);
    assert_pair_absent(
        failure
            .ready
            .as_slice()
            .try_into()
            .expect("two exact readiness facts"),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_startup_failure_reaps_first_exact_child() {
    let fixture = PairFixture::new();
    let failure = supervise_owned_pair(fixture.supervisors_with_failing_b(), |_| async { Ok(()) })
        .await
        .expect_err("second startup failure remains attributed");
    assert_eq!(failure.ready.len(), 1);
    assert_eq!(failure.teardown.len(), 1);
    let ready = &failure.ready[0];
    let teardown = failure.teardown[0]
        .as_ref()
        .expect("first exact child teardown completes");
    assert_eq!(ready.pid, teardown.pid);
    assert!(!process_is_alive(ready.pid));
    assert!(!teardown.port_open_after);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aborted_pair_supervisor_reaps_only_its_exact_children() {
    let fixture = PairFixture::new();
    let unrelated_supervisor = fixture.unrelated_supervisor();
    let unrelated_stderr = unrelated_supervisor.stderr_path().to_owned();
    let unrelated = unrelated_supervisor.start().await.unwrap_or_else(|error| {
        panic!(
            "unrelated witness starts: {error}; stderr={}",
            fs::read_to_string(unrelated_stderr).unwrap_or_default()
        )
    });
    let unrelated_ready = unrelated.ready_fact();
    let (ready_sender, ready_receiver) = oneshot::channel();
    let task = tokio::spawn(supervise_owned_pair(
        fixture.supervisors(),
        move |ready| async move {
            ready_sender
                .send(ready)
                .map_err(|_| CanaryError::new("readiness witness was dropped"))?;
            pending::<CanaryResult<()>>().await
        },
    ));
    let ready = timeout(Duration::from_secs(5), ready_receiver)
        .await
        .expect("both exact children reach flow before deadline")
        .expect("readiness witness remains available");
    assert_pair_live(&ready).await;

    task.abort();
    assert!(
        task.await
            .expect_err("supervisor task was aborted")
            .is_cancelled()
    );
    assert_pair_absent(&ready).await;
    assert!(process_is_alive(unrelated_ready.pid));
    assert!(TcpStream::connect(unrelated_ready.endpoint).await.is_ok());

    let unrelated_teardown = unrelated.stop().await.expect("unrelated witness cleanup");
    assert_eq!(unrelated_teardown.pid, unrelated_ready.pid);
    assert!(!unrelated_teardown.pid_alive_after);
    assert!(!unrelated_teardown.port_open_after);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn panicking_pair_flow_is_attributed_after_both_children_are_reaped() {
    let fixture = PairFixture::new();
    let failure = supervise_owned_pair(fixture.supervisors(), |_| async {
        panic!("controlled pair-flow panic");
        #[allow(unreachable_code)]
        Ok::<(), CanaryError>(())
    })
    .await
    .expect_err("controlled panic becomes an attributed pair failure");

    assert_eq!(failure.ready.len(), 2);
    assert_eq!(failure.teardown.len(), 2);
    assert!(
        failure
            .flow_error
            .as_deref()
            .is_some_and(|error| error.contains("controlled pair-flow panic"))
    );
    for (ready, teardown) in failure.ready.iter().zip(&failure.teardown) {
        let teardown = teardown.as_ref().expect("exact child teardown succeeds");
        assert_eq!(ready.pid, teardown.pid);
        assert_eq!(ready.endpoint, teardown.endpoint);
        assert_ne!(ready.pid, 75_649, "forbidden unowned PID was touched");
        assert!(teardown.completed);
        assert!(!teardown.pid_alive_after);
        assert!(!process_is_alive(ready.pid));
        assert!(!teardown.port_open_after);
    }
}

async fn assert_pair_live(ready: &[CroissantReadyFact; 2]) {
    assert_ne!(ready[0].pid, ready[1].pid);
    assert_ne!(ready[0].endpoint, ready[1].endpoint);
    for child in ready {
        assert_ne!(child.pid, 75_649, "forbidden unowned PID was touched");
        assert!(process_is_alive(child.pid));
        assert!(TcpStream::connect(child.endpoint).await.is_ok());
    }
}

async fn assert_pair_absent(ready: &[CroissantReadyFact; 2]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let absent = ready.iter().all(|child| !process_is_alive(child.pid));
        let closed = ports_are_closed(ready).await;
        if absent && closed {
            for child in ready {
                assert!(
                    child.data_path.join("term-observed").is_file(),
                    "exact child did not observe the owner's bounded TERM teardown: {child:?}"
                );
            }
            return;
        }
        assert!(
            Instant::now() < deadline,
            "owned children were not reaped: {ready:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn ports_are_closed(ready: &[CroissantReadyFact; 2]) -> bool {
    for child in ready {
        if TcpStream::connect(child.endpoint).await.is_ok() {
            return false;
        }
    }
    true
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
        [self.supervisor("relay-a"), self.supervisor("relay-b")]
    }

    fn unrelated_supervisor(&self) -> CroissantSupervisor {
        self.supervisor("unrelated-relay")
    }

    fn supervisors_with_failing_b(&self) -> [CroissantSupervisor; 2] {
        let source = repository_root().expect("source checkout");
        [
            self.supervisor("startup-relay-a"),
            CroissantSupervisor::prepare(
                &failing_executable(self.temporary.path()),
                &source,
                &self.temporary.path().join("startup-relay-b"),
                &owner("startup-relay-b"),
                &seed_hash(b"startup-relay-b"),
                CroissantLimits::test(),
            )
            .expect("failing child supervisor"),
        ]
    }

    fn supervisor(&self, label: &str) -> CroissantSupervisor {
        let source = repository_root().expect("source checkout");
        let root = self.temporary.path().join(label);
        CroissantSupervisor::prepare(
            &self.binary,
            &source,
            &root,
            &owner(label),
            &seed_hash(label.as_bytes()),
            CroissantLimits::test(),
        )
        .expect("exact child supervisor")
    }
}

fn executable(root: &Path) -> PathBuf {
    let path = root.join("controlled-croissant-supervision-fixture");
    fs::write(
        &path,
        concat!(
            "#!/usr/bin/python3\n",
            "import os, signal, socket, sys, time\n",
            "def stop(_signal, _frame):\n",
            "    open(os.path.join(os.environ['DATAPATH'], 'term-observed'), 'w').close()\n",
            "    sys.exit(0)\n",
            "signal.signal(signal.SIGTERM, stop)\n",
            "s = socket.socket()\n",
            "s.bind(('127.0.0.1', int(os.environ['PORT'])))\n",
            "s.listen()\n",
            "time.sleep(30)\n",
        ),
    )
    .expect("fixture executable");
    let mut permissions = fs::metadata(&path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("fixture permissions");
    path
}

fn failing_executable(root: &Path) -> PathBuf {
    let path = root.join("failing-croissant-supervision-fixture");
    fs::write(&path, "#!/bin/sh\nexit 71\n").expect("failing fixture executable");
    let mut permissions = fs::metadata(&path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("fixture permissions");
    path
}

fn owner(label: &str) -> String {
    hex::encode(Sha256::digest(format!("owner:{label}").as_bytes()))
}

fn seed_hash(seed: &[u8]) -> String {
    hex::encode(Sha256::digest(seed))
}
