//! Bounded ownership of external canary processes.

use std::os::unix::process::CommandExt;
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::{CanaryError, CanaryResult};

const OUTPUT_CAPACITY: usize = 1_048_576;
const CLEANUP_CAPACITY: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(super) struct OwnedOutput {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) owner_reaped: bool,
    pub(super) process_group_clean: bool,
}

struct RunObservation {
    status: Option<ExitStatus>,
    owner_reaped: bool,
    captured: Option<(Vec<u8>, Vec<u8>)>,
    readers_joined: bool,
    exceeded_bound: bool,
    primary_error: Option<CanaryError>,
}

pub(super) async fn run_owned(
    mut command: Command,
    deadline: Duration,
) -> CanaryResult<OwnedOutput> {
    command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = tokio::process::Command::from(command).spawn()?;
    let pid = child
        .id()
        .ok_or_else(|| CanaryError::new("spawned child has no process identifier"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CanaryError::new("owned child stdout was not captured"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CanaryError::new("owned child stderr was not captured"))?;
    let mut readers = tokio::spawn(async move {
        let (stdout, stderr) = tokio::join!(read_bounded(stdout), read_bounded(stderr));
        Ok((stdout?, stderr?))
    });

    let absolute_deadline = Instant::now() + deadline;
    let mut observation = observe_run(&mut child, &mut readers, absolute_deadline).await;

    let cleanup_deadline = Instant::now() + CLEANUP_CAPACITY;
    let mut cleanup_failures = Vec::new();
    let process_group_clean = match clean_process_group(pid, cleanup_deadline).await {
        Ok(()) => true,
        Err(error) => {
            cleanup_failures.push(error.to_string());
            false
        }
    };
    if !observation.owner_reaped {
        match tokio::time::timeout_at(cleanup_deadline, child.wait()).await {
            Ok(Ok(owner_status)) => {
                observation.status.get_or_insert(owner_status);
                observation.owner_reaped = true;
            }
            Ok(Err(error)) => cleanup_failures.push(error.to_string()),
            Err(_) => cleanup_failures.push(format!("timed out reaping process owner {pid}")),
        }
    }

    if !observation.readers_joined {
        match collect_readers(cleanup_deadline, &mut readers).await {
            Ok(Some(output)) => {
                observation.captured = Some(output);
                observation.readers_joined = true;
            }
            Ok(None) => {}
            Err(error) => {
                observation.readers_joined = true;
                if observation.primary_error.is_none() && !observation.exceeded_bound {
                    observation.primary_error = Some(error);
                }
            }
        }
    }
    if !observation.readers_joined
        && let Err(error) = abort_and_join_readers(cleanup_deadline, &mut readers).await
    {
        cleanup_failures.push(error.to_string());
    }

    let (stdout, stderr) = observation.captured.unwrap_or_default();
    if observation.exceeded_bound && observation.primary_error.is_none() {
        let cleanup_evidence = if process_group_clean && observation.owner_reaped {
            format!("; process group {pid} killed and owner reaped")
        } else {
            String::new()
        };
        observation.primary_error = Some(CanaryError::new(format!(
            "external proof exceeded bound{cleanup_evidence}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr),
        )));
    }
    if let Some(primary) = observation.primary_error {
        if cleanup_failures.is_empty() {
            return Err(primary);
        }
        return Err(CanaryError::new(format!(
            "{primary}; cleanup also failed: {}",
            cleanup_failures.join("; ")
        )));
    }
    if !cleanup_failures.is_empty() {
        return Err(CanaryError::new(cleanup_failures.join("; ")));
    }
    Ok(OwnedOutput {
        status: observation
            .status
            .ok_or_else(|| CanaryError::new("process owner status was not observed"))?,
        stdout,
        stderr,
        owner_reaped: observation.owner_reaped,
        process_group_clean,
    })
}

async fn observe_run(
    child: &mut tokio::process::Child,
    readers: &mut JoinHandle<CanaryResult<(Vec<u8>, Vec<u8>)>>,
    deadline: Instant,
) -> RunObservation {
    let owner = tokio::time::timeout_at(deadline, child.wait()).await;
    let mut observation = RunObservation {
        status: None,
        owner_reaped: false,
        captured: None,
        readers_joined: false,
        exceeded_bound: false,
        primary_error: None,
    };
    match owner {
        Ok(Ok(status)) => {
            observation.status = Some(status);
            observation.owner_reaped = true;
        }
        Ok(Err(error)) => observation.primary_error = Some(error.into()),
        Err(_) => observation.exceeded_bound = true,
    }
    match collect_readers(deadline, readers).await {
        Ok(Some(output)) => {
            observation.captured = Some(output);
            observation.readers_joined = true;
        }
        Ok(None) => observation.exceeded_bound = true,
        Err(error) => {
            observation.readers_joined = true;
            if observation.primary_error.is_none() && !observation.exceeded_bound {
                observation.primary_error = Some(error);
            }
        }
    }
    observation
}

async fn clean_process_group(pid: u32, deadline: Instant) -> CanaryResult<()> {
    if !process_group_has_live_members(pid).await? {
        return Ok(());
    }
    let kill_result = kill_process_group(pid).await;
    if let Err(error) = kill_result
        && process_group_has_live_members(pid).await?
    {
        return Err(error);
    }
    tokio::time::timeout_at(deadline, async {
        loop {
            if !process_group_has_live_members(pid).await? {
                return Ok(());
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .map_err(|_| CanaryError::new(format!("timed out cleaning process group {pid}")))?
}

async fn process_group_has_live_members(pid: u32) -> CanaryResult<bool> {
    let output = tokio::process::Command::new("/bin/ps")
        .args(["-axo", "pgid=,stat="])
        .stdin(Stdio::null())
        .output()
        .await?;
    if !output.status.success() {
        return Err(CanaryError::new("failed to inspect owned process groups"));
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut fields = line.split_whitespace();
        let Some(group) = fields.next() else {
            continue;
        };
        let Some(state) = fields.next() else {
            continue;
        };
        if group.parse::<u32>().ok() == Some(pid) && !state.starts_with('Z') {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn kill_process_group(pid: u32) -> CanaryResult<()> {
    let status = tokio::process::Command::new("/bin/kill")
        .args(["-KILL", &format!("-{pid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;
    if !status.success() {
        return Err(CanaryError::new(format!(
            "failed to kill owned process group {pid}"
        )));
    }
    Ok(())
}

async fn read_bounded(mut reader: impl AsyncRead + Unpin) -> CanaryResult<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8_192];
    let mut exceeded = false;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = OUTPUT_CAPACITY.saturating_sub(bytes.len());
        bytes.extend_from_slice(&buffer[..read.min(remaining)]);
        exceeded |= read > remaining;
    }
    if exceeded {
        return Err(CanaryError::new(format!(
            "external process output exceeded {OUTPUT_CAPACITY} bytes"
        )));
    }
    Ok(bytes)
}

async fn collect_readers(
    deadline: Instant,
    readers: &mut JoinHandle<CanaryResult<(Vec<u8>, Vec<u8>)>>,
) -> CanaryResult<Option<(Vec<u8>, Vec<u8>)>> {
    match tokio::time::timeout_at(deadline, &mut *readers).await {
        Ok(output) => output
            .map_err(|error| CanaryError::new(format!("output readers failed: {error}")))?
            .map(Some),
        Err(_) => Ok(None),
    }
}

async fn abort_and_join_readers(
    deadline: Instant,
    readers: &mut JoinHandle<CanaryResult<(Vec<u8>, Vec<u8>)>>,
) -> CanaryResult<()> {
    readers.abort();
    match tokio::time::timeout_at(deadline, &mut *readers).await {
        Ok(Err(error)) if error.is_cancelled() => Ok(()),
        Ok(Err(error)) => Err(CanaryError::new(format!(
            "failed joining aborted output readers: {error}"
        ))),
        Ok(Ok(_)) => Ok(()),
        Err(_) => Err(CanaryError::new("timed out joining output readers")),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;
    use std::time::Duration;

    use super::run_owned;

    #[tokio::test]
    async fn timeout_kills_owned_group_and_reaps_owner() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30 & echo DESCENDANT_PID=$!; wait"]);
        let failure = run_owned(command, Duration::from_secs(1))
            .await
            .expect_err("owned process tree must exceed the bound");
        let message = failure.to_string();
        assert!(message.contains("process group"));
        assert!(message.contains("owner reaped"));
        let descendant = message
            .split("DESCENDANT_PID=")
            .nth(1)
            .and_then(|suffix| suffix.split_whitespace().next())
            .expect("descendant process identifier");
        let status = Command::new("ps")
            .args(["-o", "stat=", "-p", descendant])
            .output()
            .expect("inspect descendant after group kill");
        let state = String::from_utf8_lossy(&status.stdout);
        assert!(state.trim().is_empty() || state.trim_start().starts_with('Z'));
    }

    #[tokio::test]
    async fn owner_exit_does_not_escape_deadline_through_inherited_pipes() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 2 & echo DESCENDANT_PID=$!"]);
        let started = tokio::time::Instant::now();
        let failure = run_owned(command, Duration::from_millis(75))
            .await
            .expect_err("inherited descendant pipe must remain deadline-bound");
        assert!(started.elapsed() < Duration::from_secs(1));
        let message = failure.to_string();
        assert!(message.contains("process group"));
        assert!(message.contains("owner reaped"));
    }

    #[tokio::test]
    async fn successful_owner_cleans_descendant_with_redirected_streams() {
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "sleep 30 </dev/null >/dev/null 2>/dev/null & echo DESCENDANT_PID=$!",
        ]);
        let output = run_owned(command, Duration::from_secs(1))
            .await
            .expect("successful owner remains successful after group cleanup");
        assert!(output.status.success());
        assert!(output.owner_reaped);
        assert!(output.process_group_clean);
        let descendant = String::from_utf8_lossy(&output.stdout)
            .split("DESCENDANT_PID=")
            .nth(1)
            .and_then(|suffix| suffix.split_whitespace().next())
            .expect("descendant process identifier")
            .to_owned();
        let status = Command::new("ps")
            .args(["-o", "stat=", "-p", &descendant])
            .output()
            .expect("inspect descendant after successful owner exit");
        let state = String::from_utf8_lossy(&status.stdout);
        assert!(state.trim().is_empty() || state.trim_start().starts_with('Z'));
    }

    #[tokio::test]
    async fn bounded_output_failure_cleans_redirected_descendant_before_returning() {
        let pid_file = std::env::temp_dir().join(format!(
            "fava-canary-output-bound-descendant-{}.pid",
            std::process::id()
        ));
        let _ = fs::remove_file(&pid_file);
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            concat!(
                "sleep 30 </dev/null >/dev/null 2>/dev/null & ",
                "printf '%s' \"$!\" > \"$1\"; /usr/bin/head -c 1048577 /dev/zero"
            ),
            "fava-output-bound",
            pid_file.to_str().expect("temporary path is UTF-8"),
        ]);
        let failure = run_owned(command, Duration::from_secs(2))
            .await
            .expect_err("oversized output must be refused");
        assert!(failure.to_string().contains("exceeded 1048576 bytes"));

        let descendant = fs::read_to_string(&pid_file).expect("descendant pid was recorded");
        fs::remove_file(&pid_file).expect("temporary pid file removed");
        let status = Command::new("ps")
            .args(["-o", "stat=", "-p", descendant.trim()])
            .output()
            .expect("inspect descendant after bounded-output refusal");
        let state = String::from_utf8_lossy(&status.stdout);
        let clean = state.trim().is_empty() || state.trim_start().starts_with('Z');
        if !clean {
            Command::new("/bin/kill")
                .args(["-KILL", descendant.trim()])
                .status()
                .expect("remove leaked descendant after failed assertion");
        }
        assert!(clean);
    }
}
