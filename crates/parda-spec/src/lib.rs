//! Data contracts shared by every Parda Bench component; the JSON Schemas in `spec/` are generated from these types.

pub mod entity;
pub mod manifest;
pub mod protocol;
pub mod record;
pub mod sample;
pub mod scenario;

use schemars::{Schema, schema_for};

/// Every published contract, keyed by the file stem used under `spec/`.
#[must_use]
pub fn schemas() -> Vec<(&'static str, Schema)> {
    vec![
        ("sample", schema_for!(sample::Sample)),
        ("tool-manifest", schema_for!(manifest::ToolManifest)),
        (
            "adapter-request",
            schema_for!(protocol::adapter::AdapterRequest),
        ),
        (
            "adapter-response",
            schema_for!(protocol::adapter::AdapterResponse),
        ),
        (
            "driver-request",
            schema_for!(protocol::driver::DriverRequest),
        ),
        (
            "driver-response",
            schema_for!(protocol::driver::DriverResponse),
        ),
        ("scenario", schema_for!(scenario::Scenario)),
        ("detection-record", schema_for!(record::DetectionRecord)),
        ("restore-record", schema_for!(record::RestoreRecord)),
        ("run-meta", schema_for!(record::RunMeta)),
    ]
}
