"""Live repository state for the Phase 3 ignored-test inventory."""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

from .ignored_test_discovery import (
    discover_excluded_node_skip_markers,
    discover_excluded_rust_ignore_markers,
    discover_playwright_skips,
    discover_vscode_unsupported_skip_markers,
)
from .ignored_test_report import InventoryAnalysis, build_inventory_payload
from .report_input_contract import validate_bound_input_paths
from .test_catalog_scanner import scan_repository


REPORT_SCHEMA_PATH = "verification/schemas/ignored-test-inventory-report.schema.json"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


@dataclass(frozen=True)
class LiveIgnoredTestInventoryState:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    analysis: InventoryAnalysis


def build_live_inventory_state(
    root: Path,
    *,
    timestamp: str | None = None,
    require_clean_commit: bool = False,
) -> LiveIgnoredTestInventoryState:
    root = root.resolve()
    scanner = scan_repository(root, timestamp=timestamp)
    scanner_payload = scanner.to_dict()
    if scanner_payload["scan_status"] != "complete":
        raise ValueError("existing-test scanner is incomplete")
    if require_clean_commit and not COMMIT_RE.fullmatch(scanner.provenance.commit):
        raise ValueError("source commit must identify a clean full Git SHA")
    playwright = discover_playwright_skips(root)
    vscode_unsupported = discover_vscode_unsupported_skip_markers(root)
    excluded_rust = discover_excluded_rust_ignore_markers(root)
    excluded_node = discover_excluded_node_skip_markers(root)
    analysis = build_inventory_payload(
        scanner_facts=list(scanner.inferred_facts),
        scanner_diagnostics=list(scanner.diagnostics),
        playwright_facts=playwright.facts,
        playwright_diagnostics=playwright.diagnostics,
        playwright_scanned_files=playwright.scanned_files,
        vscode_scanned_files=vscode_unsupported.scanned_files,
        root=root,
        additional_diagnostics=[
            *vscode_unsupported.diagnostics,
            *excluded_rust.diagnostics,
            *excluded_node.diagnostics,
        ],
        excluded_rust_scanned_files=excluded_rust.scanned_files,
        excluded_node_scanned_files=excluded_node.scanned_files,
    )
    if analysis.summary["errors"]:
        raise ValueError("ignored-test discovery produced error diagnostics")
    input_paths = tuple(
        sorted(
            {
                *scanner.provenance.input_paths,
                *vscode_unsupported.input_paths,
                *playwright.input_paths,
                *excluded_rust.input_paths,
                *excluded_node.input_paths,
                *_report_contract_paths(root),
            }
        )
    )
    failures = validate_bound_input_paths(root, input_paths)
    if failures:
        raise ValueError("; ".join(failures))
    return LiveIgnoredTestInventoryState(
        commit=scanner.provenance.commit,
        timestamp=scanner.provenance.timestamp,
        platform=scanner.provenance.platform,
        input_paths=input_paths,
        analysis=analysis,
    )


def _report_contract_paths(root: Path) -> set[str]:
    paths = {
        "scripts/report_ignored_test_inventory.py",
        "scripts/validate_ignored_test_inventory_report.py",
        "scripts/verification/ignored_test_cli.py",
        "scripts/verification/ignored_test_discovery.py",
        "scripts/verification/ignored_test_live.py",
        "scripts/verification/ignored_test_models.py",
        "scripts/verification/ignored_test_report.py",
        "scripts/verification/ignored_test_validation.py",
        "scripts/verification/report_input_contract.py",
        REPORT_SCHEMA_PATH,
        "verification/ignored-tests.toml",
        "verification/schemas/ignored-test.schema.json",
    }
    verification_dir = root / "scripts/verification"
    paths.update(
        path.relative_to(root).as_posix()
        for path in verification_dir.glob("test_catalog_*.py")
        if path.is_file() and not path.name.endswith("_tests.py")
    )
    return paths
