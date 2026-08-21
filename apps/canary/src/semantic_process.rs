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
    let mut stdout_reader = tokio::spawn(read_bounded(stdout));
    let mut stderr_reader = tokio::spawn(read_bounded(stderr));

    let absolute_deadline = Instant::now() + deadline;
    let owner = tokio::time::timeout_at(absolute_deadline, child.wait()).await;
    let (status, owner_reaped) = match owner {
        Ok(status) => (Some(status?), true),
        Err(_) => (None, false),
    };
    if let Some(status) = status
        && let Some((stdout, stderr)) =
            collect_readers(absolute_deadline, &mut stdout_reader, &mut stderr_reader).await?
    {
        let cleanup_deadline = Instant::now() + CLEANUP_CAPACITY;
        clean_process_group(pid, cleanup_deadline).await?;
        return Ok(OwnedOutput {
            status,
            stdout,
            stderr,
            owner_reaped,
            process_group_clean: true,
        });
    }

    let cleanup_deadline = Instant::now() + CLEANUP_CAPACITY;
    clean_process_group(pid, cleanup_deadline).await?;
    if !owner_reaped {
        tokio::time::timeout_at(cleanup_deadline, child.wait())
            .await
            .map_err(|_| CanaryError::new(format!("timed out reaping process owner {pid}")))??;
    }
    let captured =
        collect_readers(cleanup_deadline, &mut stdout_reader, &mut stderr_reader).await?;
    if captured.is_none() {
        stdout_reader.abort();
        stderr_reader.abort();
    }
    let (stdout, stderr) = captured.unwrap_or_default();
    Err(CanaryError::new(format!(
        "external proof exceeded bound; process group {pid} killed and owner reaped; stdout: {}; stderr: {}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr),
    )))
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

async fn join_reader(reader: &mut JoinHandle<CanaryResult<Vec<u8>>>) -> CanaryResult<Vec<u8>> {
    (&mut *reader)
        .await
        .map_err(|error| CanaryError::new(format!("output reader failed: {error}")))?
}

async fn collect_readers(
    deadline: Instant,
    stdout: &mut JoinHandle<CanaryResult<Vec<u8>>>,
    stderr: &mut JoinHandle<CanaryResult<Vec<u8>>>,
) -> CanaryResult<Option<(Vec<u8>, Vec<u8>)>> {
    match tokio::time::timeout_at(deadline, async {
        tokio::try_join!(join_reader(stdout), join_reader(stderr))
    })
    .await
    {
        Ok(output) => output.map(Some),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
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
}
