//! Tools with their own HTTP service: each adapter request is sent as a JSON POST to `base_url`, and the body of the
//! reply is the adapter response.

use std::time::{Duration, Instant};

use crate::adapter::{Reply, accept};
use crate::error::AdapterError;
use crate::process::GroupChild;
use parda_spec::protocol::adapter::{AdapterOp, AdapterRequest, AdapterResponse};

pub struct HttpApiTool {
    /// Held so the service is killed when the tool is dropped.
    _process: GroupChild,
    client: reqwest::Client,
    base_url: String,
    next_id: u64,
}

impl HttpApiTool {
    #[must_use]
    pub fn new(process: GroupChild, base_url: String) -> Self {
        Self {
            _process: process,
            client: reqwest::Client::new(),
            base_url,
            next_id: 1,
        }
    }

    /// POSTs one request and waits up to `timeout` for the reply.
    ///
    /// # Errors
    /// Fails on connection errors, timeout, a non-JSON or mismatched reply, or a tool-reported error.
    pub async fn call(&mut self, op: AdapterOp, timeout: Duration) -> Result<Reply, AdapterError> {
        let id = self.next_id;
        self.next_id += 1;
        let http = |source| AdapterError::Http {
            url: self.base_url.clone(),
            source,
        };
        let started = Instant::now();
        let response = self
            .client
            .post(&self.base_url)
            .json(&AdapterRequest { id, op })
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    AdapterError::Timeout {
                        request_id: id,
                        timeout,
                    }
                } else {
                    http(e)
                }
            })?;
        let body = response.text().await.map_err(http)?;
        let wall = started.elapsed();
        let parsed: AdapterResponse =
            serde_json::from_str(&body).map_err(|source| AdapterError::BadResponse {
                line: body.clone(),
                source,
            })?;
        accept(id, parsed, wall)
    }
}
