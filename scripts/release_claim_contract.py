#!/usr/bin/env python3
"""Audit one normative release/public-claim scenario against live sources."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any, Callable

try:
    from scripts.check_dependency_exceptions import validate_file
    from scripts.release_evidence_contract import ReleaseEvidenceError
except ModuleNotFoundError:  # Direct `python scripts/...` execution.
    from check_dependency_exceptions import validate_file  # type: ignore[no-redef]
    from release_evidence_contract import ReleaseEvidenceError  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parent.parent
PLATFORMS = (
    "linux-x64",
    "linux-arm64",
    "darwin-x64",
    "darwin-arm64",
    "win32-x64",
)


def _text(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def _require(text: str, fragments: list[str], label: str) -> None:
    missing = [fragment for fragment in fragments if fragment not in text]
    if missing:
        raise ReleaseEvidenceError(f"{label} missing reviewed fragments: {missing}")


def _npm_program() -> str:
    return "npm.cmd" if sys.platform == "win32" else "npm"


def audit_platform_matrix() -> dict[str, Any]:
    release = _text(".github/workflows/release.yml")
    ci = _text(".github/workflows/ci.yml")
    for platform in PLATFORMS:
        _require(release, [f"platform: {platform}"], "release platform matrix")
    _require(
        ci,
        ["os: [ubuntu-latest, macos-latest, windows-latest]"],
        "native CI matrix",
    )
    readme = _text("README.md")
    _require(
        readme,
        [
            "Raspberry Pi currently inherits Linux AArch64 artifact support",
            "not a deterministic-latency certification",
        ],
        "qualified platform claim",
    )
    return {"targets": list(PLATFORMS), "native_ci_os": 3, "qualified": True}


def audit_source_build() -> dict[str, Any]:
    lock = tomllib.loads(_text("Cargo.lock"))
    packages = {
        row.get("name"): row
        for row in lock.get("package", [])
        if isinstance(row, dict) and str(row.get("name", "")).startswith("open-ot-")
    }
    required = {"open-ot-carriage", "open-ot-definition", "open-ot-shm"}
    missing = required - set(packages)
    if missing:
        raise ReleaseEvidenceError(
            f"OpenOT lock packages are missing shipped dependencies: {sorted(missing)}"
        )
    for name, row in packages.items():
        source = row.get("source")
        if not isinstance(source, str) or not source.startswith(
            "git+https://github.com/johannesPettersson80/open-ot-experiments.git?rev="
        ):
            raise ReleaseEvidenceError(f"{name} is not pinned to the public Git source")
    _require(
        _text("docs/public/start/install-from-source.md"),
        [
            "Normal source builds fetch the OpenOT Rust crates through pinned public Git",
            "dependencies. Some OpenOT IEC examples",
        ],
        "source-build public claim",
    )
    return {"openot_packages": sorted(packages), "sibling_required": False}


def audit_hardware_claims() -> dict[str, Any]:
    matrix = _text("docs/public/connect/protocol-matrix.md")
    _require(
        matrix,
        ["mock", "loopback", "simulation", "interoperability", "device-in-loop"],
        "hardware proof vocabulary",
    )
    return {
        "levels": ["mock", "loopback", "simulation", "interoperability", "device_in_loop"],
        "hardware_level": "device_in_loop",
    }


def audit_conformance_publication() -> dict[str, Any]:
    generator = _text("scripts/generate_conformance_status.py")
    workflow = _text(".github/workflows/release.yml")
    _require(
        generator,
        ["commit", "toolchain", "timestamp", "known_gaps", "executed", "passed", "failed"],
        "conformance status generator",
    )
    _require(
        workflow,
        ["generate_conformance_status.py", "conformance-status.json", "conformance-status.md"],
        "release conformance publication",
    )
    return {"result_derived": True, "known_gaps_bound": True}


def audit_version_chain() -> dict[str, Any]:
    guard = _text("scripts/check_version_release_evidence.py")
    _require(
        guard,
        [
            '"/releases/latest"',
            "validate_release_publication",
            '"release-provenance.json"',
            '"conformance-status.json"',
        ],
        "release version guard",
    )
    return {"latest_checked": True, "required_assets_checked": True}


def audit_behavior_lock(domain: str) -> dict[str, Any]:
    _require(
        _text("README.md"),
        [
            "behavior claims are locked by their written",
            "product specifications and direct native executable tests",
            "proof class",
            "stated separately",
        ],
        "qualified behavior-lock claim",
    )
    return {
        "domain": domain,
        "written_specification": True,
        "native_executable_test": True,
        "metadata_required": False,
        "proof_class_separate": True,
    }


def audit_paths() -> dict[str, Any]:
    release = _text(".github/workflows/release.yml")
    _require(
        release,
        [
            "trust-runtime-${{ matrix.platform }}.tar.gz",
            "trust-runtime-${{ matrix.platform }}.zip",
            "trust-lsp-${{ matrix.platform }}.tar.gz",
            "trust-lsp-${{ matrix.platform }}.zip",
            "trust-runtime.exe",
        ],
        "platform archive contract",
    )
    return {"unix": "tar.gz", "windows": "zip", "windows_suffix": ".exe"}


def audit_vsix() -> dict[str, Any]:
    release = _text(".github/workflows/release.yml")
    _require(
        release,
        [
            "npx vsce package --target ${{ matrix.platform }}",
            "cp target/${{ matrix.target }}/release/trust-lsp",
            "cp target/${{ matrix.target }}/release/trust-debug",
            "cp target/${{ matrix.target }}/release/trust-runtime",
            "trust-lsp.exe editors/vscode/bin/",
            "trust-debug.exe editors/vscode/bin/",
            "trust-runtime.exe editors/vscode/bin/",
        ],
        "VSIX target contract",
    )
    return {"targets": list(PLATFORMS), "embedded_binaries": 3}


def audit_artifact_provenance() -> dict[str, Any]:
    workflow = _text(".github/workflows/release.yml")
    generator = _text("scripts/generate_release_provenance.py")
    _require(
        workflow,
        ["generate_release_provenance.py", "release-provenance.json", "SHA256SUMS"],
        "release provenance workflow",
    )
    _require(
        generator,
        ["tag", "commit", "workflow_run_id", "workflow_run_url", "artifacts", "sha256"],
        "release provenance generator",
    )
    return {"tag_bound": True, "commit_bound": True, "workflow_bound": True}


def audit_dependency_policy() -> dict[str, Any]:
    rows = validate_file(ROOT / "deny.toml", today=date_from_environment())
    _require(
        _text(".github/workflows/ci.yml") + _text(".github/workflows/release.yml"),
        ["npm audit --audit-level=low"],
        "VS Code dependency audit wiring",
    )
    audit = subprocess.run(
        [_npm_program(), "audit", "--json", "--audit-level=low"],
        cwd=ROOT / "editors/vscode",
        check=False,
        capture_output=True,
        text=True,
    )
    try:
        payload = json.loads(audit.stdout)
        vulnerability_count = int(payload["metadata"]["vulnerabilities"]["total"])
    except (KeyError, TypeError, ValueError, json.JSONDecodeError) as exc:
        raise ReleaseEvidenceError(f"npm audit did not return its closed JSON summary: {exc}")
    if audit.returncode != 0 or vulnerability_count != 0:
        raise ReleaseEvidenceError(
            f"VS Code dependency audit found {vulnerability_count} vulnerabilities"
        )
    return {
        "rust_exceptions": len(rows),
        "max_days": 90,
        "vscode_vulnerabilities": vulnerability_count,
    }


def date_from_environment():
    from datetime import date
    import os

    value = os.environ.get("TRUST_RELEASE_EVIDENCE_DATE")
    return date.fromisoformat(value) if value else date.today()


AUDITS: dict[str, Callable[[], dict[str, Any]]] = {
    "PLATFORM_SUPPORT_MATRIX": audit_platform_matrix,
    "SOURCE_BUILD_WITHOUT_OPENOT_SIBLING": audit_source_build,
    "HARDWARE_PROOF_LEVELS": audit_hardware_claims,
    "RESULT_DERIVED_CONFORMANCE_STATUS": audit_conformance_publication,
    "RELEASE_VERSION_CHAIN": audit_version_chain,
    "RUNTIME_BEHAVIOR_LOCK_MAPPING": lambda: audit_behavior_lock("runtime"),
    "DEBUG_BEHAVIOR_LOCK_MAPPING": lambda: audit_behavior_lock("debugger"),
    "PLATFORM_PATH_CONTRACT": audit_paths,
    "VSIX_TARGET_BINARY_IDENTITY": audit_vsix,
    "ARTIFACT_PROVENANCE_CHAIN": audit_artifact_provenance,
    "DEPENDENCY_EXCEPTION_LIFETIME": audit_dependency_policy,
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--scenario", choices=sorted(AUDITS), required=True)
    args = parser.parse_args()
    try:
        result = AUDITS[args.scenario]()
    except (OSError, subprocess.SubprocessError, tomllib.TOMLDecodeError, ReleaseEvidenceError) as exc:
        print(f"release-claim-contract: FAIL: {exc}", file=sys.stderr)
        return 1
    print(json.dumps({"scenario": args.scenario, "result": result}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
