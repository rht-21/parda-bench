from parda_bench.report.doc import Doc
from parda_bench.report.report import RestoreRun, build, ms, pct
from parda_bench.spec.record import (
    CONTROL_TOOL,
    CheckFailure,
    Fail,
    Hardware,
    Pass,
    RestoreRecord,
    RestoreSuite,
    RunMeta,
    ToolRef,
)
from parda_bench.spec.scenario import CheckKind


def meta(run_id: str, driver: str, tool: str = "t") -> RunMeta:
    return RunMeta(
        run_id=run_id,
        started_at="2026-10-01T00:00:00Z",
        harness_version="0.2.0",
        tool=ToolRef(name=tool, version="1", commit="c", config="default"),
        hardware=Hardware(os="linux", arch="x86_64", cpu="cpu", cores=4, memory_mb=1024),
        suite=RestoreSuite(driver=driver),
    )


def fail(*checks: tuple[CheckKind, str]) -> Fail:
    return Fail(failures=[CheckFailure(check=c, detail=d) for c, d in checks], upstream_excerpt="", client_excerpt="")


def test_markdown_escapes_pipes_and_html_escapes_markup() -> None:
    d = Doc("T")
    d.heading(2, "Section")
    d.table(["a", "b"], [["x|y", "<z>"]])
    assert "## Section\n" in d.to_markdown() and "| a | b |\n|---|---|\n| x\\|y | <z> |\n" in d.to_markdown()
    assert "<td>&lt;z&gt;</td>" in d.to_html() and "<z>" not in d.to_html()


def test_restore_matrix_marks_each_outcome() -> None:
    records = [
        RestoreRecord(scenario_id="a", outcome=Pass()),
        RestoreRecord(scenario_id="b", outcome=fail((CheckKind.NO_LEAK_UPSTREAM, "leaked"))),
    ]
    md = build([], [RestoreRun(meta("r1", "raw_http"), records)]).to_markdown()
    assert "| a | pass |" in md and "| b | FAIL |" in md
    assert "no_leak_upstream: leaked" in md and "| t | raw_http | 1 | 1 | 0 |" in md
    assert "Driver checks" not in md


def test_control_runs_are_listed_as_driver_checks_only() -> None:
    records = [
        RestoreRecord(
            scenario_id="fail-529",
            outcome=fail((CheckKind.NO_LEAK_UPSTREAM, "expected"), (CheckKind.FAILS_CLOSED, "saw 500")),
        )
    ]
    md = build([], [RestoreRun(meta("c1", "litellm", CONTROL_TOOL), records)]).to_markdown()
    assert "## Restore" not in md and "| litellm | 1 | 1 |" in md
    assert "litellm: fail-529: fails_closed: saw 500" in md


def test_formats_numbers_and_empty_reports() -> None:
    assert (pct(0.8766), pct(None), ms(1_234_567)) == ("87.7", "–", "1.23")
    assert "No runs found." in build([], []).to_markdown()
