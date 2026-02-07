#!/usr/bin/env python3
"""Aggregate ST test flake samples into a rolling-window report."""

from __future__ import annotations

import argparse
import datetime as dt
import glob
import io
import json
import sys
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass
class FlakeSample:
    source: str
    generated_at: dt.datetime
    runs: int
    failures: int
    passes: int
    flake_rate_percent: float


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Aggregate ST test flake samples over a rolling window from local files "
            "and/or GitHub Actions artifacts."
        )
    )
    parser.add_argument(
        "--input-glob",
        action="append",
        default=[],
        help="Local glob(s) that resolve to flake sample JSON files",
    )
    parser.add_argument(
        "--sample-file",
        action="append",
        default=[],
        help="Explicit local sample JSON file path(s) to include",
    )
    parser.add_argument("--github-repo", help="GitHub repo in owner/name form")
    parser.add_argument("--github-token", help="GitHub token for Actions artifact API")
    parser.add_argument(
        "--artifact-name-prefix",
        default="nightly-reliability-",
        help="Artifact name prefix to scan when using --github-repo",
    )
    parser.add_argument(
        "--sample-name",
        default="st-test-flake-sample.json",
        help="Sample JSON file name inside each artifact zip",
    )
    parser.add_argument("--days", type=int, default=14, help="Rolling window size in days")
    parser.add_argument("--min-samples", type=int, default=14)
    parser.add_argument("--max-aggregate-rate", type=float, default=0.0)
    parser.add_argument(
        "--tcunit-max-rate",
        type=float,
        default=None,
        help="Optional TcUnit baseline max flake rate percent for comparison.",
    )
    parser.add_argument("--output-json", required=True)
    parser.add_argument("--output-md")
    parser.add_argument(
        "--enforce-gate",
        action="store_true",
        help="Return non-zero when aggregate gate is fail or pending.",
    )
    parser.add_argument(
        "--pending-ok",
        action="store_true",
        help="When enforcing, treat insufficient sample count as non-fatal.",
    )
    return parser.parse_args()


def parse_timestamp(value: str) -> dt.datetime:
    normalized = value.replace("Z", "+00:00")
    parsed = dt.datetime.fromisoformat(normalized)
    if parsed.tzinfo is None:
        return parsed.replace(tzinfo=dt.timezone.utc)
    return parsed.astimezone(dt.timezone.utc)


def parse_sample(payload: dict[str, Any], source: str) -> FlakeSample:
    generated_at_raw = payload.get("generated_at")
    runs = payload.get("runs")
    failures = payload.get("failures")
    passes = payload.get("passes")
    rate = payload.get("flake_rate_percent")

    if not isinstance(generated_at_raw, str):
        raise ValueError(f"missing/invalid generated_at in {source}")
    if not all(isinstance(v, int) for v in [runs, failures, passes]):
        raise ValueError(f"missing/invalid run counters in {source}")
    if not isinstance(rate, (int, float)):
        raise ValueError(f"missing/invalid flake_rate_percent in {source}")
    if runs <= 0:
        raise ValueError(f"runs must be > 0 in {source}")
    if failures < 0 or passes < 0:
        raise ValueError(f"negative counters in {source}")

    return FlakeSample(
        source=source,
        generated_at=parse_timestamp(generated_at_raw),
        runs=runs,
        failures=failures,
        passes=passes,
        flake_rate_percent=float(rate),
    )


def load_sample_file(path: Path) -> FlakeSample:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"sample is not a JSON object: {path}")
    return parse_sample(payload, str(path))


def collect_local_samples(input_globs: list[str], sample_files: list[str]) -> list[FlakeSample]:
    samples: list[FlakeSample] = []
    resolved: set[Path] = set()

    for pattern in input_globs:
        for candidate in glob.glob(pattern):
            resolved.add(Path(candidate))
    for sample in sample_files:
        resolved.add(Path(sample))

    for path in sorted(resolved):
        if not path.exists():
            continue
        samples.append(load_sample_file(path))
    return samples


def github_api_get_json(url: str, token: str) -> dict[str, Any]:
    request = urllib.request.Request(url)
    request.add_header("Accept", "application/vnd.github+json")
    request.add_header("Authorization", f"Bearer {token}")
    request.add_header("X-GitHub-Api-Version", "2022-11-28")
    with urllib.request.urlopen(request, timeout=30) as response:
        payload = json.loads(response.read().decode("utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("unexpected GitHub API response payload")
    return payload


def github_download_artifact_zip(url: str, token: str) -> bytes:
    request = urllib.request.Request(url)
    request.add_header("Accept", "application/vnd.github+json")
    request.add_header("Authorization", f"Bearer {token}")
    request.add_header("X-GitHub-Api-Version", "2022-11-28")
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def collect_github_samples(
    repo: str,
    token: str,
    days: int,
    artifact_name_prefix: str,
    sample_name: str,
) -> list[FlakeSample]:
    now = dt.datetime.now(dt.timezone.utc)
    cutoff = now - dt.timedelta(days=days)
    page = 1
    samples: list[FlakeSample] = []
    max_pages = 10

    while page <= max_pages:
        url = f"https://api.github.com/repos/{repo}/actions/artifacts?per_page=100&page={page}"
        payload = github_api_get_json(url, token)
        artifacts = payload.get("artifacts")
        if not isinstance(artifacts, list) or not artifacts:
            break

        page_had_candidates = False
        for artifact in artifacts:
            if not isinstance(artifact, dict):
                continue
            name = artifact.get("name")
            expired = artifact.get("expired")
            created_at_raw = artifact.get("created_at")
            archive_download_url = artifact.get("archive_download_url")
            if (
                not isinstance(name, str)
                or not name.startswith(artifact_name_prefix)
                or expired is True
                or not isinstance(created_at_raw, str)
                or not isinstance(archive_download_url, str)
            ):
                continue
            created_at = parse_timestamp(created_at_raw)
            if created_at < cutoff:
                continue
            page_had_candidates = True

            blob = github_download_artifact_zip(archive_download_url, token)
            with zipfile.ZipFile(io.BytesIO(blob)) as archive:
                candidate = None
                for member in archive.namelist():
                    if member.endswith(sample_name):
                        candidate = member
                        break
                if candidate is None:
                    continue
                payload_raw = archive.read(candidate).decode("utf-8")
                payload_json = json.loads(payload_raw)
                if not isinstance(payload_json, dict):
                    continue
                source = f"github:{name}:{candidate}"
                samples.append(parse_sample(payload_json, source))

        if not page_had_candidates:
            # Continue paging because artifact listing is not guaranteed to be strictly date-grouped.
            page += 1
            continue
        page += 1

    return samples


def summarize_samples(
    samples: list[FlakeSample], window_days: int, now: dt.datetime
) -> dict[str, Any]:
    cutoff = now - dt.timedelta(days=window_days)
    filtered = [sample for sample in samples if sample.generated_at >= cutoff]
    filtered.sort(key=lambda sample: sample.generated_at)

    total_runs = sum(sample.runs for sample in filtered)
    total_failures = sum(sample.failures for sample in filtered)
    total_passes = sum(sample.passes for sample in filtered)
    aggregate_rate = 0.0
    if total_runs > 0:
        aggregate_rate = round((total_failures / total_runs) * 100.0, 4)

    daily: dict[str, dict[str, int | float]] = {}
    for sample in filtered:
        day_key = sample.generated_at.date().isoformat()
        day = daily.setdefault(day_key, {"samples": 0, "runs": 0, "failures": 0, "rate": 0.0})
        day["samples"] = int(day["samples"]) + 1
        day["runs"] = int(day["runs"]) + sample.runs
        day["failures"] = int(day["failures"]) + sample.failures

    for day in daily.values():
        runs = int(day["runs"])
        failures = int(day["failures"])
        day["rate"] = round((failures / runs) * 100.0, 4) if runs > 0 else 0.0

    sample_rows = [
        {
            "source": sample.source,
            "generated_at": sample.generated_at.isoformat(),
            "runs": sample.runs,
            "passes": sample.passes,
            "failures": sample.failures,
            "flake_rate_percent": sample.flake_rate_percent,
        }
        for sample in filtered
    ]

    return {
        "window_days": window_days,
        "sample_count": len(filtered),
        "coverage_days": len(daily),
        "total_runs": total_runs,
        "total_passes": total_passes,
        "total_failures": total_failures,
        "aggregate_flake_rate_percent": aggregate_rate,
        "daily": dict(sorted(daily.items())),
        "samples": sample_rows,
    }


def evaluate_gate(
    summary: dict[str, Any],
    min_samples: int,
    max_aggregate_rate: float,
    tcunit_max_rate: float | None,
) -> tuple[str, list[str]]:
    reasons: list[str] = []
    sample_count = int(summary["sample_count"])
    aggregate = float(summary["aggregate_flake_rate_percent"])

    if sample_count < min_samples:
        reasons.append(f"insufficient samples: {sample_count} < {min_samples}")
        return "pending", reasons

    if aggregate > max_aggregate_rate:
        reasons.append(
            f"aggregate flake rate {aggregate:.4f}% exceeds max {max_aggregate_rate:.4f}%"
        )

    if tcunit_max_rate is not None and aggregate > tcunit_max_rate:
        reasons.append(
            f"aggregate flake rate {aggregate:.4f}% exceeds TcUnit baseline {tcunit_max_rate:.4f}%"
        )

    if reasons:
        return "fail", reasons
    return "pass", ["aggregate gate satisfied"]


def write_markdown(path: Path, payload: dict[str, Any]) -> None:
    summary = payload["summary"]
    gate = payload["gate"]
    assert isinstance(summary, dict)
    assert isinstance(gate, dict)

    lines = [
        "# ST Test Flake 14-Day Aggregate",
        "",
        f"- Generated: `{payload['generated_at']}`",
        f"- Window: `{summary['window_days']}` days",
        f"- Samples: `{summary['sample_count']}`",
        f"- Coverage days: `{summary['coverage_days']}`",
        f"- Total runs: `{summary['total_runs']}`",
        f"- Total failures: `{summary['total_failures']}`",
        f"- Aggregate flake rate: `{float(summary['aggregate_flake_rate_percent']):.4f}%`",
        f"- Gate status: `{gate['status']}`",
        "",
        "## Gate Reasons",
    ]
    for reason in gate["reasons"]:
        lines.append(f"- {reason}")

    lines.extend(
        [
            "",
            "## Daily Summary",
            "| Day | Samples | Runs | Failures | Rate |",
            "|---|---:|---:|---:|---:|",
        ]
    )
    daily = summary["daily"]
    assert isinstance(daily, dict)
    for day, row in daily.items():
        assert isinstance(row, dict)
        lines.append(
            f"| {day} | {row['samples']} | {row['runs']} | {row['failures']} | {float(row['rate']):.4f}% |"
        )

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    now = dt.datetime.now(dt.timezone.utc)

    samples: list[FlakeSample] = []
    samples.extend(collect_local_samples(args.input_glob, args.sample_file))

    if args.github_repo:
        if not args.github_token:
            print("--github-token is required when --github-repo is set", file=sys.stderr)
            return 2
        samples.extend(
            collect_github_samples(
                repo=args.github_repo,
                token=args.github_token,
                days=args.days,
                artifact_name_prefix=args.artifact_name_prefix,
                sample_name=args.sample_name,
            )
        )

    summary = summarize_samples(samples, args.days, now)
    gate_status, gate_reasons = evaluate_gate(
        summary=summary,
        min_samples=args.min_samples,
        max_aggregate_rate=args.max_aggregate_rate,
        tcunit_max_rate=args.tcunit_max_rate,
    )

    payload: dict[str, Any] = {
        "version": 1,
        "generated_at": now.isoformat(),
        "inputs": {
            "input_glob": args.input_glob,
            "sample_file": args.sample_file,
            "github_repo": args.github_repo,
            "artifact_name_prefix": args.artifact_name_prefix,
            "sample_name": args.sample_name,
            "days": args.days,
            "min_samples": args.min_samples,
            "max_aggregate_rate": args.max_aggregate_rate,
            "tcunit_max_rate": args.tcunit_max_rate,
        },
        "summary": summary,
        "gate": {
            "status": gate_status,
            "reasons": gate_reasons,
            "enforced": bool(args.enforce_gate),
            "pending_ok": bool(args.pending_ok),
        },
    }

    output_json = Path(args.output_json)
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if args.output_md:
        write_markdown(Path(args.output_md), payload)

    print(
        "flake aggregate: "
        f"samples={summary['sample_count']} "
        f"coverage_days={summary['coverage_days']} "
        f"total_runs={summary['total_runs']} "
        f"total_failures={summary['total_failures']} "
        f"rate={float(summary['aggregate_flake_rate_percent']):.4f}% "
        f"gate={gate_status}"
    )

    if args.enforce_gate:
        if gate_status == "fail":
            return 1
        if gate_status == "pending" and not args.pending_ok:
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
