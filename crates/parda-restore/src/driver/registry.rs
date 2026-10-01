//! `drivers/drivers.toml`: how to start each out-of-process driver and which APIs it speaks.

use std::collections::BTreeMap;
use std::path::Path;

use parda_spec::scenario::Api;
use serde::Deserialize;

use crate::driver::DriverError;

pub const REGISTRY_FILE: &str = "drivers.toml";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverSpec {
    /// Run with the drivers directory as working directory.
    pub command: Vec<String>,
    pub apis: Vec<Api>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    drivers: BTreeMap<String, DriverSpec>,
}

/// # Errors
/// Fails if the registry is missing or malformed.
pub fn load(drivers_dir: &Path) -> Result<BTreeMap<String, DriverSpec>, DriverError> {
    let path = drivers_dir.join(REGISTRY_FILE);
    let err = |message: String| DriverError::Registry {
        path: path.display().to_string(),
        message,
    };
    let source = std::fs::read_to_string(&path).map_err(|e| err(e.to_string()))?;
    let registry: Registry = toml::from_str(&source).map_err(|e| err(e.to_string()))?;
    Ok(registry.drivers)
}
