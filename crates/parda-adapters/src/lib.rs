//! Transports to benchmarked tools: stdio NDJSON workers, HTTP proxies and HTTP APIs.

pub mod adapter;
pub mod detect;
pub mod error;
pub mod http_api;
pub mod process;
pub mod stdio;
pub mod tool;

pub use adapter::{Adapter, Reply};
pub use error::AdapterError;
pub use tool::Tool;
