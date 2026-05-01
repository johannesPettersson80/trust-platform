#!/usr/bin/env python3
"""Build stable Runtime VM benchmark gate summaries."""

from __future__ import annotations

import argparse
import json
import statistics
import tempfile
from pathlib import Path
from typing import Any

METRICS = ("p50_us", "p95_us", "p99_us", "max_us", "measured_duration_ms")
FIXTURE_REGRESSION_METRICS = ("p95_us", "p99_us", "measured_duration_ms")


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def percent_delta(current: float, baseline: float) -> float | None:
    if baseline == 0.0:
        return None
    return ((current - baseline) / baseline) * 100.0


def spread_pct(values: list[float], center: float) -> float | None:
    if center == 0.0:
        return None
    return ((max(values) - min(values)) / center) * 100.0


def normalize_summary_rows(summary: dict[str, Any]) -> dict[str, dict[str, Any]]:
    if "fixtures" in summary:
        return {row["name"]: row for row in summary["fixtures"]}
    if "rows" in summary:
        return {row["name"]: row for row in summary["rows"]}
    raise ValueError("summary must contain either 'fixtures' or 'rows'")


def aggregate_corpus_summaries(paths: list[Path]) -> list[dict[str, Any]]:
    if not paths:
        raise ValueError("at least one corpus summary is required")

    runs = [normalize_summary_rows(load_json(path)) for path in paths]
    expected_names = list(runs[0].keys())
    expected_set = set(expected_names)
    for index, run in enumerate(runs[1:], start=2):
        if set(run.keys()) != expected_set:
            missing = sorted(expected_set - set(run.keys()))
            extra = sorted(set(run.keys()) - expected_set)
            raise ValueError(
                f"run {index} fixture mismatch: missing={missing} extra={extra}"
            )

    fixtures: list[dict[str, Any]] = []
    for name in expected_names:
        rows = [run[name] for run in runs]
        fixture: dict[str, Any] = {"name": name}
        for key in ("project", "completed", "last_error", "fallbacks", "vm_highlight"):
            if key in rows[-1]:
                fixture[key] = rows[-1][key]
        if "overruns" in rows[-1]:
            fixture["overruns"] = max(int(row.get("overruns", 0)) for row in rows)

        confidence: dict[str, Any] = {}
        for metric in METRICS:
            values = [float(row.get(metric, 0.0)) for row in rows]
            median_value = float(statistics.median(values))
            fixture[metric] = median_value
            confidence[metric] = {
                "min": min(values),
                "max": max(values),
                "median": median_value,
                "spread_pct": spread_pct(values, median_value),
            }
        fixture["confidence"] = confidence
        fixtures.append(fixture)
    return fixtures


def aggregate_gate(fixtures: list[dict[str, Any]]) -> dict[str, Any]:
    total_measured = sum(float(row.get("measured_duration_ms", 0.0)) for row in fixtures)
    throughput = 0.0
    if total_measured > 0.0:
        throughput = len(fixtures) / (total_measured / 1000.0)
    return {
        "workloads": len(fixtures),
        "worst_p95_us": max((float(row["p95_us"]) for row in fixtures), default=0.0),
        "worst_p99_us": max((float(row["p99_us"]) for row in fixtures), default=0.0),
        "total_measured_ms": total_measured,
        "throughput_workloads_per_sec": throughput,
    }


def compare_fixtures(
    fixtures: list[dict[str, Any]], baseline_path: Path | None
) -> dict[str, Any] | None:
    if baseline_path is None:
        return None

    baseline_rows = normalize_summary_rows(load_json(baseline_path))
    rows = []
    for fixture in fixtures:
        name = fixture["name"]
        baseline = baseline_rows.get(name)
        if baseline is None:
            rows.append({"name": name, "status": "missing-baseline-fixture"})
            continue
        metric_deltas = {}
        for metric in METRICS:
            current = float(fixture.get(metric, 0.0))
            previous = float(baseline.get(metric, 0.0))
            metric_deltas[metric] = {
                "baseline": previous,
                "current": current,
                "delta_pct": percent_delta(current, previous),
            }
        rows.append({"name": name, "status": "compared", "metrics": metric_deltas})

    return {
        "baseline_path": str(baseline_path),
        "fixtures": rows,
    }


def evaluate_fixture_regressions(
    compare: dict[str, Any] | None, budget_pct: float
) -> dict[str, Any]:
    if compare is None:
        return {
            "status": "not-run",
            "budget_pct": budget_pct,
            "metrics": list(FIXTURE_REGRESSION_METRICS),
            "fixtures": [],
        }

    fixtures = []
    failed = False
    for row in compare["fixtures"]:
        fixture = {
            "name": row["name"],
            "status": "pass",
            "regressions": [],
        }
        if row["status"] != "compared":
            fixture["status"] = row["status"]
            failed = True
            fixtures.append(fixture)
            continue

        for metric in FIXTURE_REGRESSION_METRICS:
            delta = row["metrics"][metric]["delta_pct"]
            if delta is not None and delta > budget_pct:
                fixture["regressions"].append(
                    {
                        "metric": metric,
                        "delta_pct": delta,
                        "budget_pct": budget_pct,
                    }
                )

        if fixture["regressions"]:
            fixture["status"] = "fail"
            failed = True
        fixtures.append(fixture)

    return {
        "status": "fail" if failed else "pass",
        "budget_pct": budget_pct,
        "metrics": list(FIXTURE_REGRESSION_METRICS),
        "fixtures": fixtures,
    }


def render_markdown(summary: dict[str, Any]) -> str:
    confidence = summary["confidence"]
    aggregate = summary["aggregate"]
    lines = [
        "# Runtime VM Benchmark Gate",
        "",
        f"- profile: {summary['profile']}",
        f"- tier: {summary['tier']}",
        f"- build mode: {summary['build_mode']}",
        f"- samples: {confidence['samples']}",
        f"- warmup cycles: {confidence['warmup_cycles']}",
        f"- low-noise runs: {confidence['low_noise_runs']}",
        f"- workloads: {aggregate['workloads']}",
        f"- worst p95 us: {aggregate['worst_p95_us']:.3f}",
        f"- worst p99 us: {aggregate['worst_p99_us']:.3f}",
        f"- total measured benchmark ms: {aggregate['total_measured_ms']:.3f}",
        "",
        "| workload | p50 us | p95 us | p99 us | max us | measured ms | fallback | p95 spread % |",
        "|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for row in summary["fixtures"]:
        p95_spread = row["confidence"]["p95_us"]["spread_pct"]
        p95_spread_text = "n/a" if p95_spread is None else f"{p95_spread:.3f}"
        lines.append(
            f"| {row['name']} | {row['p50_us']:.3f} | {row['p95_us']:.3f} | "
            f"{row['p99_us']:.3f} | {row['max_us']:.3f} | "
            f"{row['measured_duration_ms']:.3f} | {row.get('fallbacks', 0)} | "
            f"{p95_spread_text} |"
        )

    compare = summary.get("compare")
    if compare:
        lines.extend(
            [
                "",
                f"Compare baseline: `{compare['baseline_path']}`",
                "",
                "| workload | p95 delta % | p99 delta % | measured delta % |",
                "|---|---:|---:|---:|",
            ]
        )
        for row in compare["fixtures"]:
            if row["status"] != "compared":
                lines.append(f"| {row['name']} | n/a | n/a | n/a |")
                continue
            metrics = row["metrics"]
            def fmt(metric: str) -> str:
                delta = metrics[metric]["delta_pct"]
                return "n/a" if delta is None else f"{delta:.3f}"

            lines.append(
                f"| {row['name']} | {fmt('p95_us')} | {fmt('p99_us')} | "
                f"{fmt('measured_duration_ms')} |"
            )

    fixture_gate = summary.get("fixture_regression_gate")
    if fixture_gate:
        lines.extend(
            [
                "",
                f"Fixture regression gate: `{fixture_gate['status']}` "
                f"(budget +{fixture_gate['budget_pct']:.3f}%)",
                "",
                "| workload | status | regressions |",
                "|---|---|---|",
            ]
        )
        for row in fixture_gate["fixtures"]:
            regressions = ", ".join(
                f"{item['metric']} +{item['delta_pct']:.3f}%"
                for item in row["regressions"]
            )
            lines.append(f"| {row['name']} | {row['status']} | {regressions or '-'} |")

    lines.extend(["", "Result: RECORDED", ""])
    return "\n".join(lines)


def build_summary(args: argparse.Namespace) -> dict[str, Any]:
    corpus_paths = [Path(path) for path in args.corpus_summary]
    fixtures = aggregate_corpus_summaries(corpus_paths)
    first = load_json(corpus_paths[0])
    compare = compare_fixtures(
        fixtures, Path(args.compare_baseline) if args.compare_baseline else None
    )
    fixture_regression_budget_pct = float(
        getattr(args, "fixture_regression_budget_pct", 5.0)
    )
    summary = {
        "profile": args.profile,
        "tier": args.tier,
        "build_mode": first.get("build_mode", "generic"),
        "confidence": {
            "samples": args.samples,
            "warmup_cycles": args.warmup_cycles,
            "low_noise_runs": args.low_noise_runs,
            "corpus_summary_paths": [str(path) for path in corpus_paths],
        },
        "aggregate": aggregate_gate(fixtures),
        "fixtures": fixtures,
        "compare": compare,
        "fixture_regression_gate": evaluate_fixture_regressions(
            compare, fixture_regression_budget_pct
        ),
        "result": "recorded",
    }
    return summary


def write_outputs(summary: dict[str, Any], out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "summary.json").write_text(json.dumps(summary, indent=2))
    (out_dir / "summary.md").write_text(render_markdown(summary))


def self_test() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        current = {
            "tier": "default",
            "build_mode": "generic",
            "rows": [
                {
                    "name": "loop_arith",
                    "project": "loop",
                    "p50_us": 10.0,
                    "p95_us": 20.0,
                    "p99_us": 30.0,
                    "max_us": 40.0,
                    "overruns": 0,
                    "completed": 1,
                    "last_error": 0,
                    "fallbacks": 0,
                    "vm_highlight": "vm-clean",
                    "measured_duration_ms": 100.0,
                },
                {
                    "name": "string_stdlib",
                    "project": "string",
                    "p50_us": 40.0,
                    "p95_us": 56.0,
                    "p99_us": 60.0,
                    "max_us": 70.0,
                    "overruns": 0,
                    "completed": 1,
                    "last_error": 0,
                    "fallbacks": 0,
                    "vm_highlight": "vm-clean",
                    "measured_duration_ms": 160.0,
                }
            ],
        }
        repeat = json.loads(json.dumps(current))
        repeat["rows"][0]["p95_us"] = 22.0
        repeat["rows"][1]["p95_us"] = 58.0
        baseline = json.loads(json.dumps(current))
        baseline["rows"][0]["p95_us"] = 25.0
        baseline["rows"][1]["p95_us"] = 50.0
        (root / "current.json").write_text(json.dumps(current))
        (root / "repeat.json").write_text(json.dumps(repeat))
        (root / "baseline.json").write_text(json.dumps(baseline))

        args = argparse.Namespace(
            corpus_summary=[str(root / "current.json"), str(root / "repeat.json")],
            compare_baseline=str(root / "baseline.json"),
            out_dir=str(root / "out"),
            profile="quick-low-noise",
            tier="default",
            samples=4,
            warmup_cycles=1,
            low_noise_runs=2,
            fixture_regression_budget_pct=5.0,
        )
        summary = build_summary(args)
        assert summary["confidence"]["low_noise_runs"] == 2
        fixture = summary["fixtures"][0]
        assert fixture["p95_us"] == 21.0
        delta = summary["compare"]["fixtures"][0]["metrics"]["p95_us"]["delta_pct"]
        assert round(delta, 3) == -16.0
        assert summary["fixture_regression_gate"]["status"] == "fail"
        regression_rows = {
            row["name"]: row for row in summary["fixture_regression_gate"]["fixtures"]
        }
        assert regression_rows["loop_arith"]["status"] == "pass"
        assert regression_rows["string_stdlib"]["status"] == "fail"
        assert (
            regression_rows["string_stdlib"]["regressions"][0]["metric"] == "p95_us"
        )
        write_outputs(summary, Path(args.out_dir))
        assert (Path(args.out_dir) / "summary.json").exists()
        assert "Compare baseline" in (Path(args.out_dir) / "summary.md").read_text()
        assert "Fixture regression gate: `fail`" in (
            Path(args.out_dir) / "summary.md"
        ).read_text()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus-summary", action="append", default=[])
    parser.add_argument("--compare-baseline")
    parser.add_argument("--out-dir", default="target/gate-artifacts/runtime-vm-bench")
    parser.add_argument("--profile", default="quick")
    parser.add_argument("--tier", default="default")
    parser.add_argument("--samples", type=int, default=32)
    parser.add_argument("--warmup-cycles", type=int, default=8)
    parser.add_argument("--low-noise-runs", type=int, default=1)
    parser.add_argument("--fixture-regression-budget-pct", type=float, default=5.0)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.self_test:
        self_test()
        return
    summary = build_summary(args)
    write_outputs(summary, Path(args.out_dir))


if __name__ == "__main__":
    main()
