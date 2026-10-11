import argparse
import asyncio
import json
import logging
import sys
from pathlib import Path

from parda_bench import __version__
from parda_bench.adapters.detect import DetectionRunner
from parda_bench.adapters.errors import AdapterError
from parda_bench.adapters.tool import Tool
from parda_bench.cli.results import RunDir
from parda_bench.data import GENERATOR_VERSION
from parda_bench.data.build import DATASET_VERSION, DATASETS, PER_TEMPLATE, SEED, build, templates_for
from parda_bench.data.io import (
    SAMPLES_FILE,
    DataIoError,
    DatasetManifest,
    file_sha256,
    read_manifest,
    read_samples,
    samples_sha256,
    write_dataset,
)
from parda_bench.data.stats import stats
from parda_bench.data.validate import validate
from parda_bench.eval.detection import evaluate
from parda_bench.eval.matching import MatchMode
from parda_bench.report.report import RestoreRun, ScoredDetection, pct
from parda_bench.report.report import build as build_report
from parda_bench.report.runs import DETECTIONS_FILE, RESTORE_FILE, Run, RunsError, load_dataset_for, load_runs
from parda_bench.restore.driver import RAW_HTTP, Driver
from parda_bench.restore.runner import (
    LIBRARY_DRIVER,
    ToolRole,
    redact_only,
    role,
    run_control,
    run_library,
    run_proxy,
)
from parda_bench.restore.scenarios import ScenarioError, load_dir
from parda_bench.spec.record import DetectionSuite, Fail, NotApplicable, Pass, RestoreRecord, RestoreSuite
from parda_bench.spec.schemas import write_schemas

log = logging.getLogger("parda_bench")
PROGRESS_EVERY = 250


class CliError(Exception):
    """A problem the user can fix; printed without a traceback."""


def cmd_data_build(args: argparse.Namespace) -> None:
    samples = build(templates_for(args.version), args.seed, args.per_template)
    issues = validate(samples)
    if issues:
        raise CliError(f"generated dataset is inconsistent (generator bug): {issues[:5]}")
    digest = samples_sha256(samples)
    directory = args.out / args.version
    if (directory / SAMPLES_FILE).exists():
        if file_sha256(directory / SAMPLES_FILE) == digest:
            log.info("%s already holds this exact dataset", directory)
            return
        raise CliError(f"{directory} already holds a different dataset; published datasets are immutable")
    manifest = DatasetManifest(
        version=args.version,
        seed=args.seed,
        per_template=args.per_template,
        generator_version=GENERATOR_VERSION,
        sample_count=len(samples),
        samples_sha256=digest,
    )
    write_dataset(directory, samples, manifest)
    log.info("wrote %d samples to %s", len(samples), directory)


def cmd_data_validate(args: argparse.Namespace) -> None:
    manifest = read_manifest(args.dir)
    digest = file_sha256(args.dir / SAMPLES_FILE)
    if digest != manifest.samples_sha256:
        raise CliError(f"{args.dir / SAMPLES_FILE} has sha256 {digest}, manifest says {manifest.samples_sha256}")
    samples = read_samples(args.dir / SAMPLES_FILE)
    if len(samples) != manifest.sample_count:
        raise CliError(f"{len(samples)} samples on disk, manifest says {manifest.sample_count}")
    issues = validate(samples)
    for issue in issues:
        print(f"{issue.sample_id}: {issue.message}")
    if issues:
        raise CliError(f"{len(issues)} issues in {args.dir}")
    print(f"{args.dir}: {len(samples)} samples, ok")


def cmd_data_stats(args: argparse.Namespace) -> None:
    print(json.dumps(stats(read_samples(args.dir / SAMPLES_FILE)), indent=2, ensure_ascii=False))


async def cmd_detect(args: argparse.Namespace) -> None:
    tool = Tool.load(args.tool, args.config)
    manifest = read_manifest(args.dataset)
    samples = read_samples(args.dataset / SAMPLES_FILE)[: args.limit]
    suite = DetectionSuite(dataset_version=manifest.version, dataset_sha256=manifest.samples_sha256)
    run_dir = RunDir.create(args.results, tool, suite, DETECTIONS_FILE)
    log.info("run %s on %d samples", run_dir.run_id, len(samples))
    runner = DetectionRunner(tool, run_dir.tool_log)
    records = []
    try:
        for i, sample in enumerate(samples, 1):
            record = await runner.detect(sample)
            run_dir.append(record)
            records.append(record)
            if i % PROGRESS_EVERY == 0:
                log.info("%d/%d", i, len(samples))
    finally:
        await runner.shutdown()
        run_dir.close()
    report = evaluate(samples, records)
    print(
        f"{tool.manifest.name}: strict F1 {pct(report.scores[MatchMode.STRICT].micro.f1())}, "
        f"relaxed F1 {pct(report.scores[MatchMode.RELAXED].micro.f1())}, {report.failed_samples} failed; "
        f"results in {run_dir.path}"
    )


async def cmd_restore(args: argparse.Namespace) -> None:
    scenarios = load_dir(args.scenarios)
    records: list[RestoreRecord]
    if args.tool is None:
        run_dir = RunDir.create_control(args.results, args.driver, RESTORE_FILE)
        driver = await Driver.open(args.driver, args.drivers)
        try:
            records = await run_control(driver, scenarios)
        finally:
            await driver.close()
    else:
        tool = Tool.load(args.tool, args.config)
        tool_role = role(tool)
        if tool_role is not ToolRole.PROXY and args.driver != RAW_HTTP:
            raise CliError(f"{tool.manifest.name} is not a proxy; drivers apply only to proxy tools")
        driver_name = args.driver if tool_role is ToolRole.PROXY else LIBRARY_DRIVER
        run_dir = RunDir.create(args.results, tool, RestoreSuite(driver=driver_name), RESTORE_FILE)
        if tool_role is ToolRole.PROXY:
            driver = await Driver.open(args.driver, args.drivers)
            try:
                records = await run_proxy(tool, driver, scenarios, run_dir.tool_log)
            finally:
                await driver.close()
        elif tool_role is ToolRole.LIBRARY:
            records = await run_library(tool, scenarios, run_dir.tool_log)
        else:
            records = redact_only(scenarios)
    for record in records:
        run_dir.append(record)
    run_dir.close()

    def count(kind: type) -> int:
        return sum(isinstance(r.outcome, kind) for r in records)

    print(f"{count(Pass)} pass, {count(Fail)} fail, {count(NotApplicable)} not applicable; results in {run_dir.path}")


def _latest_per_target(runs: list[Run]) -> list[Run]:
    """The newest run for each (suite, tool, config, driver or dataset); `runs` is oldest first."""
    latest: dict[tuple[str, str, str], Run] = {}
    for run in runs:
        suite = run.meta.suite
        target = f"detect:{suite.dataset_version}" if isinstance(suite, DetectionSuite) else f"restore:{suite.driver}"
        latest[(target, run.meta.tool.name, run.meta.tool.config)] = run
    return list(latest.values())


def cmd_report(args: argparse.Namespace) -> None:
    runs = _latest_per_target(load_runs(args.results)) if args.results.is_dir() else []
    detections, restores, datasets = [], [], {}
    for run in runs:
        suite = run.meta.suite
        if isinstance(suite, DetectionSuite):
            if suite.dataset_sha256 not in datasets:
                datasets[suite.dataset_sha256] = load_dataset_for(args.data, run.meta)
            detections.append(ScoredDetection(run.meta, evaluate(datasets[suite.dataset_sha256], run.detections)))
        else:
            restores.append(RestoreRun(run.meta, run.restores))
    doc = build_report(detections, restores)
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "report.md").write_text(doc.to_markdown(), encoding="utf-8")
    (args.out / "report.html").write_text(doc.to_html(), encoding="utf-8")
    print(f"{len(runs)} runs reported in {args.out / 'report.md'} and {args.out / 'report.html'}")


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="parda-bench", description="Benchmark and restore test harness for Indian PII masking tools"
    )
    p.add_argument("--version", action="version", version=f"parda-bench {__version__}")
    sub = p.add_subparsers(dest="command", required=True)

    schema = sub.add_parser("schema", help="write the JSON Schema of every contract into a directory")
    schema.add_argument("--out", type=Path, default=Path("spec"))
    schema.set_defaults(run=lambda a: write_schemas(a.out))

    data = sub.add_parser("data", help="build, validate and describe the detection dataset").add_subparsers(
        dest="data_command", required=True
    )
    data_build = data.add_parser("build", help="generate a versioned dataset from the built-in templates")
    data_build.add_argument("--version", choices=sorted(DATASETS), default=DATASET_VERSION)
    data_build.add_argument("--seed", type=int, default=SEED)
    data_build.add_argument("--per-template", type=int, default=PER_TEMPLATE)
    data_build.add_argument("--out", type=Path, default=Path("data"))
    data_build.set_defaults(run=cmd_data_build)
    for name, handler, text in (
        ("validate", cmd_data_validate, "check a dataset's digest, spans and check digits"),
        ("stats", cmd_data_stats, "print a dataset's composition as JSON"),
    ):
        command = data.add_parser(name, help=text)
        command.add_argument("dir", type=Path)
        command.set_defaults(run=handler)

    detect = sub.add_parser("detect", help="run a tool's detector over a dataset and store its predictions")
    detect.add_argument("--tool", type=Path, required=True, help="tool directory containing tool.toml")
    detect.add_argument("--config", default="default")
    detect.add_argument("--dataset", type=Path, required=True, help="dataset directory, e.g. data/v0.2.0")
    detect.add_argument("--limit", type=int, help="only the first N samples, for a quick check")
    detect.add_argument("--results", type=Path, default=Path("results"))
    detect.set_defaults(run=lambda a: asyncio.run(cmd_detect(a)))

    restore = sub.add_parser("restore", help="run the restore scenarios against a tool (or none, with --control)")
    target = restore.add_mutually_exclusive_group(required=True)
    target.add_argument("--tool", type=Path, help="tool directory containing tool.toml")
    target.add_argument("--control", action="store_true", help="no tool: check the drivers and the suite")
    restore.add_argument("--config", default="default")
    restore.add_argument("--driver", default=RAW_HTTP, help="raw_http or a name from drivers/drivers.toml")
    restore.add_argument("--scenarios", type=Path, default=Path("scenarios"))
    restore.add_argument("--drivers", type=Path, default=Path("drivers"))
    restore.add_argument("--results", type=Path, default=Path("results"))
    restore.set_defaults(run=lambda a: asyncio.run(cmd_restore(a)))

    report = sub.add_parser("report", help="score stored runs and write report.md and report.html")
    report.add_argument("--results", type=Path, default=Path("results"))
    report.add_argument("--data", type=Path, default=Path("data"))
    report.add_argument("--out", type=Path, default=Path("reports"))
    report.set_defaults(run=cmd_report)
    return p


def main(argv: list[str] | None = None) -> int:
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s: %(message)s", stream=sys.stderr)
    args = parser().parse_args(argv)
    try:
        args.run(args)
    except (CliError, AdapterError, DataIoError, RunsError, ScenarioError, FileExistsError) as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
