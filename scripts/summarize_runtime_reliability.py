#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
from datetime import datetime, timezone
from pathlib import Path
from statistics import mean


CPU_RE = re.compile(r"\bcpu=([0-9]+(?:\.[0-9]+)?)\b")
RSS_RE = re.compile(r"\bmem_rss_kb=([0-9]+)\b")
RESTART_RE = re.compile(r"\brestarts?=([0-9]+)\b")
PROCESS_ALIVE_RE = re.compile(r"\bprocess_alive=(true|false)\b", re.IGNORECASE)
FAULT_STATE_RE = re.compile(r"\bstate=faulted\b", re.IGNORECASE)
FAULT_FIELD_RE = re.compile(r"\bfault=([^\s]+)\b", re.IGNORECASE)
TASK_STATS_RE = re.compile(
    r"\btask=([^\s]+)\s+min_ms=([0-9]+(?:\.[0-9]+)?)\s+"
    r"avg_ms=([0-9]+(?:\.[0-9]+)?)\s+max_ms=([0-9]+(?:\.[0-9]+)?)\s+"
    r"last_ms=([0-9]+(?:\.[0-9]+)?)\s+overruns=([0-9]+)\b"
)


def load_lines(path: Path) -> list[str]:
    if not path.exists():
        raise FileNotFoundError(f"log file not found: {path}")
    return [line.strip() for line in path.read_text(encoding="utf-8").splitlines()]


def first_timestamp(samples: list[str]) -> str | None:
    return samples[0].split()[0] if samples else None


def last_timestamp(samples: list[str]) -> str | None:
    return samples[-1].split()[0] if samples else None


def summarize_load_log(lines: list[str]) -> dict[str, object]:
    samples = [line for line in lines if line and not line.startswith("#")]
    stats_unavailable = sum("stats=unavailable" in line for line in samples)
    task_names: set[str] = set()
    overruns_max = 0
    worst_max_ms: float | None = None
    worst_jitter_ms: float | None = None
    avg_ms_values: list[float] = []
    task_sample_count = 0

    for line in samples:
        match = TASK_STATS_RE.search(line)
        if not match:
            continue
        task_name = match.group(1)
        min_ms = float(match.group(2))
        avg_ms = float(match.group(3))
        max_ms = float(match.group(4))
        overruns = int(match.group(6))
        jitter = max_ms - min_ms

        task_names.add(task_name)
        task_sample_count += 1
        avg_ms_values.append(avg_ms)
        overruns_max = max(overruns_max, overruns)
        worst_max_ms = max(max_ms, worst_max_ms or max_ms)
        worst_jitter_ms = max(jitter, worst_jitter_ms or jitter)

    return {
        "samples": len(samples),
        "task_samples": task_sample_count,
        "tasks": sorted(task_names),
        "stats_unavailable": stats_unavailable,
        "first_timestamp": first_timestamp(samples),
        "last_timestamp": last_timestamp(samples),
        "avg_task_avg_ms": mean(avg_ms_values) if avg_ms_values else None,
        "worst_max_ms": worst_max_ms,
        "worst_jitter_ms": worst_jitter_ms,
        "max_overruns": overruns_max,
    }


def has_fault_marker(line: str) -> bool:
    if FAULT_STATE_RE.search(line):
        return True
    fault_field = FAULT_FIELD_RE.search(line)
    if not fault_field:
        return False
    return fault_field.group(1).strip().lower() not in ("none", "0", "false")


def summarize_soak_log(lines: list[str]) -> dict[str, object]:
    samples = [line for line in lines if line and not line.startswith("#")]
    cpu_values: list[float] = []
    rss_values: list[int] = []
    fault_mentions = 0
    restart_max = 0
    process_exits = 0

    for line in samples:
        cpu_match = CPU_RE.search(line)
        if cpu_match:
            cpu_values.append(float(cpu_match.group(1)))

        rss_match = RSS_RE.search(line)
        if rss_match:
            rss_values.append(int(rss_match.group(1)))

        if has_fault_marker(line):
            fault_mentions += 1

        restart_match = RESTART_RE.search(line)
        if restart_match:
            restart_max = max(restart_max, int(restart_match.group(1)))

        process_alive_match = PROCESS_ALIVE_RE.search(line)
        if process_alive_match and process_alive_match.group(1).lower() == "false":
            process_exits += 1

    return {
        "samples": len(samples),
        "first_timestamp": first_timestamp(samples),
        "last_timestamp": last_timestamp(samples),
        "fault_mentions": fault_mentions,
        "restart_max": restart_max,
        "process_exits": process_exits,
        "cpu_pct": {
            "min": min(cpu_values) if cpu_values else None,
            "avg": mean(cpu_values) if cpu_values else None,
            "max": max(cpu_values) if cpu_values else None,
        },
        "mem_rss_kb": {
            "min": min(rss_values) if rss_values else None,
            "avg": mean(rss_values) if rss_values else None,
            "max": max(rss_values) if rss_values else None,
        },
    }


def format_number(value: float | int | None, digits: int = 2) -> str:
    if value is None:
        return "n/a"
    if isinstance(value, int):
        return str(value)
    return f"{value:.{digits}f}"


def build_gate_result(
    summary: dict[str, object],
    *,
    max_load_stats_unavailable: int,
    max_load_overruns: int,
    max_load_max_ms: float,
    max_load_jitter_ms: float,
    max_soak_fault_mentions: int,
    max_soak_process_exits: int,
    max_soak_restarts: int,
    max_soak_rss_kb: int,
    max_soak_cpu_pct: float,
) -> dict[str, object]:
    load = summary["load"]
    soak = summary["soak"]
    assert isinstance(load, dict)
    assert isinstance(soak, dict)
    soak_cpu = soak["cpu_pct"]
    soak_mem = soak["mem_rss_kb"]
    assert isinstance(soak_cpu, dict)
    assert isinstance(soak_mem, dict)

    failures: list[str] = []
    if int(load["samples"]) == 0:
        failures.append("load: no samples collected")
    if int(soak["samples"]) == 0:
        failures.append("soak: no samples collected")

    if int(load["stats_unavailable"]) > max_load_stats_unavailable:
        failures.append(
            "load: stats_unavailable="
            f"{load['stats_unavailable']} exceeds {max_load_stats_unavailable}"
        )
    if int(load["max_overruns"]) > max_load_overruns:
        failures.append(
            f"load: max_overruns={load['max_overruns']} exceeds {max_load_overruns}"
        )
    worst_max_ms = load.get("worst_max_ms")
    if worst_max_ms is None:
        failures.append("load: no task timing samples parsed")
    elif float(worst_max_ms) > max_load_max_ms:
        failures.append(
            f"load: worst_max_ms={worst_max_ms:.3f} exceeds {max_load_max_ms:.3f}"
        )
    worst_jitter_ms = load.get("worst_jitter_ms")
    if worst_jitter_ms is None:
        failures.append("load: no jitter samples parsed")
    elif float(worst_jitter_ms) > max_load_jitter_ms:
        failures.append(
            f"load: worst_jitter_ms={worst_jitter_ms:.3f} exceeds {max_load_jitter_ms:.3f}"
        )

    if int(soak["fault_mentions"]) > max_soak_fault_mentions:
        failures.append(
            f"soak: fault_mentions={soak['fault_mentions']} exceeds {max_soak_fault_mentions}"
        )
    if int(soak["process_exits"]) > max_soak_process_exits:
        failures.append(
            f"soak: process_exits={soak['process_exits']} exceeds {max_soak_process_exits}"
        )
    if int(soak["restart_max"]) > max_soak_restarts:
        failures.append(
            f"soak: restart_max={soak['restart_max']} exceeds {max_soak_restarts}"
        )
    soak_rss_max = soak_mem.get("max")
    if soak_rss_max is None:
        failures.append("soak: no RSS samples parsed")
    elif int(soak_rss_max) > max_soak_rss_kb:
        failures.append(
            f"soak: mem_rss_kb.max={soak_rss_max} exceeds {max_soak_rss_kb}"
        )
    soak_cpu_max = soak_cpu.get("max")
    if soak_cpu_max is None:
        failures.append("soak: no CPU samples parsed")
    elif float(soak_cpu_max) > max_soak_cpu_pct:
        failures.append(
            f"soak: cpu_pct.max={soak_cpu_max:.2f} exceeds {max_soak_cpu_pct:.2f}"
        )

    return {
        "passed": not failures,
        "failures": failures,
        "thresholds": {
            "max_load_stats_unavailable": max_load_stats_unavailable,
            "max_load_overruns": max_load_overruns,
            "max_load_max_ms": max_load_max_ms,
            "max_load_jitter_ms": max_load_jitter_ms,
            "max_soak_fault_mentions": max_soak_fault_mentions,
            "max_soak_process_exits": max_soak_process_exits,
            "max_soak_restarts": max_soak_restarts,
            "max_soak_rss_kb": max_soak_rss_kb,
            "max_soak_cpu_pct": max_soak_cpu_pct,
        },
    }


def write_markdown(path: Path, summary: dict[str, object]) -> None:
    load = summary["load"]
    soak = summary["soak"]
    gate = summary.get("gate")
    assert isinstance(load, dict)
    assert isinstance(soak, dict)
    cpu = soak["cpu_pct"]
    mem = soak["mem_rss_kb"]
    assert isinstance(cpu, dict)
    assert isinstance(mem, dict)

    lines = [
        "# Runtime Reliability Summary",
        "",
        f"- Generated: `{summary['generated_at']}`",
        "",
        "## Load Test",
        "",
        f"- Samples: `{load['samples']}`",
        f"- Parsed task samples: `{load['task_samples']}`",
        f"- Tasks: `{', '.join(load['tasks']) if load['tasks'] else 'n/a'}`",
        f"- Stats unavailable entries: `{load['stats_unavailable']}`",
        f"- Worst task max cycle (ms): `{format_number(load['worst_max_ms'])}`",
        f"- Worst task jitter (ms): `{format_number(load['worst_jitter_ms'])}`",
        f"- Max overruns counter: `{load['max_overruns']}`",
        f"- First timestamp: `{load['first_timestamp']}`",
        f"- Last timestamp: `{load['last_timestamp']}`",
        "",
        "## Soak Test",
        "",
        f"- Samples: `{soak['samples']}`",
        f"- Fault mentions: `{soak['fault_mentions']}`",
        f"- Process exits: `{soak['process_exits']}`",
        f"- Max restart counter: `{soak['restart_max']}`",
        f"- First timestamp: `{soak['first_timestamp']}`",
        f"- Last timestamp: `{soak['last_timestamp']}`",
        "",
        "## Resource Trends",
        "",
        "| Metric | Min | Avg | Max |",
        "| --- | ---: | ---: | ---: |",
        f"| CPU % | {format_number(cpu['min'])} | {format_number(cpu['avg'])} | {format_number(cpu['max'])} |",
        f"| RSS KB | {format_number(mem['min'])} | {format_number(mem['avg'])} | {format_number(mem['max'])} |",
        "",
    ]

    if isinstance(gate, dict):
        lines.append("## Gate Result")
        lines.append("")
        lines.append(f"- Passed: `{gate.get('passed')}`")
        failures = gate.get("failures")
        if isinstance(failures, list) and failures:
            lines.append("")
            lines.append("### Failures")
            lines.append("")
            for failure in failures:
                lines.append(f"- {failure}")
        lines.append("")

    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Summarize runtime load/soak logs for reliability artifacts."
    )
    parser.add_argument("--load-log", required=True)
    parser.add_argument("--soak-log", required=True)
    parser.add_argument("--output-json", required=True)
    parser.add_argument("--output-md", required=True)
    parser.add_argument(
        "--enforce-gates",
        action="store_true",
        help="Fail if reliability gate thresholds are exceeded.",
    )
    parser.add_argument("--max-load-stats-unavailable", type=int, default=0)
    parser.add_argument("--max-load-overruns", type=int, default=0)
    parser.add_argument("--max-load-max-ms", type=float, default=20.0)
    parser.add_argument("--max-load-jitter-ms", type=float, default=20.0)
    parser.add_argument("--max-soak-fault-mentions", type=int, default=0)
    parser.add_argument("--max-soak-process-exits", type=int, default=0)
    parser.add_argument("--max-soak-restarts", type=int, default=0)
    parser.add_argument("--max-soak-rss-kb", type=int, default=262_144)
    parser.add_argument("--max-soak-cpu-pct", type=float, default=95.0)
    args = parser.parse_args()

    load_lines_data = load_lines(Path(args.load_log))
    soak_lines_data = load_lines(Path(args.soak_log))

    load_summary = summarize_load_log(load_lines_data)
    soak_summary = summarize_soak_log(soak_lines_data)

    if int(load_summary["samples"]) == 0:
        print(f"load log has no samples: {args.load_log}")
        return 1
    if int(soak_summary["samples"]) == 0:
        print(f"soak log has no samples: {args.soak_log}")
        return 1

    summary: dict[str, object] = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "load": load_summary,
        "soak": soak_summary,
    }

    gate_result = build_gate_result(
        summary,
        max_load_stats_unavailable=args.max_load_stats_unavailable,
        max_load_overruns=args.max_load_overruns,
        max_load_max_ms=args.max_load_max_ms,
        max_load_jitter_ms=args.max_load_jitter_ms,
        max_soak_fault_mentions=args.max_soak_fault_mentions,
        max_soak_process_exits=args.max_soak_process_exits,
        max_soak_restarts=args.max_soak_restarts,
        max_soak_rss_kb=args.max_soak_rss_kb,
        max_soak_cpu_pct=args.max_soak_cpu_pct,
    )
    summary["gate"] = gate_result

    output_json = Path(args.output_json)
    output_md = Path(args.output_md)
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_md.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    write_markdown(output_md, summary)
    print(f"wrote {output_json} and {output_md}")

    if args.enforce_gates and not bool(gate_result["passed"]):
        failures = gate_result.get("failures", [])
        if isinstance(failures, list):
            for failure in failures:
                print(f"gate failure: {failure}")
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
