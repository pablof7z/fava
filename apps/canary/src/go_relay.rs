//! Go relay process supervision for Phase F cross-implementation relay testing.
//!
//! Handles relay29 and communities-relay (both Go, env-var-configured).
//! Unlike the nostr-rs-relay supervisor, these binaries:
//! - Use environment variables instead of a config file.
//! - Have no `--version` flag to check.
//! - Use an LMDB or bbolt database directory that persists across restarts.

use std::fs::{self, OpenOptions};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::Instant;

use crate::{CanaryError, CanaryResult};

/// Supervisor for a Go NIP-29 relay (relay29 or communities-relay).
pub(crate) struct GoRelaySupervisor {
    binary: PathBuf,
    port: u16,
    relay_privkey: String,
    data_dir: PathBuf,
    relay_name: String,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl GoRelaySupervisor {
    /// Prepare the supervisor. Does not start the relay yet.
    ///
    /// - `privkey`: 64-char lowercase hex secp256k1 private key for the relay.
    pub(crate) fn prepare(
        binary: &Path,
        relay_dir: &Path,
        port: u16,
        relay_name: &str,
        privkey: &str,
    ) -> CanaryResult<Self> {
        let data_dir = relay_dir.join("data");
        fs::create_dir_all(&data_dir)?;
        Ok(Self {
            binary: binary.to_owned(),
            port,
            relay_privkey: privkey.to_owned(),
            data_dir,
            relay_name: relay_name.to_owned(),
            stdout: relay_dir.join("stdout.log"),
            stderr: relay_dir.join("stderr.log"),
        })
    }

    /// TCP address the relay will listen on.
    pub(crate) fn address(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.port))
    }

    /// WebSocket URL for this relay.
    pub(crate) fn url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }

    /// Spawn a new relay process. Returns when the relay is TCP-ready.
    pub(crate) async fn spawn(&self, generation: u64) -> CanaryResult<GoRelayProcess> {
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.stdout)?;
        let stderr = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.stderr)?;
        let domain = format!("localhost:{}", self.port);
        let mut child = Command::new(&self.binary)
            .env("PORT", self.port.to_string())
            .env("DOMAIN", &domain)
            .env("RELAY_NAME", &self.relay_name)
            .env("RELAY_PRIVKEY", &self.relay_privkey)
            .env("DATABASE_PATH", &self.data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true)
            .spawn()?;
        let pid = child
            .id()
            .ok_or_else(|| CanaryError::new("Go relay process has no pid"))?;
        wait_tcp_ready(&mut child, self.address()).await?;
        Ok(GoRelayProcess {
            child,
            pid,
            generation,
        })
    }
}

/// A running Go relay process.
pub(crate) struct GoRelayProcess {
    child: Child,
    pid: u32,
    generation: u64,
}

impl GoRelayProcess {
    pub(crate) fn fact(&self, action: &'static str) -> GoRelayFact {
        GoRelayFact {
            action,
            pid: self.pid,
            generation: self.generation,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) async fn hard_kill(mut self) -> CanaryResult<GoRelayFact> {
        self.child.kill().await?;
        self.child.wait().await?;
        Ok(self.fact("hard-killed"))
    }

    pub(crate) async fn graceful_stop(mut self) -> CanaryResult<GoRelayFact> {
        // SIGTERM + wait with fallback kill.
        let status = Command::new("kill")
            .args(["-TERM", &self.pid.to_string()])
            .status()
            .await?;
        if !status.success() {
            return Err(CanaryError::new(format!(
                "failed to send SIGTERM to Go relay pid {}",
                self.pid
            )));
        }
        if let Ok(exit) = tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await {
            exit?;
        } else {
            self.child.kill().await?;
            self.child.wait().await?;
        }
        Ok(self.fact("gracefully-stopped"))
    }
}

/// Evidence fact about a Go relay process lifecycle event.
#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct GoRelayFact {
    pub(crate) action: &'static str,
    pub(crate) pid: u32,
    pub(crate) generation: u64,
}

/// Wait until the relay is accepting TCP connections or times out.
async fn wait_tcp_ready(child: &mut Child, address: SocketAddr) -> CanaryResult<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if TcpStream::connect(address).await.is_ok() {
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            return Err(CanaryError::new(format!(
                "Go relay exited before readiness with {status}"
            )));
        }
        if Instant::now() >= deadline {
            return Err(CanaryError::new(format!(
                "Go relay did not listen on {address} before readiness deadline"
            )));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
