//! A run's directory: `meta.json`, one JSONL record file, and the tool's log.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use parda_adapters::Tool;
use parda_spec::record::{RunMeta, Suite, ToolRef};
use serde::Serialize;

use crate::hardware;

pub struct RunDir {
    path: PathBuf,
    run_id: String,
    records: BufWriter<File>,
}

impl RunDir {
    pub fn create(results: &Path, tool: &Tool, suite: Suite, records_file: &str) -> Result<Self> {
        let tool_ref = ToolRef {
            name: tool.manifest.name.clone(),
            version: tool.manifest.version.clone(),
            commit: tool.manifest.commit.clone(),
            config: tool.config_name.clone(),
        };
        Self::create_for(results, tool_ref, suite, records_file)
    }

    /// A restore run with no tool between the driver and the mock.
    pub fn create_control(results: &Path, driver: &str, records_file: &str) -> Result<Self> {
        let tool_ref = ToolRef {
            name: parda_spec::record::CONTROL_TOOL.to_owned(),
            version: "-".to_owned(),
            commit: "-".to_owned(),
            config: "default".to_owned(),
        };
        Self::create_for(
            results,
            tool_ref,
            Suite::Restore {
                driver: driver.to_owned(),
            },
            records_file,
        )
    }

    fn create_for(results: &Path, tool: ToolRef, suite: Suite, records_file: &str) -> Result<Self> {
        let started_at = humantime::format_rfc3339_seconds(SystemTime::now()).to_string();
        let target = match &suite {
            Suite::Detection { .. } => "detect".to_owned(),
            Suite::Restore { driver } => format!("restore-{driver}"),
        };
        let stamp: String = started_at
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        let run_id = format!("{stamp}-{}-{}-{target}", tool.name, tool.config);
        let path = results.join(&run_id);
        fs::create_dir_all(results).with_context(|| format!("creating {}", results.display()))?;
        // A run directory is never reused: records from two runs must not mix.
        fs::create_dir(&path).with_context(|| {
            format!(
                "creating {} (a run with this id already exists; retry in a second)",
                path.display()
            )
        })?;
        let meta = RunMeta {
            run_id: run_id.clone(),
            started_at,
            harness_version: env!("CARGO_PKG_VERSION").to_owned(),
            tool,
            hardware: hardware::probe(),
            suite,
        };
        let mut json = serde_json::to_vec_pretty(&meta)?;
        json.push(b'\n');
        fs::write(path.join(parda_report::runs::META_FILE), json)?;
        let file = File::create(path.join(records_file))?;
        Ok(Self {
            path,
            run_id,
            records: BufWriter::new(file),
        })
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn tool_log(&self) -> PathBuf {
        self.path.join("tool.log")
    }

    /// Appends one record and flushes, so an interrupted run keeps everything recorded so far.
    pub fn append<T: Serialize>(&mut self, record: &T) -> Result<()> {
        serde_json::to_writer(&mut self.records, record)?;
        self.records.write_all(b"\n")?;
        self.records.flush()?;
        Ok(())
    }
}
