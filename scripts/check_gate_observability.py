#!/usr/bin/env python3
"""Check that long-running CI/release gates use progress-visible child runs."""

from __future__ import annotations

import argparse
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
REQUIRED_MARKERS = {
    ".github/workflows/ci.yml": [
        "scripts/run_with_progress.py",
        "ci-cargo-build",
        "ci-runtime-warnings",
        "ci-full-test",
        "ci-conformance-pass-1",
        "ci-conformance-pass-2",
        "ci-vscode-binaries",
        "ci-vscode-extension",
    ],
    "scripts/runtime_comms_conformance_gate.sh": ["run_with_progress.py"],
    "scripts/runtime_comms_fuzz_gate.sh": ["run_with_progress.py"],
    "scripts/runtime_comms_bench_gate.sh": ["run_with_progress.py"],
    "scripts/runtime_cloud_security_profile_gate.sh": ["run_with_progress.py"],
    "scripts/runtime_mesh_tls_stability_gate.sh": ["run_with_progress.py"],
    "scripts/runtime_vm_bench_gate.sh": ["run_with_progress.py"],
    "scripts/runtime_vm_determinism_reliability_gate.sh": ["run_with_progress.py"],
    "scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh": ["run_with_progress.py"],
}


def findings_for_text(path: str, text: str) -> list[str]:
    findings: list[str] = []
    for marker in REQUIRED_MARKERS.get(path, []):
        if marker not in text:
            findings.append(f"{path}: missing observability marker {marker!r}")
    return findings


def run_self_test() -> int:
    bad = findings_for_text("scripts/runtime_comms_conformance_gate.sh", "cargo test\n")
    good = findings_for_text(
        "scripts/runtime_comms_conformance_gate.sh",
        "python3 scripts/run_with_progress.py -- cargo test\n",
    )
    errors: list[str] = []
    if not bad:
        errors.append("missing marker fixture did not fail")
    if good:
        errors.append(f"observed marker fixture failed: {good}")
    if errors:
        print("gate observability check self-test: failed")
        for error in errors:
            print(f"- {error}")
        return 1
    print("gate observability check self-test: ok")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return run_self_test()

    findings: list[str] = []
    for rel_path in sorted(REQUIRED_MARKERS):
        path = REPO_ROOT / rel_path
        if not path.is_file():
            findings.append(f"{rel_path}: missing required gate file")
            continue
        findings.extend(findings_for_text(rel_path, path.read_text(encoding="utf-8")))

    if findings:
        print("gate observability check: failed")
        for finding in findings:
            print(f"- {finding}")
        return 1
    print("gate observability check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
