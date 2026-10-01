//! Out-of-process driver speaking `driver-request` / `driver-response` NDJSON on stdin/stdout.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use parda_adapters::process::{GroupChild, Output, spawn_worker};
use parda_spec::protocol::driver::{DriverRequest, DriverResponse};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout};

use crate::driver::DriverError;

/// A whole conversation; turns are individually bounded by the driver's own HTTP timeouts.
const CONVERSATION_TIMEOUT: Duration = Duration::from_secs(300);

pub struct StdioDriver {
    _process: GroupChild,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

impl StdioDriver {
    /// # Errors
    /// Fails if the process cannot start.
    pub fn start(dir: &Path, command: &[String]) -> Result<Self, DriverError> {
        let mut process = spawn_worker(dir, command, &BTreeMap::new(), &Output::Inherit)?;
        let stdin = process.child().stdin.take().ok_or(DriverError::Exited)?;
        let stdout = process.child().stdout.take().ok_or(DriverError::Exited)?;
        Ok(Self {
            _process: process,
            stdin,
            stdout: BufReader::new(stdout).lines(),
        })
    }

    /// # Errors
    /// Fails if the driver process dies, hangs, or answers with something other than a matching response.
    pub async fn run(&mut self, request: &DriverRequest) -> Result<DriverResponse, DriverError> {
        let mut line = serde_json::to_vec(request).map_err(std::io::Error::other)?;
        line.push(b'\n');
        self.stdin.write_all(&line).await?;
        self.stdin.flush().await?;
        let reply = tokio::time::timeout(CONVERSATION_TIMEOUT, self.stdout.next_line())
            .await
            .map_err(|_| DriverError::Timeout(CONVERSATION_TIMEOUT))??
            .ok_or(DriverError::Exited)?;
        let response: DriverResponse =
            serde_json::from_str(&reply).map_err(|source| DriverError::BadResponse {
                line: reply.clone(),
                source,
            })?;
        if response.id == request.id {
            Ok(response)
        } else {
            Err(DriverError::BadResponse {
                line: reply,
                source: serde::de::Error::custom(format!(
                    "id {} does not match request {}",
                    response.id, request.id
                )),
            })
        }
    }
}
