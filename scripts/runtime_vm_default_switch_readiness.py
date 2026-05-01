#!/usr/bin/env python3
"""Write the MP-060 Runtime VM default-switch readiness ledger."""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path
from typing import Any


PERFORMANCE_THRESHOLDS = {
    "aggregate_median_ratio_max": 0.50,
    "aggregate_p99_ratio_max": 0.70,
    "aggregate_throughput_ratio_min": 2.00,
}


def load_json(path: Path | None) -> dict[str, Any] | None:
    if path is None or not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def artifact_status(path: Path | None, expected_result: str | None = None) -> dict[str, Any]:
    if path is None:
        return {"status": "not-provided", "path": None}
    data = load_json(path)
    if data is None:
        return {"status": "missing", "path": str(path)}
    result = data.get("result")
    status = "present"
    if expected_result is not None:
        status = "pass" if result == expected_result else "unexpected-result"
    return {"status": status, "path": str(path), "result": result}


def benchmark_status(path: Path | None) -> tuple[dict[str, Any], list[dict[str, str]]]:
    data = load_json(path)
    if data is None:
        return (
            {"status": "missing", "path": str(path) if path else None},
            [
                {
                    "id": "MP060-PERF-EVIDENCE",
                    "status": "blocked",
                    "detail": "No current benchmark summary was supplied to the readiness ledger.",
                }
            ],
        )

    aggregate = data.get("aggregate", {})
    ratios = data.get("ratios") or data.get("aggregate_ratios")
    if not isinstance(ratios, dict):
        return (
            {
                "status": "recorded-not-thresholded",
                "path": str(path),
                "aggregate": aggregate,
                "thresholds": PERFORMANCE_THRESHOLDS,
            },
            [
                {
                    "id": "MP060-PERF-THRESHOLDS",
                    "status": "blocked",
                    "detail": "Benchmark summary records fixture timings but does not contain aggregate median/p99/throughput ratios for the MP-060 default-switch thresholds.",
                }
            ],
        )

    median = float(ratios.get("median", float("inf")))
    p99 = float(ratios.get("p99", float("inf")))
    throughput = float(ratios.get("throughput", 0.0))
    passed = (
        median <= PERFORMANCE_THRESHOLDS["aggregate_median_ratio_max"]
        and p99 <= PERFORMANCE_THRESHOLDS["aggregate_p99_ratio_max"]
        and throughput >= PERFORMANCE_THRESHOLDS["aggregate_throughput_ratio_min"]
    )
    risk = []
    if not passed:
        risk.append(
            {
                "id": "MP060-PERF-THRESHOLDS",
                "status": "blocked",
                "detail": f"Current ratios median={median}, p99={p99}, throughput={throughput} do not meet MP-060 default-switch thresholds.",
            }
        )
    return (
        {
            "status": "pass" if passed else "blocked",
            "path": str(path),
            "ratios": {
                "median": median,
                "p99": p99,
                "throughput": throughput,
            },
            "thresholds": PERFORMANCE_THRESHOLDS,
        },
        risk,
    )


def build_ledger(args: argparse.Namespace) -> dict[str, Any]:
    benchmark, benchmark_risks = benchmark_status(
        Path(args.benchmark_summary) if args.benchmark_summary else None
    )
    residual_risks = [
        *benchmark_risks,
        {
            "id": "MP060-ROLLBACK-MODEL",
            "status": "controlled",
            "detail": "Rollback is versioned release rollback; interpreter startup fallback is intentionally blocked by the production backend guard.",
        },
    ]
    evidence = {
        "production_backend_guard": {
            "status": args.production_guard_status,
            "command": "./scripts/runtime_vm_production_backend_guard.sh",
        },
        "register_stack_differential": {
            "status": args.differential_status,
            "command": "cargo test -p trust-runtime --test bytecode_vm_differential -- --nocapture",
        },
        "malformed_bytecode_fuzz": artifact_status(
            Path(args.fuzz_summary) if args.fuzz_summary else None,
            expected_result="pass",
        ),
        "benchmark": benchmark,
    }
    blocking = [
        risk for risk in residual_risks if risk["status"] == "blocked"
    ] + [
        {"id": key, "status": value["status"], "detail": "Required evidence did not pass."}
        for key, value in evidence.items()
        if value["status"] not in {"pass", "present"}
    ]
    return {
        "result": "ready" if not blocking else "blocked",
        "thresholds": PERFORMANCE_THRESHOLDS,
        "evidence": evidence,
        "residual_risks": residual_risks,
        "blocking_items": blocking,
    }


def render_markdown(ledger: dict[str, Any]) -> str:
    lines = [
        "# Runtime VM Default-Switch Readiness Ledger",
        "",
        f"- result: {ledger['result']}",
        "",
        "## Evidence",
    ]
    for name, entry in ledger["evidence"].items():
        detail = entry.get("path") or entry.get("command") or ""
        lines.append(f"- {name}: {entry['status']} {detail}".rstrip())
    lines.extend(["", "## Residual Risks"])
    for risk in ledger["residual_risks"]:
        lines.append(f"- {risk['id']} ({risk['status']}): {risk['detail']}")
    if ledger["blocking_items"]:
        lines.extend(["", "## Blocking Items"])
        for item in ledger["blocking_items"]:
            lines.append(f"- {item['id']} ({item['status']}): {item['detail']}")
    lines.append("")
    return "\n".join(lines)


def write_outputs(ledger: dict[str, Any], out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "summary.json").write_text(json.dumps(ledger, indent=2), encoding="utf-8")
    (out_dir / "summary.md").write_text(render_markdown(ledger), encoding="utf-8")


def self_test() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        fuzz = root / "fuzz.json"
        fuzz.write_text(json.dumps({"result": "pass"}), encoding="utf-8")
        args = argparse.Namespace(
            out_dir=str(root / "out"),
            fuzz_summary=str(fuzz),
            benchmark_summary=None,
            production_guard_status="pass",
            differential_status="pass",
        )
        ledger = build_ledger(args)
        assert ledger["result"] == "blocked"
        assert ledger["evidence"]["malformed_bytecode_fuzz"]["status"] == "pass"
        assert any(item["id"] == "MP060-PERF-EVIDENCE" for item in ledger["blocking_items"])
        write_outputs(ledger, Path(args.out_dir))
        assert (Path(args.out_dir) / "summary.json").is_file()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out-dir", default="target/gate-artifacts/runtime-vm-default-switch-readiness")
    parser.add_argument("--fuzz-summary")
    parser.add_argument("--benchmark-summary")
    parser.add_argument("--production-guard-status", choices=["pass", "fail", "not-run"], default="not-run")
    parser.add_argument("--differential-status", choices=["pass", "fail", "not-run"], default="not-run")
    parser.add_argument("--require-ready", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        print("runtime VM readiness ledger self-test: ok")
        return 0

    ledger = build_ledger(args)
    write_outputs(ledger, Path(args.out_dir))
    print(f"runtime VM readiness ledger: {ledger['result']}")
    print(f"summary: {Path(args.out_dir) / 'summary.json'}")
    return 1 if args.require_ready and ledger["result"] != "ready" else 0


if __name__ == "__main__":
    raise SystemExit(main())
