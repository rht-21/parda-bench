//! One running tool, whatever its transport, with typed helpers for each adapter operation.

use std::collections::BTreeSet;
use std::time::Duration;

use parda_spec::manifest::{Capability, Transport};
use parda_spec::protocol::adapter::{
    AdapterOp, AdapterOutcome, AdapterResponse, AdapterResult, DetectedEntity,
};

use crate::error::AdapterError;
use crate::http_api::HttpApiTool;
use crate::process::{Output, spawn_service, spawn_worker, wait_until_listening};
use crate::stdio::StdioWorker;
use crate::tool::Tool;

/// Tools may load large models at startup.
pub const STARTUP_TIMEOUT: Duration = Duration::from_secs(600);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// A successful reply with both latency measurements.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub result: AdapterResult,
    /// Measured inside the tool.
    pub elapsed_ns: u64,
    /// Measured by the harness, including IPC.
    pub wall_ns: u64,
}

pub enum Adapter {
    Stdio(StdioWorker),
    HttpApi(HttpApiTool),
}

impl Adapter {
    /// Starts the tool, performs the `hello` handshake and checks the capabilities it reports against the manifest.
    ///
    /// # Errors
    /// Fails if the tool cannot start, does not answer `hello`, reports different capabilities, or is a proxy
    /// (proxies are driven through `process::spawn_service` by the restore suite).
    pub async fn start(tool: &Tool, output: &Output) -> Result<Self, AdapterError> {
        let env = tool.env();
        let mut adapter = match &tool.manifest.transport {
            Transport::Stdio { command } => Self::Stdio(StdioWorker::new(spawn_worker(
                &tool.dir, command, &env, output,
            )?)?),
            Transport::HttpApi { start, base_url } => {
                let mut process = spawn_service(&tool.dir, start, &env, output)?;
                wait_until_listening(base_url, &mut process, STARTUP_TIMEOUT).await?;
                Self::HttpApi(HttpApiTool::new(process, base_url.clone()))
            }
            Transport::HttpProxy { .. } => return Err(AdapterError::WrongTransport("http-proxy")),
        };
        let reply = adapter.call(AdapterOp::Hello, STARTUP_TIMEOUT).await?;
        let AdapterResult::Hello { capabilities, .. } = reply.result else {
            return Err(op_mismatch("hello", &reply.result));
        };
        check_capabilities(&tool.manifest.capabilities, &capabilities)?;
        Ok(adapter)
    }

    /// Sends one operation with the given timeout.
    ///
    /// # Errors
    /// Any transport, protocol or tool error.
    pub async fn call(&mut self, op: AdapterOp, timeout: Duration) -> Result<Reply, AdapterError> {
        match self {
            Self::Stdio(w) => w.call(op, timeout).await,
            Self::HttpApi(h) => h.call(op, timeout).await,
        }
    }

    /// # Errors
    /// Any transport, protocol or tool error.
    pub async fn detect(
        &mut self,
        text: &str,
    ) -> Result<(Vec<DetectedEntity>, Reply), AdapterError> {
        let reply = self
            .call(
                AdapterOp::Detect {
                    text: text.to_owned(),
                },
                REQUEST_TIMEOUT,
            )
            .await?;
        match &reply.result {
            AdapterResult::Detect { entities } => Ok((entities.clone(), reply)),
            other => Err(op_mismatch("detect", other)),
        }
    }

    /// # Errors
    /// Any transport, protocol or tool error.
    pub async fn mask(&mut self, session: &str, text: &str) -> Result<String, AdapterError> {
        let op = AdapterOp::Mask {
            session: session.to_owned(),
            text: text.to_owned(),
        };
        match self.call(op, REQUEST_TIMEOUT).await?.result {
            AdapterResult::Mask { text } => Ok(text),
            other => Err(op_mismatch("mask", &other)),
        }
    }

    /// # Errors
    /// Any transport, protocol or tool error.
    pub async fn unmask(&mut self, session: &str, text: &str) -> Result<String, AdapterError> {
        let op = AdapterOp::Unmask {
            session: session.to_owned(),
            text: text.to_owned(),
        };
        match self.call(op, REQUEST_TIMEOUT).await?.result {
            AdapterResult::Unmask { text } => Ok(text),
            other => Err(op_mismatch("unmask", &other)),
        }
    }

    /// # Errors
    /// Any transport, protocol or tool error.
    pub async fn unmask_stream(
        &mut self,
        session: &str,
        chunk: &str,
        is_final: bool,
    ) -> Result<String, AdapterError> {
        let op = AdapterOp::UnmaskStream {
            session: session.to_owned(),
            chunk: chunk.to_owned(),
            is_final,
        };
        match self.call(op, REQUEST_TIMEOUT).await?.result {
            AdapterResult::UnmaskStream { text } => Ok(text),
            other => Err(op_mismatch("unmask_stream", &other)),
        }
    }

    /// Stops the tool; a stdio worker is asked to exit first.
    pub async fn shutdown(self) {
        match self {
            Self::Stdio(w) => w.shutdown().await,
            Self::HttpApi(h) => h.terminate().await,
        }
    }
}

/// Checks a response belongs to request `id` and converts a tool error into `AdapterError::Tool`.
pub(crate) fn accept(
    id: u64,
    response: AdapterResponse,
    wall: Duration,
) -> Result<Reply, AdapterError> {
    if response.id != id {
        return Err(AdapterError::IdMismatch {
            expected: id,
            got: response.id,
        });
    }
    match response.outcome {
        AdapterOutcome::Ok { elapsed_ns, result } => Ok(Reply {
            result,
            elapsed_ns,
            wall_ns: u64::try_from(wall.as_nanos()).unwrap_or(u64::MAX),
        }),
        AdapterOutcome::Error { message } => Err(AdapterError::Tool(message)),
    }
}

fn check_capabilities(
    declared: &[Capability],
    reported: &[Capability],
) -> Result<(), AdapterError> {
    let declared: BTreeSet<Capability> = declared.iter().copied().collect();
    let reported: BTreeSet<Capability> = reported.iter().copied().collect();
    if declared == reported {
        Ok(())
    } else {
        Err(AdapterError::CapabilityMismatch { declared, reported })
    }
}

fn op_mismatch(expected: &'static str, got: &AdapterResult) -> AdapterError {
    AdapterError::OpMismatch {
        expected,
        got: format!("{got:?}"),
    }
}
