//! Bounded ownership of external canary processes.

use std::os::unix::process::CommandExt;
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{CanaryError, CanaryResult};

const OUTPUT_CAPACITY: usize = 1_048_576;

#[derive(Debug)]
pub(super) struct OwnedOutput {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) owner_reaped: bool,
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
    let stdout_reader = tokio::spawn(read_bounded(stdout));
    let stderr_reader = tokio::spawn(read_bounded(stderr));

    let wait = tokio::time::timeout(deadline, child.wait()).await;
    let timed_out = wait.is_err();
    let status = if let Ok(status) = wait {
        status?
    } else {
        kill_process_group(pid).await?;
        child.wait().await?
    };
    let stdout = join_reader(stdout_reader).await?;
    let stderr = join_reader(stderr_reader).await?;
    if timed_out {
        return Err(CanaryError::new(format!(
            "external proof exceeded bound; process group {pid} killed and owner reaped; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr),
        )));
    }
    Ok(OwnedOutput {
        status,
        stdout,
        stderr,
        owner_reaped: true,
    })
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

async fn join_reader(
    reader: tokio::task::JoinHandle<CanaryResult<Vec<u8>>>,
) -> CanaryResult<Vec<u8>> {
    reader
        .await
        .map_err(|error| CanaryError::new(format!("output reader failed: {error}")))?
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
}
