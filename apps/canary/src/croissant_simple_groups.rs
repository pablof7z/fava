//! Controlled two-Croissant proof for the public multi-relay simple-groups flow.

use std::future::Future;
use std::path::PathBuf;

use crate::CanaryResult;
use crate::croissant::{CroissantReadyFact, CroissantSupervisor, CroissantTeardown};

/// Process-memory input for one controlled two-relay simple-groups proof.
#[derive(Clone, Debug)]
pub struct CroissantSimpleGroupsOptions {
    /// Croissant executable launched twice without modifying its checkout.
    pub relay_binary: PathBuf,
    /// Croissant source checkout used for exact source-revision evidence.
    pub source_checkout: PathBuf,
    /// Disposable identity seed, never retained outside process memory.
    pub scenario_seed: String,
    /// Parent directory for one fresh durable evidence bundle.
    pub runs_directory: PathBuf,
}

/// Durable location produced by one completed controlled run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CroissantSimpleGroupsOutcome {
    /// Fresh run directory containing the completed manifest and artifacts.
    pub run_directory: PathBuf,
}

#[derive(Debug)]
pub(crate) struct OwnedPairCompletion<T> {
    pub(crate) ready: [CroissantReadyFact; 2],
    pub(crate) teardown: [CroissantTeardown; 2],
    pub(crate) flow: T,
}

#[derive(Debug)]
pub(crate) struct OwnedPairFailure {
    pub(crate) ready: Vec<CroissantReadyFact>,
    pub(crate) teardown: Vec<Result<CroissantTeardown, String>>,
    pub(crate) flow_error: Option<String>,
    pub(crate) startup_error: Option<String>,
}

impl std::fmt::Display for OwnedPairFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "two-child Croissant run failed: startup={:?}; flow={:?}; cleanup={:?}",
            self.startup_error, self.flow_error, self.teardown
        )
    }
}

impl std::error::Error for OwnedPairFailure {}

pub(crate) async fn supervise_owned_pair<T, F, Fut>(
    supervisors: [CroissantSupervisor; 2],
    flow: F,
) -> Result<OwnedPairCompletion<T>, OwnedPairFailure>
where
    F: FnOnce([CroissantReadyFact; 2]) -> Fut,
    Fut: Future<Output = CanaryResult<T>>,
{
    let [supervisor_a, supervisor_b] = supervisors;
    let process_a = supervisor_a
        .start()
        .await
        .map_err(|error| OwnedPairFailure {
            ready: Vec::new(),
            teardown: Vec::new(),
            flow_error: None,
            startup_error: Some(error.to_string()),
        })?;
    let ready_a = process_a.ready_fact();
    let process_b = match supervisor_b.start().await {
        Ok(process) => process,
        Err(error) => {
            let cleanup_a = process_a
                .stop()
                .await
                .map_err(|cleanup| cleanup.to_string());
            return Err(OwnedPairFailure {
                ready: vec![ready_a],
                teardown: vec![cleanup_a],
                flow_error: None,
                startup_error: Some(error.to_string()),
            });
        }
    };
    let ready_b = process_b.ready_fact();
    let ready = [ready_a, ready_b];
    let flow = flow(ready.clone()).await;
    let cleanup_a = process_a.stop().await.map_err(|error| error.to_string());

    let cleanup_b = process_b.stop().await.map_err(|error| error.to_string());
    match (flow, cleanup_a, cleanup_b) {
        (Ok(flow), Ok(teardown_a), Ok(teardown_b)) => Ok(OwnedPairCompletion {
            ready,
            teardown: [teardown_a, teardown_b],
            flow,
        }),
        (flow, cleanup_a, cleanup_b) => Err(OwnedPairFailure {
            ready: ready.into(),
            teardown: vec![cleanup_a, cleanup_b],
            flow_error: flow.err().map(|error| error.to_string()),
            startup_error: None,
        }),
    }
}
