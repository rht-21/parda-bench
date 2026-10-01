use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use parda_eval::detection::evaluate;
use parda_report::report::{RestoreRun, ScoredDetection, build};
use parda_report::runs::{Run, RunBody, load_dataset_for, load_runs};
use parda_spec::record::Suite;
use parda_spec::sample::Sample;

pub fn run(results: &Path, data: &Path, out: &Path) -> Result<()> {
    let runs = latest_per_target(load_runs(results)?);
    let mut datasets: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
    let mut detections = Vec::new();
    let mut restores = Vec::new();
    for run in &runs {
        match (&run.body, &run.meta.suite) {
            (RunBody::Detection(records), Suite::Detection { dataset_sha256, .. }) => {
                if !datasets.contains_key(dataset_sha256) {
                    datasets.insert(dataset_sha256.clone(), load_dataset_for(data, &run.meta)?);
                }
                let report = evaluate(&datasets[dataset_sha256], records)
                    .with_context(|| format!("scoring run {}", run.meta.run_id))?;
                detections.push(ScoredDetection {
                    meta: &run.meta,
                    report,
                });
            }
            (RunBody::Restore(records), _) => restores.push(RestoreRun {
                meta: &run.meta,
                records,
            }),
            (RunBody::Detection(_), Suite::Restore { .. }) => {}
        }
    }
    let doc = build(&detections, &restores);
    fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let md = out.join("report.md");
    let html = out.join("report.html");
    fs::write(&md, doc.to_markdown()).with_context(|| format!("writing {}", md.display()))?;
    fs::write(&html, doc.to_html()).with_context(|| format!("writing {}", html.display()))?;
    println!(
        "{} runs reported in {} and {}",
        runs.len(),
        md.display(),
        html.display()
    );
    Ok(())
}

/// Keeps the newest run for each (suite, tool, config, driver or dataset); `runs` is oldest first.
fn latest_per_target(runs: Vec<Run>) -> Vec<Run> {
    let mut latest: BTreeMap<(String, String, String), Run> = BTreeMap::new();
    for run in runs {
        let target = match &run.meta.suite {
            Suite::Detection {
                dataset_version, ..
            } => format!("detect:{dataset_version}"),
            Suite::Restore { driver } => format!("restore:{driver}"),
        };
        let key = (
            target,
            run.meta.tool.name.clone(),
            run.meta.tool.config.clone(),
        );
        latest.insert(key, run);
    }
    latest.into_values().collect()
}
