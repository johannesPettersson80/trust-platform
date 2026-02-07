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
FAULT_RE = re.compile(r"\b(state=faulted|faulted=true|fault=true)\b", re.IGNORECASE)


def load_lines(path: Path) -> list[str]:
    if not path.exists():
        raise FileNotFoundError(f"log file not found: {path}")
    return [line.strip() for line in path.read_text(encoding="utf-8").splitlines()]


def summarize_load_log(lines: list[str]) -> dict[str, int | str | None]:
    samples = [line for line in lines if line and not line.startswith("#")]
    stats_unavailable = sum("stats=unavailable" in line for line in samples)
    first_ts = samples[0].split()[0] if samples else None
    last_ts = samples[-1].split()[0] if samples else None
    return {
        "samples": len(samples),
        "stats_unavailable": stats_unavailable,
        "first_timestamp": first_ts,
        "last_timestamp": last_ts,
    }


def summarize_soak_log(lines: list[str]) -> dict[str, object]:
    samples = [line for line in lines if line and not line.startswith("#")]
    cpu_values: list[float] = []
    rss_values: list[int] = []
    fault_mentions = 0
    restart_max = 0

    for line in samples:
        cpu_match = CPU_RE.search(line)
        if cpu_match:
            cpu_values.append(float(cpu_match.group(1)))

        rss_match = RSS_RE.search(line)
        if rss_match:
            rss_values.append(int(rss_match.group(1)))

        if FAULT_RE.search(line):
            fault_mentions += 1

        restart_match = RESTART_RE.search(line)
        if restart_match:
            restart_max = max(restart_max, int(restart_match.group(1)))

    first_ts = samples[0].split()[0] if samples else None
    last_ts = samples[-1].split()[0] if samples else None

    return {
        "samples": len(samples),
        "first_timestamp": first_ts,
        "last_timestamp": last_ts,
        "fault_mentions": fault_mentions,
        "restart_max": restart_max,
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


def write_markdown(path: Path, summary: dict[str, object]) -> None:
    load = summary["load"]
    soak = summary["soak"]
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
        f"- Stats unavailable entries: `{load['stats_unavailable']}`",
        f"- First timestamp: `{load['first_timestamp']}`",
        f"- Last timestamp: `{load['last_timestamp']}`",
        "",
        "## Soak Test",
        "",
        f"- Samples: `{soak['samples']}`",
        f"- Fault mentions: `{soak['fault_mentions']}`",
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
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Summarize runtime load/soak logs for reliability artifacts."
    )
    parser.add_argument("--load-log", required=True)
    parser.add_argument("--soak-log", required=True)
    parser.add_argument("--output-json", required=True)
    parser.add_argument("--output-md", required=True)
    args = parser.parse_args()

    load_lines_data = load_lines(Path(args.load_log))
    soak_lines_data = load_lines(Path(args.soak_log))

    load_summary = summarize_load_log(load_lines_data)
    soak_summary = summarize_soak_log(soak_lines_data)

    if load_summary["samples"] == 0:
        print(f"load log has no samples: {args.load_log}")
        return 1
    if soak_summary["samples"] == 0:
        print(f"soak log has no samples: {args.soak_log}")
        return 1

    summary: dict[str, object] = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "load": load_summary,
        "soak": soak_summary,
    }

    output_json = Path(args.output_json)
    output_md = Path(args.output_md)
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_md.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    write_markdown(output_md, summary)
    print(f"wrote {output_json} and {output_md}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
