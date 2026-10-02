use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;
use parda_data::build::{build as build_samples, builtin_templates};
use parda_data::io::{
    DatasetManifest, SAMPLES_FILE, file_sha256, read_manifest, read_samples, samples_sha256,
    write_dataset,
};
use parda_data::stats::Stats;
use parda_data::validate::validate as validate_samples;

#[derive(Debug, Args)]
pub struct BuildArgs {
    #[arg(long, default_value_t = 42)]
    seed: u64,
    /// Samples rendered from each template.
    #[arg(long, default_value_t = 25)]
    per_template: usize,
    /// Dataset version; the dataset is written to `<out>/<version>/`.
    #[arg(long, default_value = parda_data::DATASET_VERSION)]
    version: String,
    #[arg(long, default_value = "data")]
    out: PathBuf,
}

pub fn build(args: &BuildArgs) -> Result<()> {
    let templates = builtin_templates()?;
    let samples = build_samples(&templates, args.seed, args.per_template);
    let issues = validate_samples(&samples);
    if !issues.is_empty() {
        bail!("generated dataset is inconsistent (generator bug): {issues:#?}");
    }
    let digest = samples_sha256(&samples)?;
    let dir = args.out.join(&args.version);
    if dir.join(SAMPLES_FILE).exists() {
        let existing = file_sha256(&dir.join(SAMPLES_FILE))?;
        if existing == digest {
            tracing::info!("{} already holds this exact dataset", dir.display());
            return Ok(());
        }
        bail!(
            "{} already holds a different dataset; published datasets are immutable, choose a new --version",
            dir.display()
        );
    }
    let manifest = DatasetManifest {
        version: args.version.clone(),
        seed: args.seed,
        per_template: args.per_template,
        generator_version: parda_data::GENERATOR_VERSION.to_owned(),
        sample_count: samples.len(),
        samples_sha256: digest,
    };
    write_dataset(&dir, &samples, &manifest)?;
    tracing::info!(
        "wrote {} samples from {} templates to {}",
        samples.len(),
        templates.len(),
        dir.display()
    );
    Ok(())
}

pub fn validate(dir: &Path) -> Result<()> {
    let manifest = read_manifest(dir)?;
    let samples_path = dir.join(SAMPLES_FILE);
    let digest = file_sha256(&samples_path)?;
    if digest != manifest.samples_sha256 {
        bail!(
            "{} has sha256 {digest}, manifest says {}",
            samples_path.display(),
            manifest.samples_sha256
        );
    }
    let samples = read_samples(&samples_path)?;
    if samples.len() != manifest.sample_count {
        bail!(
            "{} samples on disk, manifest says {}",
            samples.len(),
            manifest.sample_count
        );
    }
    let issues = validate_samples(&samples);
    for issue in &issues {
        println!("{}: {}", issue.sample_id, issue.message);
    }
    if !issues.is_empty() {
        bail!("{} issues in {}", issues.len(), dir.display());
    }
    println!("{}: {} samples, ok", dir.display(), samples.len());
    Ok(())
}

pub fn stats(dir: &Path) -> Result<()> {
    let samples = read_samples(&dir.join(SAMPLES_FILE))?;
    let json = serde_json::to_string_pretty(&Stats::of(&samples)).context("serializing stats")?;
    println!("{json}");
    Ok(())
}
