//! Long-lived NDJSON worker on stdin/stdout.

use std::time::{Duration, Instant};

use parda_spec::protocol::adapter::{AdapterOp, AdapterRequest, AdapterResponse};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout};

use crate::adapter::{Reply, accept};
use crate::error::AdapterError;
use crate::process::GroupChild;

const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

pub struct StdioWorker {
    process: GroupChild,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl StdioWorker {
    /// Wraps a process started by `process::spawn_worker`.
    ///
    /// # Errors
    /// Fails if the child's stdin or stdout was not piped.
    pub fn new(mut process: GroupChild) -> Result<Self, AdapterError> {
        let missing =
            |s: &str| AdapterError::Io(std::io::Error::other(format!("worker {s} is not piped")));
        let stdin = process
            .child()
            .stdin
            .take()
            .ok_or_else(|| missing("stdin"))?;
        let stdout = process
            .child()
            .stdout
            .take()
            .ok_or_else(|| missing("stdout"))?;
        Ok(Self {
            process,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            next_id: 1,
        })
    }

    /// Sends one request and waits up to `timeout` for its reply.
    ///
    /// # Errors
    /// Fails on I/O errors, timeout, a malformed or mismatched reply, or a tool-reported error.
    pub async fn call(&mut self, op: AdapterOp, timeout: Duration) -> Result<Reply, AdapterError> {
        let id = self.next_id;
        self.next_id += 1;
        let mut line = serde_json::to_vec(&AdapterRequest { id, op })
            .map_err(|e| AdapterError::Io(std::io::Error::other(e)))?;
        line.push(b'\n');
        let started = Instant::now();
        self.stdin.write_all(&line).await?;
        self.stdin.flush().await?;
        let reply_line = tokio::time::timeout(timeout, self.stdout.next_line())
            .await
            .map_err(|_| AdapterError::Timeout {
                request_id: id,
                timeout,
            })??
            .ok_or(AdapterError::WorkerExited { request_id: id })?;
        let wall = started.elapsed();
        let response: AdapterResponse =
            serde_json::from_str(&reply_line).map_err(|source| AdapterError::BadResponse {
                line: reply_line.clone(),
                source,
            })?;
        accept(id, response, wall)
    }

    /// Asks the worker to exit, then kills its process group if it has not exited within a grace period.
    pub async fn shutdown(mut self) {
        if let Err(e) = self.call(AdapterOp::Shutdown, SHUTDOWN_GRACE).await {
            tracing::debug!("worker did not acknowledge shutdown: {e}");
        }
        drop(self.stdin);
        if tokio::time::timeout(SHUTDOWN_GRACE, self.process.child().wait())
            .await
            .is_err()
        {
            self.process.terminate().await;
        }
    }
}
