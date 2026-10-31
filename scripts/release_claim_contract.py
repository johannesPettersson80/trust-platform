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
    from scripts import check_npm_audit
    from scripts.check_dependency_exceptions import validate_file
    from scripts.release_evidence_contract import ReleaseEvidenceError
except ModuleNotFoundError:  # Direct `python scripts/...` execution.
    import check_npm_audit  # type: ignore[no-redef]
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
            "A Raspberry Pi runs the Linux AArch64 build",
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
            "and debugger behaviour is written down in a product specification",
            "checked by native executable",
            "depends on hardware or on the",
            "says what it was proven on",
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
            "npx --no-install vsce package --target ${{ matrix.platform }}",
            "cp target/${{ matrix.target }}/release/trust-lsp",
            "cp target/${{ matrix.target }}/release/trust-debug",
            "cp target/${{ matrix.target }}/release/trust-runtime",
            "trust-lsp.exe editors/vscode/bin/",
            "trust-debug.exe editors/vscode/bin/",
            "trust-runtime.exe editors/vscode/bin/",
            # Specification 44 §4.6: each platform's extension carries its toolchain bundle.
            "needs: [plan, release-preflight, toolchain-bundle]",
            "name: toolchain-${{ matrix.platform }}\n          path: editors/vscode/bin",
            'wanted = f"extension/bin/trust-toolchain-{platform}.tar.gz"',
        ],
        "VSIX target contract",
    )
    return {"targets": list(PLATFORMS), "embedded_binaries": 3, "carried_toolchain": True}


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
    for workflow in (".github/workflows/ci.yml", ".github/workflows/release.yml"):
        _require(_text(workflow), ["scripts/check_npm_audit.py"], f"VS Code dependency audit wiring in {workflow}")
    # The VS Code lockfile follows the same owned-exception policy (specification 24, "Dependency Exceptions").
    unowned, _stale, owned = check_npm_audit.findings(
        check_npm_audit.run_audit(ROOT), ROOT, date_from_environment()
    )
    if unowned:
        raise ReleaseEvidenceError("VS Code dependency audit: " + "; ".join(unowned))
    return {
        "rust_exceptions": len(rows),
        "vscode_exceptions": owned,
        "max_days": 90,
        "vscode_unowned_advisories": 0,
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
