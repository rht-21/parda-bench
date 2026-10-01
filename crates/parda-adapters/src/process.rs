//! Starting tool processes and waiting for services to listen.
//!
//! Each process runs in its own process group, and the whole group is killed when its handle is dropped:
//! launchers such as `uv run` keep the real tool as a child that would otherwise outlive them.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::{Instant, sleep};

use crate::error::AdapterError;

const READY_POLL: Duration = Duration::from_millis(100);

/// Where a tool's stderr (and a service's stdout) goes.
#[derive(Debug, Clone)]
pub enum Output {
    Inherit,
    /// Appended to this file.
    Log(PathBuf),
}

/// A child process and everything it started; the group is killed on drop.
pub struct GroupChild {
    child: Child,
}

impl GroupChild {
    pub fn child(&mut self) -> &mut Child {
        &mut self.child
    }

    /// Sends SIGKILL to the whole process group.
    pub fn kill_group(&mut self) {
        let Some(pid) = self.child.id().and_then(|id| i32::try_from(id).ok()) else {
            return;
        };
        match killpg(Pid::from_raw(pid), Signal::SIGKILL) {
            Ok(()) | Err(Errno::ESRCH) => {}
            Err(e) => tracing::warn!("killing process group {pid}: {e}"),
        }
    }
}

impl Drop for GroupChild {
    fn drop(&mut self) {
        self.kill_group();
    }
}

/// Starts a stdio worker: stdin and stdout are piped for the protocol.
///
/// # Errors
/// Fails if the log file cannot be opened or the command cannot be started.
pub fn spawn_worker(
    dir: &Path,
    command: &[String],
    env: &BTreeMap<String, String>,
    stderr: &Output,
) -> Result<GroupChild, AdapterError> {
    let mut cmd = base_command(dir, command, env)?;
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(open(stderr)?);
    spawn(cmd, dir, command)
}

/// Starts a long-running service (HTTP proxy or API) with both output streams sent to `output`.
///
/// # Errors
/// Fails if the log file cannot be opened or the command cannot be started.
pub fn spawn_service(
    dir: &Path,
    command: &[String],
    env: &BTreeMap<String, String>,
    output: &Output,
) -> Result<GroupChild, AdapterError> {
    let mut cmd = base_command(dir, command, env)?;
    cmd.stdin(Stdio::null())
        .stdout(open(output)?)
        .stderr(open(output)?);
    spawn(cmd, dir, command)
}

fn spawn(mut cmd: Command, dir: &Path, command: &[String]) -> Result<GroupChild, AdapterError> {
    let child = cmd
        .spawn()
        .map_err(|source| spawn_error(dir, command, source))?;
    Ok(GroupChild { child })
}

/// Polls until `base_url`'s host and port accept a TCP connection.
///
/// # Errors
/// Fails if the URL has no host or port, the process exits first, or `timeout` passes.
pub async fn wait_until_listening(
    base_url: &str,
    process: &mut GroupChild,
    timeout: Duration,
) -> Result<(), AdapterError> {
    let not_ready = || AdapterError::NotReady {
        url: base_url.to_owned(),
        timeout,
    };
    let url = reqwest::Url::parse(base_url).map_err(|_| not_ready())?;
    let host = url.host_str().ok_or_else(not_ready)?.to_owned();
    let port = url.port_or_known_default().ok_or_else(not_ready)?;
    let deadline = Instant::now() + timeout;
    loop {
        if TcpStream::connect((host.as_str(), port)).await.is_ok() {
            return Ok(());
        }
        if let Some(status) = process.child().try_wait()? {
            return Err(AdapterError::Io(std::io::Error::other(format!(
                "service exited with {status} before listening on {base_url}"
            ))));
        }
        if Instant::now() >= deadline {
            return Err(not_ready());
        }
        sleep(READY_POLL).await;
    }
}

fn base_command(
    dir: &Path,
    command: &[String],
    env: &BTreeMap<String, String>,
) -> Result<Command, AdapterError> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| spawn_error(dir, command, std::io::Error::other("empty command")))?;
    // A relative program path like `./run.sh` would otherwise resolve against the harness's working directory.
    let program = if program.contains('/') && Path::new(program).is_relative() {
        dir.join(program).into_os_string()
    } else {
        program.into()
    };
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(dir).envs(env).process_group(0);
    Ok(cmd)
}

fn open(output: &Output) -> Result<Stdio, AdapterError> {
    match output {
        Output::Inherit => Ok(Stdio::inherit()),
        Output::Log(path) => {
            let file = File::options().create(true).append(true).open(path)?;
            Ok(Stdio::from(file))
        }
    }
}

fn spawn_error(dir: &Path, command: &[String], source: std::io::Error) -> AdapterError {
    AdapterError::Spawn {
        command: command.join(" "),
        dir: dir.to_owned(),
        source,
    }
}
