"""Live Phase 11 hardware-lab state derived from committed sources."""

from __future__ import annotations

import platform as host_platform
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any

from .hardware_lab import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    HARDWARE_LAB_PATH,
    MANIFEST_SCHEMA_PATH,
    REPORT_SCHEMA_PATH,
    load_hardware_lab_document,
    validate_hardware_lab_document,
)
from .metadata_validator.core import Validator
from .report_input_contract import (
    resolve_report_output_path,
    validate_bound_input_paths,
    validator_code_input_paths,
)
from .test_catalog_common import input_digest


INPUT_PATHS = {
    ".github/workflows/protocol-device-in-loop.yml",
    "crates/trust-runtime/tests/device_in_the_loop.rs",
    "docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-07/device-in-loop-gates.md",
    "docs/specs/24-release-evidence.md",
    "examples/communication/gpio/README.md",
    "examples/communication/gpio/io.toml",
    "examples/communication/gpio/runtime.toml",
    "examples/communication/gpio/src/config.st",
    "examples/communication/gpio/src/main.st",
    "examples/communication/gpio/trust-lsp.toml",
    "scripts/gpio_hardware_test.sh",
    "scripts/report_hardware_lab.py",
    "scripts/runtime_device_in_loop_gate.sh",
    "scripts/validate_hardware_lab_report.py",
    "scripts/verification/hardware_lab.py",
    "scripts/verification/hardware_lab_cli.py",
    "scripts/verification/hardware_lab_live.py",
    "scripts/verification/hardware_lab_report.py",
    "scripts/verification/hardware_lab_validation.py",
    "verification/gate-inventory.toml",
    "verification/README.md",
    HARDWARE_LAB_PATH,
    "verification/ignored-tests.toml",
    "verification/release-evidence.toml",
    MANIFEST_SCHEMA_PATH,
    REPORT_SCHEMA_PATH,
    "verification/spec-sources.toml",
    "verification/suites/hardware-lab.toml",
}


@dataclass(frozen=True)
class HardwareLabState:
    commit: str
    branch: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    input_digest: str
    output_json: str
    output_markdown: str
    cases: tuple[dict[str, Any], ...]
    summary: dict[str, int]
    public_claim: dict[str, Any]


def build_live_hardware_lab_state(
    root: Path,
    *,
    branch: str,
    timestamp: str,
    json_path: Path = DEFAULT_JSON_PATH,
    markdown_path: Path = DEFAULT_MARKDOWN_PATH,
    require_clean_commit: bool = False,
) -> HardwareLabState:
    root = root.resolve()
    if not branch:
        raise ValueError("branch is required")
    _validate_timestamp(timestamp)
    output_json = resolve_report_output_path(root, json_path, "hardware-lab JSON")[0]
    output_markdown = resolve_report_output_path(root, markdown_path, "hardware-lab Markdown")[0]

    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        messages = "; ".join(f"{item.path}: {item.message}" for item in validator.failures)
        raise ValueError(f"verification metadata prerequisite failed: {messages}")
    document = load_hardware_lab_document(root)
    failures = validate_hardware_lab_document(
        root,
        document,
        ignored_tests=validator.ignored_tests,
        spec_sources=validator.spec_sources,
        suites=validator.suites,
        gate_inventory=validator.gate_inventory,
    )
    if failures:
        raise ValueError("; ".join(failures))

    inputs = tuple(sorted(INPUT_PATHS | validator_code_input_paths(root)))
    path_failures = validate_bound_input_paths(root, inputs)
    if path_failures:
        raise ValueError("; ".join(path_failures))
    commit = _head_commit(root, require_clean=require_clean_commit)
    cases = tuple(dict(row) for row in document["cases"])
    strict_cases = sum(row["binding_kind"] == "strict_harness" for row in cases)
    manual_cases = sum(row["binding_kind"] == "manual_script" for row in cases)
    summary = {
        "cases": len(cases),
        "protocols": len({row["protocol"] for row in cases}),
        "strict_harness_cases": strict_cases,
        "manual_script_cases": manual_cases,
        "skipped_unproven": sum(row["proof_status"] == "skipped_unproven" for row in cases),
        "evidence_records": sum(len(row["evidence_ids"]) for row in cases),
    }
    public_claim = {
        "status": document["public_claim_status"],
        "spec_source_id": document["hardware_claim_spec_source_id"],
        "hardware_qualified": False,
        "limitation": "No case has a reviewed passing named-topology lab artifact; public hardware documentation remains preview/unverified.",
    }
    return HardwareLabState(
        commit=commit,
        branch=branch,
        timestamp=timestamp,
        platform=host_platform.platform(),
        input_paths=inputs,
        input_digest=input_digest(root, list(inputs)),
        output_json=output_json,
        output_markdown=output_markdown,
        cases=cases,
        summary=summary,
        public_claim=public_claim,
    )


def validate_source_revision(root: Path, commit: object, input_paths: tuple[str, ...]) -> list[str]:
    if not isinstance(commit, str) or len(commit) != 40 or any(ch not in "0123456789abcdef" for ch in commit):
        return ["source commit must identify a clean full Git SHA"]
    failures: list[str] = []
    for relative in input_paths:
        exists = subprocess.run(
            ["git", "cat-file", "-e", f"{commit}:{relative}"],
            cwd=root,
            check=False,
            capture_output=True,
        )
        if exists.returncode:
            failures.append(f"source commit does not contain hardware-lab input {relative}")
            continue
        changed = subprocess.run(
            ["git", "diff", "--quiet", commit, "--", relative],
            cwd=root,
            check=False,
        )
        if changed.returncode:
            failures.append(f"hardware-lab input differs from source commit: {relative}")
    return failures


def _head_commit(root: Path, *, require_clean: bool) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=root, check=False, capture_output=True, text=True
    )
    commit = result.stdout.strip()
    if result.returncode or len(commit) != 40:
        raise ValueError("source commit must identify a full Git SHA")
    if require_clean:
        dirty = subprocess.run(
            ["git", "status", "--porcelain", "--untracked-files=all"],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if dirty.returncode or dirty.stdout:
            raise ValueError("source commit must identify a clean working tree")
    return commit


def _validate_timestamp(value: str) -> None:
    try:
        parsed = datetime.fromisoformat(value)
    except ValueError as exc:
        raise ValueError("timestamp must be ISO-8601 with a timezone") from exc
    if parsed.tzinfo is None:
        raise ValueError("timestamp must be ISO-8601 with a timezone")
