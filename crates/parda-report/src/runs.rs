//! Reading `results/<run-id>/` directories and the datasets detection runs refer to.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use parda_spec::record::{DetectionRecord, RestoreRecord, RunMeta, Suite};
use parda_spec::sample::Sample;
use serde::de::DeserializeOwned;

pub const META_FILE: &str = "meta.json";
pub const DETECTIONS_FILE: &str = "detections.jsonl";
pub const RESTORE_FILE: &str = "restore.jsonl";

#[derive(Debug, thiserror::Error)]
pub enum RunsError {
    #[error("{path}: {message}")]
    File { path: PathBuf, message: String },
    #[error("run {run_id} used dataset {version} with sha256 {expected}, but {path} has {found}")]
    DatasetMismatch {
        run_id: String,
        version: String,
        expected: String,
        found: String,
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunBody {
    Detection(Vec<DetectionRecord>),
    Restore(Vec<RestoreRecord>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub meta: RunMeta,
    pub body: RunBody,
}

/// Every run directory under `results`, oldest first.
///
/// # Errors
/// Fails on unreadable directories or malformed run files.
pub fn load_runs(results: &Path) -> Result<Vec<Run>, RunsError> {
    let mut runs = Vec::new();
    let entries = fs::read_dir(results).map_err(|e| file_err(results, &e))?;
    for entry in entries {
        let dir = entry.map_err(|e| file_err(results, &e))?.path();
        if !dir.join(META_FILE).is_file() {
            continue;
        }
        let meta: RunMeta = read_json(&dir.join(META_FILE))?;
        let body = match meta.suite {
            Suite::Detection { .. } => RunBody::Detection(read_jsonl(&dir.join(DETECTIONS_FILE))?),
            Suite::Restore { .. } => RunBody::Restore(read_jsonl(&dir.join(RESTORE_FILE))?),
        };
        runs.push(Run { meta, body });
    }
    runs.sort_by(|a, b| {
        a.meta
            .started_at
            .cmp(&b.meta.started_at)
            .then(a.meta.run_id.cmp(&b.meta.run_id))
    });
    Ok(runs)
}

/// The samples of `data/<version>/`, checked against the digest the run recorded.
///
/// # Errors
/// Fails if the dataset is missing, unreadable, or not byte-identical to the one the run used.
pub fn load_dataset_for(data: &Path, meta: &RunMeta) -> Result<Vec<Sample>, RunsError> {
    let Suite::Detection {
        dataset_version,
        dataset_sha256,
    } = &meta.suite
    else {
        return Ok(Vec::new());
    };
    let path = data
        .join(dataset_version)
        .join(parda_data::io::SAMPLES_FILE);
    let found = parda_data::io::file_sha256(&path).map_err(|e| RunsError::File {
        path: path.clone(),
        message: e.to_string(),
    })?;
    if &found != dataset_sha256 {
        return Err(RunsError::DatasetMismatch {
            run_id: meta.run_id.clone(),
            version: dataset_version.clone(),
            expected: dataset_sha256.clone(),
            found,
            path,
        });
    }
    parda_data::io::read_samples(&path).map_err(|e| RunsError::File {
        path,
        message: e.to_string(),
    })
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, RunsError> {
    let bytes = fs::read(path).map_err(|e| file_err(path, &e))?;
    serde_json::from_slice(&bytes).map_err(|e| file_err(path, &e))
}

fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, RunsError> {
    let file = fs::File::open(path).map_err(|e| file_err(path, &e))?;
    let mut out = Vec::new();
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| file_err(path, &e))?;
        if !line.trim().is_empty() {
            out.push(serde_json::from_str(&line).map_err(|e| RunsError::File {
                path: path.to_owned(),
                message: format!("line {}: {e}", i + 1),
            })?);
        }
    }
    Ok(out)
}

fn file_err(path: &Path, e: &dyn std::fmt::Display) -> RunsError {
    RunsError::File {
        path: path.to_owned(),
        message: e.to_string(),
    }
}
