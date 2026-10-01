//! Dataset files: `data/<version>/samples.jsonl` and its `manifest.json`.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use parda_spec::sample::Sample;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SAMPLES_FILE: &str = "samples.jsonl";
pub const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, thiserror::Error)]
pub enum DataIoError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}:{line}: {source}")]
    Json {
        path: PathBuf,
        line: usize,
        source: serde_json::Error,
    },
}

/// `manifest.json`: how a dataset was built and the digest of its samples file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetManifest {
    pub version: String,
    pub seed: u64,
    pub per_template: usize,
    pub generator_version: String,
    pub sample_count: usize,
    pub samples_sha256: String,
}

/// Writes samples and manifest into `dir`, creating it.
///
/// # Errors
/// Fails on any filesystem error.
pub fn write_dataset(
    dir: &Path,
    samples: &[Sample],
    manifest: &DatasetManifest,
) -> Result<(), DataIoError> {
    fs::create_dir_all(dir).map_err(|source| DataIoError::Io {
        path: dir.to_owned(),
        source,
    })?;
    let samples_path = dir.join(SAMPLES_FILE);
    let mut body = Vec::new();
    for (i, sample) in samples.iter().enumerate() {
        let line = serde_json::to_vec(sample).map_err(|source| DataIoError::Json {
            path: samples_path.clone(),
            line: i + 1,
            source,
        })?;
        body.extend_from_slice(&line);
        body.push(b'\n');
    }
    write_file(&samples_path, &body)?;
    let mut json = serde_json::to_vec_pretty(manifest).map_err(|source| DataIoError::Json {
        path: dir.join(MANIFEST_FILE),
        line: 0,
        source,
    })?;
    json.push(b'\n');
    write_file(&dir.join(MANIFEST_FILE), &json)
}

/// SHA-256 of `samples` serialized exactly as `write_dataset` writes them.
///
/// # Errors
/// Fails only if a sample cannot be serialized.
pub fn samples_sha256(samples: &[Sample]) -> Result<String, serde_json::Error> {
    let mut hasher = Sha256::new();
    for sample in samples {
        hasher.update(serde_json::to_vec(sample)?);
        hasher.update(b"\n");
    }
    Ok(hex::encode(hasher.finalize()))
}

/// SHA-256 of a file's bytes.
///
/// # Errors
/// Fails if the file cannot be read.
pub fn file_sha256(path: &Path) -> Result<String, DataIoError> {
    let bytes = fs::read(path).map_err(|source| DataIoError::Io {
        path: path.to_owned(),
        source,
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

/// Reads a JSONL samples file.
///
/// # Errors
/// Fails on I/O errors or a line that is not a valid `Sample`.
pub fn read_samples(path: &Path) -> Result<Vec<Sample>, DataIoError> {
    let file = fs::File::open(path).map_err(|source| DataIoError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut samples = Vec::new();
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|source| DataIoError::Io {
            path: path.to_owned(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let sample = serde_json::from_str(&line).map_err(|source| DataIoError::Json {
            path: path.to_owned(),
            line: i + 1,
            source,
        })?;
        samples.push(sample);
    }
    Ok(samples)
}

/// Reads a dataset directory's manifest.
///
/// # Errors
/// Fails if `manifest.json` is missing or malformed.
pub fn read_manifest(dir: &Path) -> Result<DatasetManifest, DataIoError> {
    let path = dir.join(MANIFEST_FILE);
    let bytes = fs::read(&path).map_err(|source| DataIoError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| DataIoError::Json {
        path,
        line: 0,
        source,
    })
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), DataIoError> {
    let mut file = fs::File::create(path).map_err(|source| DataIoError::Io {
        path: path.to_owned(),
        source,
    })?;
    file.write_all(bytes).map_err(|source| DataIoError::Io {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{build, builtin_templates};

    #[test]
    fn written_dataset_reads_back_and_matches_its_digest() {
        let dir = tempfile::tempdir().unwrap();
        let samples = build(&builtin_templates().unwrap(), 1, 2);
        let digest = samples_sha256(&samples).unwrap();
        let manifest = DatasetManifest {
            version: "test".to_owned(),
            seed: 1,
            per_template: 2,
            generator_version: crate::GENERATOR_VERSION.to_owned(),
            sample_count: samples.len(),
            samples_sha256: digest.clone(),
        };
        write_dataset(dir.path(), &samples, &manifest).unwrap();
        let back = read_samples(&dir.path().join(SAMPLES_FILE)).unwrap();
        assert_eq!(back, samples);
        assert_eq!(file_sha256(&dir.path().join(SAMPLES_FILE)).unwrap(), digest);
        assert_eq!(read_manifest(dir.path()).unwrap(), manifest);
    }

    #[test]
    fn malformed_line_reports_its_line_number() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SAMPLES_FILE);
        fs::write(&path, "\n{not json}\n").unwrap();
        let err = read_samples(&path)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(err.contains(":2:"), "{err}");
    }
}
