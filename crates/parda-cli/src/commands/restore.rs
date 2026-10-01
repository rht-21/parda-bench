use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Args;
use parda_adapters::Tool;
use parda_adapters::process::Output;
use parda_report::runs::RESTORE_FILE;
use parda_restore::driver::{Driver, RAW_HTTP};
use parda_restore::runner::{
    LIBRARY_DRIVER, ToolRole, redact_only, role, run_control, run_library, run_proxy,
};
use parda_restore::scenarios::load_dir;
use parda_spec::record::{RestoreOutcome, Suite};

use crate::results::RunDir;

#[derive(Debug, Args)]
pub struct RestoreArgs {
    /// Tool directory containing `tool.toml`.
    #[arg(long, required_unless_present = "control", conflicts_with = "control")]
    tool: Option<PathBuf>,
    #[arg(long, default_value = "default")]
    config: String,
    /// Run the drivers straight against the mock with no tool, to check the drivers and the suite.
    #[arg(long)]
    control: bool,
    /// Client driver for proxy tools and control runs: `raw_http` or a name from `drivers/drivers.toml`.
    #[arg(long, default_value = RAW_HTTP)]
    driver: String,
    #[arg(long, default_value = "scenarios")]
    scenarios: PathBuf,
    #[arg(long, default_value = "drivers")]
    drivers: PathBuf,
    #[arg(long, default_value = "results")]
    results: PathBuf,
}

pub async fn run(args: &RestoreArgs) -> Result<()> {
    let scenarios = load_dir(&args.scenarios)?;
    let (mut run_dir, records) = match &args.tool {
        None => {
            let mut driver = Driver::open(&args.driver, &args.drivers)?;
            let run_dir = RunDir::create_control(&args.results, &args.driver, RESTORE_FILE)?;
            (run_dir, run_control(&mut driver, &scenarios).await?)
        }
        Some(dir) => {
            let tool = Tool::load(dir, &args.config)?;
            let role = role(&tool);
            let driver_name = if role == ToolRole::Proxy {
                args.driver.as_str()
            } else {
                LIBRARY_DRIVER
            };
            if role != ToolRole::Proxy && args.driver != RAW_HTTP {
                bail!(
                    "{} is not a proxy; drivers apply only to proxy tools",
                    tool.manifest.name
                );
            }
            let suite = Suite::Restore {
                driver: driver_name.to_owned(),
            };
            let run_dir = RunDir::create(&args.results, &tool, suite, RESTORE_FILE)?;
            let output = Output::Log(run_dir.tool_log());
            let records = match role {
                ToolRole::Proxy => {
                    let mut driver = Driver::open(&args.driver, &args.drivers)?;
                    run_proxy(&tool, &mut driver, &scenarios, &output).await?
                }
                ToolRole::Library => run_library(&tool, &scenarios, &output).await?,
                ToolRole::RedactOnly => redact_only(&scenarios),
            };
            (run_dir, records)
        }
    };
    for record in &records {
        run_dir.append(record)?;
    }
    let count = |f: fn(&RestoreOutcome) -> bool| records.iter().filter(|r| f(&r.outcome)).count();
    println!(
        "{} pass, {} fail, {} not applicable; results in {}",
        count(|o| matches!(o, RestoreOutcome::Pass)),
        count(|o| matches!(o, RestoreOutcome::Fail { .. })),
        count(|o| matches!(o, RestoreOutcome::NotApplicable { .. })),
        run_dir.path().display()
    );
    Ok(())
}
