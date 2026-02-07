#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
from datetime import datetime, timezone
from pathlib import Path


CHECKBOX_RE = re.compile(r"^\s*-\s*\[([ xX~])\]\s+")


def parse_status_pairs(pairs: list[str]) -> dict[str, str]:
    statuses: dict[str, str] = {}
    for item in pairs:
        if "=" not in item:
            raise ValueError(f"invalid --job-status '{item}', expected name=result")
        name, value = item.split("=", 1)
        name = name.strip()
        value = value.strip()
        if not name:
            raise ValueError(f"invalid --job-status '{item}', missing name")
        statuses[name] = value
    return statuses


def parse_checklist(path: Path) -> dict[str, int]:
    total = 0
    checked = 0
    partial = 0
    unchecked = 0
    for line in path.read_text(encoding="utf-8").splitlines():
        match = CHECKBOX_RE.match(line)
        if not match:
            continue
        total += 1
        mark = match.group(1)
        if mark in ("x", "X"):
            checked += 1
        elif mark == "~":
            partial += 1
        else:
            unchecked += 1
    return {
        "total": total,
        "checked": checked,
        "partial": partial,
        "unchecked": unchecked,
    }


def discover_gate_artifacts(path: Path) -> list[str]:
    if not path.exists():
        return []
    return sorted(p.name for p in path.iterdir() if p.is_dir())


def write_markdown(
    output_path: Path,
    generated_at: str,
    gate_artifacts_present: list[str],
    missing_gates: list[str],
    job_statuses: dict[str, str],
    failed_jobs: list[str],
    checklist_statuses: dict[str, dict[str, int] | str],
    verdict: str,
) -> None:
    lines: list[str] = []
    lines.append("# Release Gate Report")
    lines.append("")
    lines.append(f"- Generated: `{generated_at}`")
    lines.append(f"- Verdict: **{verdict}**")
    lines.append("")
    lines.append("## Gate Artifacts")
    lines.append("")
    if gate_artifacts_present:
        for gate in gate_artifacts_present:
            lines.append(f"- [x] `{gate}`")
    else:
        lines.append("- [ ] No gate artifacts found")
    if missing_gates:
        lines.append("")
        lines.append("### Missing Required Gate Artifacts")
        lines.append("")
        for gate in missing_gates:
            lines.append(f"- [ ] `{gate}`")

    lines.append("")
    lines.append("## Job Status")
    lines.append("")
    for name in sorted(job_statuses):
        value = job_statuses[name]
        marker = "x" if value == "success" else " "
        lines.append(f"- [{marker}] `{name}`: `{value}`")
    if failed_jobs:
        lines.append("")
        lines.append("### Failed Jobs")
        lines.append("")
        for name in failed_jobs:
            lines.append(f"- [ ] `{name}`")

    lines.append("")
    lines.append("## Checklist Status")
    lines.append("")
    for checklist in sorted(checklist_statuses):
        info = checklist_statuses[checklist]
        if isinstance(info, str):
            lines.append(f"- [ ] `{checklist}`: {info}")
        else:
            lines.append(
                f"- `{checklist}`: "
                f"checked={info['checked']}, partial={info['partial']}, "
                f"unchecked={info['unchecked']}, total={info['total']}"
            )

    output_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate a release gate report and fail when required artifacts are missing."
    )
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--gate-artifacts-dir", required=True)
    parser.add_argument("--required-gate", action="append", default=[])
    parser.add_argument("--job-status", action="append", default=[])
    parser.add_argument("--checklist", action="append", default=[])
    args = parser.parse_args()

    output_dir = Path(args.output_dir)
    gate_artifacts_dir = Path(args.gate_artifacts_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    try:
        job_statuses = parse_status_pairs(args.job_status)
    except ValueError as error:
        print(error)
        return 2

    gate_artifacts_present = discover_gate_artifacts(gate_artifacts_dir)
    missing_gates = sorted(set(args.required_gate) - set(gate_artifacts_present))

    failed_jobs = sorted(
        name for name, status in job_statuses.items() if status != "success"
    )

    checklist_statuses: dict[str, dict[str, int] | str] = {}
    missing_checklists: list[str] = []
    for checklist in args.checklist:
        checklist_path = Path(checklist)
        if not checklist_path.exists():
            checklist_statuses[checklist] = "missing file"
            missing_checklists.append(checklist)
            continue
        checklist_statuses[checklist] = parse_checklist(checklist_path)

    generated_at = datetime.now(timezone.utc).isoformat()
    verdict = (
        "PASS"
        if not missing_gates and not failed_jobs and not missing_checklists
        else "FAIL"
    )

    report = {
        "generated_at": generated_at,
        "verdict": verdict,
        "required_gate_artifacts": sorted(args.required_gate),
        "gate_artifacts_present": gate_artifacts_present,
        "missing_required_gate_artifacts": missing_gates,
        "job_statuses": job_statuses,
        "failed_jobs": failed_jobs,
        "checklists": checklist_statuses,
        "missing_checklists": missing_checklists,
    }

    json_path = output_dir / "release-gate-report.json"
    md_path = output_dir / "release-gate-report.md"
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    write_markdown(
        output_path=md_path,
        generated_at=generated_at,
        gate_artifacts_present=gate_artifacts_present,
        missing_gates=missing_gates,
        job_statuses=job_statuses,
        failed_jobs=failed_jobs,
        checklist_statuses=checklist_statuses,
        verdict=verdict,
    )

    if verdict == "FAIL":
        if missing_gates:
            print("missing required gate artifacts:")
            for gate in missing_gates:
                print(f"  - {gate}")
        if failed_jobs:
            print("non-success jobs:")
            for job in failed_jobs:
                print(f"  - {job} ({job_statuses[job]})")
        if missing_checklists:
            print("missing checklist files:")
            for checklist in missing_checklists:
                print(f"  - {checklist}")
        return 1

    print(f"wrote {json_path} and {md_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
