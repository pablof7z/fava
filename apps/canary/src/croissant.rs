//! Controlled Croissant process supervision for the Phase 07.1 canary.

use std::fmt;
use std::fs;
use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdTcpListener};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tokio::time::{Instant, timeout};

use crate::command_output;

/// Hard bounds applied to one controlled Croissant process.
#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct CroissantLimits {
    pub(crate) log_bytes: usize,
    pub(crate) readiness_ms: u64,
    pub(crate) teardown_ms: u64,
}

impl Default for CroissantLimits {
    fn default() -> Self {
        Self {
            log_bytes: 1_048_576,
            readiness_ms: 10_000,
            teardown_ms: 5_000,
        }
    }
}

#[cfg(test)]
impl CroissantLimits {
    pub(crate) const fn test() -> Self {
        Self {
            log_bytes: 4_096,
            readiness_ms: 2_000,
            teardown_ms: 2_000,
        }
    }
}

/// Attributed process-contract failure without captured relay content.
#[derive(Debug)]
pub(crate) enum CroissantError {
    Io(std::io::Error),
    InvalidContract(&'static str),
    EarlyExit { status: String },
    ReadinessTimeout { endpoint: SocketAddr },
    LogOverflow { stream: &'static str, limit: usize },
    TeardownFailure { pid: u32, reason: &'static str },
}

impl fmt::Display for CroissantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "Croissant process I/O failed: {error}"),
            Self::InvalidContract(message) => formatter.write_str(message),
            Self::EarlyExit { status } => {
                write!(formatter, "Croissant exited before readiness with {status}")
            }
            Self::ReadinessTimeout { endpoint } => {
                write!(
                    formatter,
                    "Croissant did not listen on {endpoint} before readiness deadline"
                )
            }
            Self::LogOverflow { stream, limit } => {
                write!(
                    formatter,
                    "Croissant {stream} exceeded the {limit}-byte bound"
                )
            }
            Self::TeardownFailure { pid, reason } => {
                write!(formatter, "Croissant pid {pid} teardown failed: {reason}")
            }
        }
    }
}

impl std::error::Error for CroissantError {}

impl From<std::io::Error> for CroissantError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Immutable executable, checkout, and readiness evidence.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct CroissantReadyFact {
    pub(crate) executable: PathBuf,
    pub(crate) executable_sha256: String,
    pub(crate) source_checkout: PathBuf,
    pub(crate) source_head: String,
    pub(crate) pid: u32,
    pub(crate) endpoint: SocketAddr,
    pub(crate) data_path: PathBuf,
    pub(crate) stdout_path: PathBuf,
    pub(crate) stderr_path: PathBuf,
    pub(crate) scenario_seed_sha256: String,
    pub(crate) readiness_completed: bool,
    pub(crate) limits: CroissantLimits,
}

/// Completed child and endpoint cleanup evidence.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct CroissantTeardown {
    pub(crate) pid: u32,
    pub(crate) endpoint: SocketAddr,
    pub(crate) completed: bool,
    pub(crate) pid_alive_after: bool,
    pub(crate) port_open_after: bool,
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
}

/// Prepared, inert Croissant child configuration.
#[derive(Debug)]
pub(crate) struct CroissantSupervisor {
    binary: PathBuf,
    source_checkout: PathBuf,
    data_path: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    owner_public_key: String,
    scenario_seed_sha256: String,
    endpoint: SocketAddr,
    executable_sha256: String,
    source_head: String,
    limits: CroissantLimits,
}

impl CroissantSupervisor {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        binary: &Path,
        source_checkout: &Path,
        root: &Path,
        owner_public_key: &str,
        scenario_seed_sha256: &str,
        limits: CroissantLimits,
    ) -> Result<Self, CroissantError> {
        if !binary.is_file() {
            return Err(CroissantError::InvalidContract(
                "Croissant executable is absent or not a file",
            ));
        }
        if owner_public_key.len() != 64
            || !owner_public_key
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(CroissantError::InvalidContract(
                "Croissant owner public key must be 32-byte hex",
            ));
        }
        if scenario_seed_sha256.len() != 64
            || !scenario_seed_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(CroissantError::InvalidContract(
                "scenario seed digest must be SHA-256 hex",
            ));
        }
        if root.exists() {
            return Err(CroissantError::InvalidContract(
                "Croissant run directory must be fresh",
            ));
        }
        let binary = fs::canonicalize(binary)?;
        let source_checkout = fs::canonicalize(source_checkout)?;
        let data_path = root.join("data");
        fs::create_dir_all(&data_path)?;
        let stdout_path = root.join("stdout.log");
        let stderr_path = root.join("stderr.log");
        fs::File::create(&stdout_path)?;
        fs::File::create(&stderr_path)?;
        let executable_sha256 = hex::encode(Sha256::digest(fs::read(&binary)?));
        let source_head = command_output(&source_checkout, "git", &["rev-parse", "HEAD"])
            .map_err(|_| CroissantError::InvalidContract("Croissant source HEAD is unavailable"))?;
        let endpoint = SocketAddr::from((Ipv4Addr::LOCALHOST, reserve_port()?));
        Ok(Self {
            binary,
            source_checkout,
            data_path,
            stdout_path,
            stderr_path,
            owner_public_key: owner_public_key.to_ascii_lowercase(),
            scenario_seed_sha256: scenario_seed_sha256.to_ascii_lowercase(),
            endpoint,
            executable_sha256,
            source_head,
            limits,
        })
    }

    pub(crate) fn stdout_path(&self) -> &Path {
        &self.stdout_path
    }

    pub(crate) fn stderr_path(&self) -> &Path {
        &self.stderr_path
    }

    pub(crate) async fn start(&self) -> Result<CroissantProcess, CroissantError> {
        let mut child = Command::new(&self.binary)
            .env_clear()
            .env("HOST", Ipv4Addr::LOCALHOST.to_string())
            .env("PORT", self.endpoint.port().to_string())
            .env("DATAPATH", &self.data_path)
            .env("DOMAIN", "")
            .env("OWNER_PUBLIC_KEY", &self.owner_public_key)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let pid = child.id().ok_or(CroissantError::InvalidContract(
            "Croissant child did not expose a pid",
        ))?;
        let stdout = child.stdout.take().ok_or(CroissantError::InvalidContract(
            "Croissant stdout pipe is unavailable",
        ))?;
        let stderr = child.stderr.take().ok_or(CroissantError::InvalidContract(
            "Croissant stderr pipe is unavailable",
        ))?;
        let stdout_log = BoundedLog::start(stdout, self.stdout_path.clone(), self.limits.log_bytes);
        let stderr_log = BoundedLog::start(stderr, self.stderr_path.clone(), self.limits.log_bytes);
        let deadline = Instant::now() + Duration::from_millis(self.limits.readiness_ms);
        loop {
            if stdout_log.overflowed() {
                terminate(&mut child, self.limits.teardown_ms).await;
                return Err(CroissantError::LogOverflow {
                    stream: "stdout",
                    limit: self.limits.log_bytes,
                });
            }
            if stderr_log.overflowed() {
                terminate(&mut child, self.limits.teardown_ms).await;
                return Err(CroissantError::LogOverflow {
                    stream: "stderr",
                    limit: self.limits.log_bytes,
                });
            }
            if TcpStream::connect(self.endpoint).await.is_ok() {
                return Ok(CroissantProcess {
                    child,
                    ready: self.ready_fact(pid),
                    stdout_log,
                    stderr_log,
                });
            }
            if let Some(status) = child.try_wait()? {
                stdout_log.finish(self.limits.teardown_ms).await?;
                stderr_log.finish(self.limits.teardown_ms).await?;
                return Err(CroissantError::EarlyExit {
                    status: status.to_string(),
                });
            }
            if Instant::now() >= deadline {
                terminate(&mut child, self.limits.teardown_ms).await;
                return Err(CroissantError::ReadinessTimeout {
                    endpoint: self.endpoint,
                });
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn ready_fact(&self, pid: u32) -> CroissantReadyFact {
        CroissantReadyFact {
            executable: self.binary.clone(),
            executable_sha256: self.executable_sha256.clone(),
            source_checkout: self.source_checkout.clone(),
            source_head: self.source_head.clone(),
            pid,
            endpoint: self.endpoint,
            data_path: self.data_path.clone(),
            stdout_path: self.stdout_path.clone(),
            stderr_path: self.stderr_path.clone(),
            scenario_seed_sha256: self.scenario_seed_sha256.clone(),
            readiness_completed: true,
            limits: self.limits,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CroissantProcess {
    child: Child,
    ready: CroissantReadyFact,
    stdout_log: BoundedLog,
    stderr_log: BoundedLog,
}

impl CroissantProcess {
    pub(crate) fn ready_fact(&self) -> CroissantReadyFact {
        self.ready.clone()
    }

    pub(crate) async fn stop(mut self) -> Result<CroissantTeardown, CroissantError> {
        let pid = self.ready.pid;
        let endpoint = self.ready.endpoint;
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await?;
        if !status.success() {
            return Err(CroissantError::TeardownFailure {
                pid,
                reason: "SIGTERM command failed",
            });
        }
        terminate(&mut self.child, self.ready.limits.teardown_ms).await;
        let stdout = self
            .stdout_log
            .finish(self.ready.limits.teardown_ms)
            .await?;
        let stderr = self
            .stderr_log
            .finish(self.ready.limits.teardown_ms)
            .await?;
        if stdout.overflowed {
            return Err(CroissantError::LogOverflow {
                stream: "stdout",
                limit: self.ready.limits.log_bytes,
            });
        }
        if stderr.overflowed {
            return Err(CroissantError::LogOverflow {
                stream: "stderr",
                limit: self.ready.limits.log_bytes,
            });
        }
        let pid_alive_after = process_is_alive(pid);
        let port_open_after = TcpStream::connect(endpoint).await.is_ok();
        if pid_alive_after || port_open_after {
            return Err(CroissantError::TeardownFailure {
                pid,
                reason: "pid or loopback port remained live",
            });
        }
        Ok(CroissantTeardown {
            pid,
            endpoint,
            completed: true,
            pid_alive_after,
            port_open_after,
            stdout_bytes: stdout.bytes,
            stderr_bytes: stderr.bytes,
        })
    }
}

#[derive(Debug)]
struct BoundedLog {
    overflow: Arc<AtomicBool>,
    task: JoinHandle<Result<LogResult, std::io::Error>>,
}

#[derive(Debug)]
struct LogResult {
    bytes: usize,
    overflowed: bool,
}

impl BoundedLog {
    fn start(
        mut reader: impl AsyncRead + Unpin + Send + 'static,
        path: PathBuf,
        limit: usize,
    ) -> Self {
        let overflow = Arc::new(AtomicBool::new(false));
        let task_overflow = Arc::clone(&overflow);
        let task = tokio::spawn(async move {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)
                .await?;
            let mut buffer = [0_u8; 4096];
            let mut retained = 0_usize;
            loop {
                let read = reader.read(&mut buffer).await?;
                if read == 0 {
                    break;
                }
                let available = limit.saturating_sub(retained);
                let to_write = read.min(available);
                if to_write != 0 {
                    file.write_all(&buffer[..to_write]).await?;
                    retained += to_write;
                }
                if to_write != read {
                    task_overflow.store(true, Ordering::Release);
                }
            }
            file.flush().await?;
            Ok(LogResult {
                bytes: retained,
                overflowed: task_overflow.load(Ordering::Acquire),
            })
        });
        Self { overflow, task }
    }

    fn overflowed(&self) -> bool {
        self.overflow.load(Ordering::Acquire)
    }

    async fn finish(self, deadline_ms: u64) -> Result<LogResult, CroissantError> {
        timeout(Duration::from_millis(deadline_ms), self.task)
            .await
            .map_err(|_| CroissantError::InvalidContract("Croissant log drain deadline elapsed"))?
            .map_err(|_| CroissantError::InvalidContract("Croissant log drain task failed"))?
            .map_err(CroissantError::Io)
    }
}

async fn terminate(child: &mut Child, deadline_ms: u64) {
    if timeout(Duration::from_millis(deadline_ms), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

fn reserve_port() -> Result<u16, CroissantError> {
    let listener = StdTcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    Ok(listener.local_addr()?.port())
}

pub(crate) fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(test)]
#[path = "croissant_tests.rs"]
mod tests;
