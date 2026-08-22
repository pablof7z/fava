//! Controlled Croissant process supervision for the Phase 07.1 canary.
//!
//! This file stays above the 500-line soft limit so launch, readiness stability, bounded logs,
//! and teardown remain one process-lifecycle owner with no competing child-state authority.

use std::fmt;
use std::fs;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdTcpListener};
#[cfg(not(target_os = "linux"))]
use std::os::unix::fs::FileExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
#[cfg(not(target_os = "linux"))]
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tokio::time::{Instant, timeout};

use crate::command_output;
#[cfg(target_os = "linux")]
use crate::sealed_executable::SealedExecutable;

/// Hard bounds applied to one controlled Croissant process.
#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct CroissantLimits {
    pub(crate) log_bytes: usize,
    pub(crate) readiness_ms: u64,
    pub(crate) readiness_stability_ms: u64,
    pub(crate) teardown_ms: u64,
}

impl Default for CroissantLimits {
    fn default() -> Self {
        Self {
            log_bytes: 1_048_576,
            readiness_ms: 10_000,
            readiness_stability_ms: 100,
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
            readiness_stability_ms: 50,
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
    pub(crate) executable_device: u64,
    pub(crate) executable_inode: u64,
    pub(crate) execution_platform: &'static str,
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
#[allow(
    clippy::struct_excessive_bools,
    reason = "each independently falsifiable teardown invariant is retained as an exact fact"
)]
#[derive(Clone, Debug, Serialize)]
pub(crate) struct CroissantTeardown {
    pub(crate) pid: u32,
    pub(crate) endpoint: SocketAddr,
    pub(crate) completed: bool,
    pub(crate) pid_alive_after: bool,
    pub(crate) port_open_after: bool,
    pub(crate) executable_removed: bool,
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
}

/// Prepared, inert Croissant child configuration.
#[derive(Debug)]
pub(crate) struct CroissantSupervisor {
    binary: Arc<StagedExecutable>,
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
        let source_status = command_output(
            &source_checkout,
            "git",
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )
        .map_err(|_| CroissantError::InvalidContract("Croissant source status is unavailable"))?;
        if !source_status.is_empty() {
            return Err(CroissantError::InvalidContract(
                "Croissant source checkout must be clean",
            ));
        }
        fs::create_dir_all(root)?;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        let executable_directory = root.join("executable");
        fs::create_dir(&executable_directory)?;
        fs::set_permissions(&executable_directory, fs::Permissions::from_mode(0o700))?;
        let executable_directory_metadata = fs::metadata(&executable_directory)?;
        let staged_path = executable_directory.join("croissant");
        fs::copy(&binary, &staged_path)?;
        fs::set_permissions(&staged_path, fs::Permissions::from_mode(0o500))?;
        let mut open = fs::OpenOptions::new();
        open.read(true).custom_flags(libc::O_NOFOLLOW);
        let file = open.open(&staged_path)?;
        if file.metadata()?.nlink() != 1 {
            return Err(CroissantError::InvalidContract(
                "Croissant staged executable must have exactly one link",
            ));
        }
        #[cfg(target_os = "linux")]
        let sealed = SealedExecutable::copy_from(&file, 134_217_728)?;
        let binary = Arc::new(StagedExecutable {
            file,
            #[cfg(target_os = "linux")]
            sealed,
            path: staged_path,
            launched: AtomicBool::new(false),
            cleaned: AtomicBool::new(false),
            directory_device: executable_directory_metadata.dev(),
            directory_inode: executable_directory_metadata.ino(),
        });
        let data_path = root.join("data");
        fs::create_dir_all(&data_path)?;
        let stdout_path = root.join("stdout.log");
        let stderr_path = root.join("stderr.log");
        fs::File::create(&stdout_path)?;
        fs::File::create(&stderr_path)?;
        let executable_sha256 = binary.sha256()?;
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

    pub(crate) fn cleanup_executable(&self) -> Result<(), CroissantError> {
        self.binary.cleanup().map_err(CroissantError::Io)
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn try_overwrite_execution_image(&self, bytes: &[u8]) -> io::Result<usize> {
        self.binary.sealed.try_overwrite(bytes)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one lifecycle owner binds descriptor launch through stable readiness"
    )]
    pub(crate) async fn start(&self) -> Result<CroissantProcess, CroissantError> {
        if self.binary.launched.swap(true, Ordering::AcqRel) {
            return Err(CroissantError::InvalidContract(
                "Croissant staged executable may be launched only once",
            ));
        }
        let mut command = opened_executable_command(&self.binary)?;
        let mut child = command
            .env_clear()
            .env("HOST", Ipv4Addr::LOCALHOST.to_string())
            .env("PORT", self.endpoint.port().to_string())
            .env("DATAPATH", &self.data_path)
            .env("DOMAIN", "")
            .env("OWNER_PUBLIC_KEY", &self.owner_public_key)
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
            if let Some(status) = child.try_wait()? {
                return finish_early_exit(
                    status.to_string(),
                    stdout_log,
                    stderr_log,
                    self.limits.teardown_ms,
                )
                .await;
            }
            if TcpStream::connect(self.endpoint).await.is_ok() {
                if let Some(status) = child.try_wait()? {
                    return finish_early_exit(
                        status.to_string(),
                        stdout_log,
                        stderr_log,
                        self.limits.teardown_ms,
                    )
                    .await;
                }
                tokio::time::sleep(Duration::from_millis(self.limits.readiness_stability_ms)).await;
                if let Some(status) = child.try_wait()? {
                    return finish_early_exit(
                        status.to_string(),
                        stdout_log,
                        stderr_log,
                        self.limits.teardown_ms,
                    )
                    .await;
                }
                if TcpStream::connect(self.endpoint).await.is_ok() {
                    if let Some(status) = child.try_wait()? {
                        return finish_early_exit(
                            status.to_string(),
                            stdout_log,
                            stderr_log,
                            self.limits.teardown_ms,
                        )
                        .await;
                    }
                    return Ok(CroissantProcess {
                        child,
                        ready: self.ready_fact(pid),
                        stdout_log,
                        stderr_log,
                        staged_executable: Arc::clone(&self.binary),
                    });
                }
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
        let (executable_device, executable_inode) = self.binary.execution_identity();
        CroissantReadyFact {
            executable: self.binary.path.clone(),
            executable_sha256: self.executable_sha256.clone(),
            executable_device,
            executable_inode,
            execution_platform: descriptor_execution_platform(),
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

#[cfg(target_os = "linux")]
#[allow(
    clippy::unnecessary_wraps,
    reason = "all platform implementations share one fail-closed result boundary"
)]
fn opened_executable_command(binary: &StagedExecutable) -> Result<Command, CroissantError> {
    let executable_stdin = binary.sealed.try_clone()?;
    let mut command = Command::new("/proc/self/fd/0");
    // The executable is both hashed and executed through this same opened object. The child file
    // action installs it as fd 0 before exec resolves /proc/self/fd/0.
    command.stdin(Stdio::from(executable_stdin));
    Ok(command)
}

#[cfg(all(target_os = "macos", test))]
#[allow(
    clippy::unnecessary_wraps,
    reason = "all platform implementations share one fail-closed result boundary"
)]
fn opened_executable_command(binary: &StagedExecutable) -> Result<Command, CroissantError> {
    let mut command = Command::new(&binary.path);
    command.stdin(Stdio::null());
    Ok(command)
}

#[cfg(not(any(target_os = "linux", all(target_os = "macos", test))))]
fn opened_executable_command(_binary: &StagedExecutable) -> Result<Command, CroissantError> {
    Err(CroissantError::InvalidContract(
        "descriptor-bound Croissant execution is unsupported on this host",
    ))
}

async fn finish_early_exit(
    status: String,
    stdout_log: BoundedLog,
    stderr_log: BoundedLog,
    teardown_ms: u64,
) -> Result<CroissantProcess, CroissantError> {
    stdout_log.finish(teardown_ms).await?;
    stderr_log.finish(teardown_ms).await?;
    Err(CroissantError::EarlyExit { status })
}

#[derive(Debug)]
pub(crate) struct CroissantProcess {
    child: Child,
    ready: CroissantReadyFact,
    stdout_log: BoundedLog,
    stderr_log: BoundedLog,
    staged_executable: Arc<StagedExecutable>,
}

#[derive(Debug)]
struct StagedExecutable {
    file: fs::File,
    #[cfg(target_os = "linux")]
    sealed: SealedExecutable,
    path: PathBuf,
    launched: AtomicBool,
    cleaned: AtomicBool,
    directory_device: u64,
    directory_inode: u64,
}

impl Drop for StagedExecutable {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

impl StagedExecutable {
    #[cfg(not(target_os = "linux"))]
    fn metadata(&self) -> io::Result<fs::Metadata> {
        self.file.metadata()
    }

    #[cfg(target_os = "linux")]
    #[allow(
        clippy::unnecessary_wraps,
        reason = "all platform implementations share one fallible digest boundary"
    )]
    fn sha256(&self) -> Result<String, CroissantError> {
        Ok(self.sealed.sha256().to_owned())
    }

    #[cfg(not(target_os = "linux"))]
    fn sha256(&self) -> Result<String, CroissantError> {
        const MAX_EXECUTABLE_BYTES: u64 = 134_217_728;
        let before = self.file.metadata()?;
        if !before.is_file() || before.len() > MAX_EXECUTABLE_BYTES {
            return Err(CroissantError::InvalidContract(
                "Croissant executable exceeded the 128-MiB bound",
            ));
        }
        let mut digest = Sha256::new();
        let mut offset = 0_u64;
        let mut buffer = [0_u8; 16_384];
        while offset < before.len() {
            let remaining = before.len() - offset;
            let wanted = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|_| CroissantError::InvalidContract("Croissant size did not fit host"))?;
            let read = self.file.read_at(&mut buffer[..wanted], offset)?;
            if read == 0 {
                return Err(CroissantError::InvalidContract(
                    "Croissant executable changed during descriptor hashing",
                ));
            }
            digest.update(&buffer[..read]);
            offset += u64::try_from(read)
                .map_err(|_| CroissantError::InvalidContract("Croissant read did not fit u64"))?;
        }
        let after = self.file.metadata()?;
        if before.dev() != after.dev() || before.ino() != after.ino() || before.len() != after.len()
        {
            return Err(CroissantError::InvalidContract(
                "Croissant executable changed during descriptor hashing",
            ));
        }
        Ok(hex::encode(digest.finalize()))
    }

    fn execution_identity(&self) -> (u64, u64) {
        #[cfg(target_os = "linux")]
        {
            (self.sealed.device(), self.sealed.inode())
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.metadata()
                .map(|item| (item.dev(), item.ino()))
                .unwrap_or((0, 0))
        }
    }

    fn cleanup(&self) -> io::Result<()> {
        if self.cleaned.load(Ordering::Acquire) {
            return Ok(());
        }
        let opened = self.file.metadata()?;
        let path = fs::symlink_metadata(&self.path)?;
        if path.file_type().is_symlink()
            || !path.is_file()
            || path.dev() != opened.dev()
            || path.ino() != opened.ino()
            || opened.nlink() != 1
        {
            return Err(io::Error::other(
                "staged executable path no longer names its opened object",
            ));
        }
        if let Some(parent) = self.path.parent() {
            let directory = fs::symlink_metadata(parent)?;
            if directory.file_type().is_symlink()
                || !directory.is_dir()
                || directory.dev() != self.directory_device
                || directory.ino() != self.directory_inode
            {
                return Err(io::Error::other(
                    "staged executable directory changed before cleanup",
                ));
            }
            fs::remove_file(&self.path)?;
            if self.file.metadata()?.nlink() != 0 {
                return Err(io::Error::other(
                    "staged executable retained an unexpected hard link",
                ));
            }
            fs::remove_dir(parent)?;
        } else {
            return Err(io::Error::other(
                "staged executable path omitted its owned directory",
            ));
        }
        self.cleaned.store(true, Ordering::Release);
        Ok(())
    }
}

fn descriptor_execution_platform() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "linux-sealed-memfd-proc-fd"
    }
    #[cfg(all(target_os = "macos", test))]
    {
        "darwin-test-only-path"
    }
    #[cfg(not(any(target_os = "linux", all(target_os = "macos", test))))]
    {
        "unsupported"
    }
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
        let terminal_failure = if stdout.overflowed {
            Some(CroissantError::LogOverflow {
                stream: "stdout",
                limit: self.ready.limits.log_bytes,
            })
        } else if stderr.overflowed {
            Some(CroissantError::LogOverflow {
                stream: "stderr",
                limit: self.ready.limits.log_bytes,
            })
        } else {
            None
        };
        let pid_alive_after = process_is_alive(pid);
        let port_open_after = TcpStream::connect(endpoint).await.is_ok();
        let terminal_failure = terminal_failure.or_else(|| {
            (pid_alive_after || port_open_after).then_some(CroissantError::TeardownFailure {
                pid,
                reason: "pid or loopback port remained live",
            })
        });
        self.staged_executable
            .cleanup()
            .map_err(|_| CroissantError::TeardownFailure {
                pid,
                reason: "staged executable cleanup failed",
            })?;
        if let Some(failure) = terminal_failure {
            return Err(failure);
        }
        Ok(CroissantTeardown {
            pid,
            endpoint,
            completed: true,
            pid_alive_after,
            port_open_after,
            executable_removed: true,
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
