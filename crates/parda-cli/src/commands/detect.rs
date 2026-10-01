use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use parda_adapters::Tool;
use parda_adapters::detect::DetectionRunner;
use parda_adapters::process::Output;
use parda_data::io::{SAMPLES_FILE, read_manifest, read_samples};
use parda_eval::detection::evaluate;
use parda_eval::matching::MatchMode;
use parda_report::runs::DETECTIONS_FILE;
use parda_spec::record::Suite;

use crate::results::RunDir;

#[derive(Debug, Args)]
pub struct DetectArgs {
    /// Tool directory containing `tool.toml`.
    #[arg(long)]
    tool: PathBuf,
    #[arg(long, default_value = "default")]
    config: String,
    /// Dataset directory, e.g. `data/v0.1.0`.
    #[arg(long)]
    dataset: PathBuf,
    /// Only the first N samples, for a quick check.
    #[arg(long)]
    limit: Option<usize>,
    #[arg(long, default_value = "results")]
    results: PathBuf,
}

const PROGRESS_EVERY: usize = 250;

pub async fn run(args: &DetectArgs) -> Result<()> {
    let tool = Tool::load(&args.tool, &args.config)?;
    let manifest = read_manifest(&args.dataset)?;
    let mut samples = read_samples(&args.dataset.join(SAMPLES_FILE))?;
    if let Some(limit) = args.limit {
        samples.truncate(limit);
    }
    let suite = Suite::Detection {
        dataset_version: manifest.version.clone(),
        dataset_sha256: manifest.samples_sha256.clone(),
    };
    let mut run_dir = RunDir::create(&args.results, &tool, suite, DETECTIONS_FILE)?;
    tracing::info!("run {} on {} samples", run_dir.run_id(), samples.len());
    let mut runner = DetectionRunner::start(&tool, Output::Log(run_dir.tool_log()))
        .await
        .with_context(|| {
            format!(
                "starting {} (log: {})",
                tool.manifest.name,
                run_dir.tool_log().display()
            )
        })?;
    let mut records = Vec::with_capacity(samples.len());
    for (i, sample) in samples.iter().enumerate() {
        let record = runner.detect(sample).await?;
        run_dir.append(&record)?;
        records.push(record);
        if (i + 1) % PROGRESS_EVERY == 0 {
            tracing::info!("{}/{}", i + 1, samples.len());
        }
    }
    runner.shutdown().await;
    let report = evaluate(&samples, &records)?;
    let f1 = |m| {
        report
            .scores
            .get(&m)
            .and_then(|s| s.micro.f1())
            .map_or_else(|| "–".to_owned(), |f| format!("{:.1}", f * 100.0))
    };
    println!(
        "{}: strict F1 {}, relaxed F1 {}, {} failed; results in {}",
        tool.manifest.name,
        f1(MatchMode::Strict),
        f1(MatchMode::Relaxed),
        report.failed_samples,
        run_dir.path().display()
    );
    Ok(())
}
