//! Third-party relay process supervision.

use std::fs::{self, OpenOptions};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::{Instant, timeout};

use crate::{CanaryError, CanaryResult};

pub(crate) const RELAY_VERSION: &str = "0.8.12";

pub(crate) struct RelaySupervisor {
    binary: PathBuf,
    config: PathBuf,
    data: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
    address: SocketAddr,
}

impl RelaySupervisor {
    pub(crate) fn prepare(binary: &Path, relay_dir: &Path, port: u16) -> CanaryResult<Self> {
        Self::prepare_with_whitelist(binary, relay_dir, port, None)
    }

    pub(crate) fn prepare_rejecting(
        binary: &Path,
        relay_dir: &Path,
        port: u16,
        permitted_pubkey: &str,
    ) -> CanaryResult<Self> {
        Self::prepare_with_whitelist(binary, relay_dir, port, Some(permitted_pubkey))
    }

    fn prepare_with_whitelist(
        binary: &Path,
        relay_dir: &Path,
        port: u16,
        permitted_pubkey: Option<&str>,
    ) -> CanaryResult<Self> {
        let config = relay_dir.join("config.toml");
        let data = relay_dir.join("data");
        fs::create_dir_all(&data)?;
        fs::write(&config, relay_config(port, permitted_pubkey))?;
        Ok(Self {
            binary: binary.to_owned(),
            config,
            data,
            stdout: relay_dir.join("stdout.log"),
            stderr: relay_dir.join("stderr.log"),
            address: SocketAddr::from(([127, 0, 0, 1], port)),
        })
    }

    pub(crate) fn address(&self) -> SocketAddr {
        self.address
    }

    pub(crate) async fn version(&self) -> CanaryResult<String> {
        let output = Command::new(&self.binary).arg("--version").output().await?;
        if !output.status.success() {
            return Err(CanaryError::new(format!(
                "relay version command failed with {}",
                output.status
            )));
        }
        let version = String::from_utf8(output.stdout)?.trim().to_owned();
        let expected = format!("nostr-rs-relay {RELAY_VERSION}");
        if version != expected {
            return Err(CanaryError::new(format!(
                "relay version mismatch: expected {expected:?}, got {version:?}"
            )));
        }
        Ok(version)
    }

    pub(crate) async fn spawn(&self, generation: u64) -> CanaryResult<RelayProcess> {
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.stdout)?;
        let stderr = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.stderr)?;
        let mut child = Command::new(&self.binary)
            .arg("--config")
            .arg(&self.config)
            .arg("--db")
            .arg(&self.data)
            .env("RUST_LOG", "info")
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true)
            .spawn()?;
        let pid = child
            .id()
            .ok_or_else(|| CanaryError::new("relay process has no pid"))?;
        wait_ready(&mut child, self.address).await?;
        Ok(RelayProcess {
            child,
            pid,
            generation,
        })
    }
}

pub(crate) struct RelayProcess {
    child: Child,
    pid: u32,
    generation: u64,
}

impl RelayProcess {
    pub(crate) fn fact(&self, action: &'static str) -> ProcessFact {
        ProcessFact {
            action,
            pid: self.pid,
            generation: self.generation,
        }
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) async fn hard_kill(mut self) -> CanaryResult<ProcessFact> {
        self.child.kill().await?;
        let status = self.child.wait().await?;
        if status.success() {
            return Err(CanaryError::new(
                "hard-killed relay unexpectedly reported successful exit",
            ));
        }
        Ok(self.fact("hard-killed"))
    }

    pub(crate) async fn graceful_stop(mut self) -> CanaryResult<ProcessFact> {
        let status = Command::new("kill")
            .args(["-TERM", &self.pid.to_string()])
            .status()
            .await?;
        if !status.success() {
            return Err(CanaryError::new(format!(
                "failed to send SIGTERM to relay pid {}",
                self.pid
            )));
        }
        if let Ok(status) = timeout(Duration::from_secs(5), self.child.wait()).await {
            status?;
        } else {
            self.child.kill().await?;
            self.child.wait().await?;
        }
        Ok(self.fact("gracefully-stopped"))
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct ProcessFact {
    pub(crate) action: &'static str,
    pub(crate) pid: u32,
    pub(crate) generation: u64,
}

async fn wait_ready(child: &mut Child, address: SocketAddr) -> CanaryResult<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if TcpStream::connect(address).await.is_ok() {
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            return Err(CanaryError::new(format!(
                "relay exited before readiness with {status}"
            )));
        }
        if Instant::now() >= deadline {
            return Err(CanaryError::new(format!(
                "relay did not listen on {address} before readiness deadline"
            )));
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn relay_config(port: u16, permitted_pubkey: Option<&str>) -> String {
    let whitelist = permitted_pubkey.map_or_else(String::new, |pubkey| {
        format!("pubkey_whitelist = [\"{pubkey}\"]\n")
    });
    format!(
        r#"[info]
relay_url = "ws://127.0.0.1:{port}/"
name = "Fava M0 canary relay"
description = "Disposable third-party relay for the Fava rewrite evidence lab"

[database]
engine = "sqlite"
in_memory = false

[network]
address = "127.0.0.1"
port = {port}
ping_interval = 30

[options]
reject_future_seconds = 1800

[limits]
max_event_bytes = 131072
max_ws_message_bytes = 131072
max_ws_frame_bytes = 131072
broadcast_buffer = 1024
event_persist_buffer = 1024

[authorization]
nip42_auth = false
{whitelist}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::relay_config;

    #[test]
    fn relay_config_is_loopback_persistent_and_port_scoped() {
        let config = relay_config(12345, None);
        assert!(config.contains("address = \"127.0.0.1\""));
        assert!(config.contains("port = 12345"));
        assert!(config.contains("in_memory = false"));
    }

    #[test]
    fn rejecting_relay_config_whitelists_only_the_control_key() {
        let config = relay_config(12345, Some("control-public-key"));
        assert!(config.contains("pubkey_whitelist = [\"control-public-key\"]"));
    }
}
