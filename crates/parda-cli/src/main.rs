mod commands;
mod hardware;
mod results;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "parda-bench",
    version,
    about = "Benchmark and restore test harness for Indian PII masking tools"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Write the JSON Schema of every Parda Bench contract into a directory.
    Schema {
        #[arg(long, default_value = "spec")]
        out: PathBuf,
    },
    /// Build, validate and describe the detection dataset.
    #[command(subcommand)]
    Data(DataCommand),
    /// Run a tool's detector over a dataset and store its predictions.
    Detect(commands::detect::DetectArgs),
    /// Run the restore scenarios against a tool (or against no tool, with `--control`).
    Restore(commands::restore::RestoreArgs),
    /// Score stored runs and write `report.md` and `report.html`.
    Report(ReportArgs),
}

#[derive(Debug, Subcommand)]
enum DataCommand {
    /// Generate a versioned dataset from the built-in templates.
    Build(commands::data::BuildArgs),
    /// Check a dataset's digest, span geometry and identifier validity.
    Validate { dir: PathBuf },
    /// Print a dataset's composition as JSON.
    Stats { dir: PathBuf },
}

#[derive(Debug, Args)]
struct ReportArgs {
    #[arg(long, default_value = "results")]
    results: PathBuf,
    #[arg(long, default_value = "data")]
    data: PathBuf,
    #[arg(long, default_value = "reports")]
    out: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    let runtime = tokio::runtime::Runtime::new()?;
    match Cli::parse().command {
        Command::Schema { out } => commands::schema::write_schemas(&out),
        Command::Data(DataCommand::Build(args)) => commands::data::build(&args),
        Command::Data(DataCommand::Validate { dir }) => commands::data::validate(&dir),
        Command::Data(DataCommand::Stats { dir }) => commands::data::stats(&dir),
        Command::Detect(args) => runtime.block_on(commands::detect::run(&args)),
        Command::Restore(args) => runtime.block_on(commands::restore::run(&args)),
        Command::Report(args) => commands::report::run(&args.results, &args.data, &args.out),
    }
}
