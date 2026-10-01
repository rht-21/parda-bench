//! Client-framework drivers: run a scripted conversation against a base URL and report what the app saw.

pub mod raw_http;
pub mod registry;
pub mod sse;
pub mod stdio;

use std::path::Path;

use parda_spec::protocol::driver::{DriverRequest, DriverResponse};
use parda_spec::scenario::Api;

use crate::driver::raw_http::RawHttpDriver;
use crate::driver::registry::DriverSpec;
use crate::driver::stdio::StdioDriver;

pub const RAW_HTTP: &str = "raw_http";

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("unknown driver `{name}`; available: {available:?}")]
    Unknown {
        name: String,
        available: Vec<String>,
    },
    #[error("reading driver registry {path}: {message}")]
    Registry { path: String, message: String },
    #[error("driver process: {0}")]
    Process(#[from] parda_adapters::AdapterError),
    #[error("driver I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("driver exited without answering")]
    Exited,
    #[error("driver did not finish the conversation within {0:?}")]
    Timeout(std::time::Duration),
    #[error("driver sent an invalid response ({source}): {line}")]
    BadResponse {
        line: String,
        source: serde_json::Error,
    },
}

pub enum Driver {
    RawHttp(RawHttpDriver),
    Stdio {
        name: String,
        apis: Vec<Api>,
        worker: Box<StdioDriver>,
    },
}

impl Driver {
    /// Opens `raw_http` (built in) or a driver listed in `drivers_dir/drivers.toml`.
    ///
    /// # Errors
    /// Fails if the driver is unknown or its process cannot start.
    pub fn open(name: &str, drivers_dir: &Path) -> Result<Self, DriverError> {
        if name == RAW_HTTP {
            return Ok(Self::RawHttp(RawHttpDriver::new()));
        }
        let specs = registry::load(drivers_dir)?;
        let DriverSpec { command, apis } =
            specs
                .get(name)
                .cloned()
                .ok_or_else(|| DriverError::Unknown {
                    name: name.to_owned(),
                    available: std::iter::once(RAW_HTTP.to_owned())
                        .chain(specs.keys().cloned())
                        .collect(),
                })?;
        let worker = StdioDriver::start(drivers_dir, &command)?;
        Ok(Self::Stdio {
            name: name.to_owned(),
            apis,
            worker: Box::new(worker),
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::RawHttp(_) => RAW_HTTP,
            Self::Stdio { name, .. } => name,
        }
    }

    #[must_use]
    pub fn supports(&self, api: Api) -> bool {
        match self {
            Self::RawHttp(_) => true,
            Self::Stdio { apis, .. } => apis.contains(&api),
        }
    }

    /// Runs one conversation.
    ///
    /// # Errors
    /// Fails only when the driver itself breaks; HTTP and API errors are reported as observations.
    pub async fn run(&mut self, request: &DriverRequest) -> Result<DriverResponse, DriverError> {
        match self {
            Self::RawHttp(d) => Ok(d.run(request).await),
            Self::Stdio { worker, .. } => worker.run(request).await,
        }
    }
}
