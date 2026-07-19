"""Pure assembly of ignored-test observations into an inventory payload."""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .ignored_test_discovery import (
    IgnoredDiscoveryBatch,
    associate_vscode_runtime_skips,
    scanner_ignore_fact,
    unsupported_scanner_ignore_diagnostics,
)
from .ignored_test_models import (
    IgnoredTestFact,
    InventoryDiagnostic,
    diagnostic_sort_key,
    record_sort_key,
)
from .test_catalog_models import InferredTestFact, ScanDiagnostic


LIMITATIONS = (
    "Discovery is static and recognizes only the ignore mechanisms named by this report.",
    "Runtime this.skip() is accepted only when contained in exactly one literal VS Code test callback.",
    "Rust files under xtask, root fuzz, and crate-local fuzz are bound by a fail-closed ignore-marker sentinel until identity support is added.",
    "Modeled Node identities are limited to VS Code Mocha and tracked Playwright capture specs; other tracked test/spec files use a fail-closed skip sentinel.",
    "Shell source has no repository-wide static ignored-test identity convention.",
    "Conformance runtime skipped results are outcomes, not source ignore declarations.",
    "Ignore classes, owners, areas, unblock conditions, expected behavior, and proof are hand-owned metadata.",
)

SURFACE_NOTES = {
    "rust": "Modeled crate Rust facts and fail-closed xtask/fuzz sentinel files are included in the scanned-file count.",
    "node": "Modeled VS Code facts and fail-closed excluded Node sentinel files are included in the scanned-file count.",
    "playwright": "Only same-line literal test.skip calls in tracked capture specs are inventory facts.",
    "shell": "Shell source has no repository-wide static ignored-test identity convention.",
    "conformance": "Runtime skipped results are not source ignore declarations.",
}


@dataclass(frozen=True)
class InventoryAnalysis:
    records: tuple[IgnoredTestFact, ...]
    diagnostics: tuple[InventoryDiagnostic, ...]
    surface_summary: tuple[dict[str, Any], ...]
    limitations: tuple[str, ...]

    @property
    def summary(self) -> dict[str, Any]:
        by_kind = Counter(item.discovery_source_kind for item in self.records)
        return {
            "records": len(self.records),
            "ignored": sum(item.ignore_state == "ignored" for item in self.records),
            "conditional": sum(
                item.ignore_state == "conditional" for item in self.records
            ),
            "diagnostics": len(self.diagnostics),
            "errors": sum(item.severity == "error" for item in self.diagnostics),
            "warnings": sum(item.severity == "warning" for item in self.diagnostics),
            "by_source_kind": [
                {"source_kind": source_kind, "records": count}
                for source_kind, count in sorted(by_kind.items())
            ],
        }


def build_inventory_payload(
    *,
    scanner_facts: list[InferredTestFact] | tuple[InferredTestFact, ...],
    scanner_diagnostics: list[ScanDiagnostic] | tuple[ScanDiagnostic, ...],
    playwright_facts: list[IgnoredTestFact] | tuple[IgnoredTestFact, ...],
    playwright_diagnostics: list[InventoryDiagnostic] | tuple[InventoryDiagnostic, ...],
    playwright_scanned_files: int | None = None,
    vscode_scanned_files: int | None = None,
    root: Path | None = None,
    additional_diagnostics: tuple[InventoryDiagnostic, ...] | list[InventoryDiagnostic] = (),
    excluded_rust_scanned_files: int = 0,
    excluded_node_scanned_files: int = 0,
) -> InventoryAnalysis:
    """Return an intent-free, exhaustive join of supported ignore observations."""

    scanner_records = [
        scanner_ignore_fact(fact)
        for fact in scanner_facts
        if fact.source_kind.startswith("rust_")
        and fact.ignore_state in {"ignored", "conditional"}
    ]
    runtime_diagnostics = [
        item for item in scanner_diagnostics if item.kind == "conditional_runtime_skip"
    ]
    if runtime_diagnostics and root is None:
        runtime = IgnoredDiscoveryBatch(
            diagnostics=[
                InventoryDiagnostic(
                    "error",
                    "vscode_runtime_skip_root_missing",
                    item.path,
                    item.line,
                    "workspace root is required for lexical runtime-skip containment",
                )
                for item in runtime_diagnostics
            ]
        )
    else:
        runtime = associate_vscode_runtime_skips(
            root or Path.cwd(), scanner_facts, scanner_diagnostics
        )
    records = [*scanner_records, *runtime.facts, *playwright_facts]
    diagnostics = [
        *runtime.diagnostics,
        *playwright_diagnostics,
        *additional_diagnostics,
        *unsupported_scanner_ignore_diagnostics(scanner_facts),
    ]
    diagnostics.extend(
        InventoryDiagnostic(
            item.severity,
            f"existing_catalog_{item.kind}",
            item.path,
            item.line,
            item.message,
        )
        for item in scanner_diagnostics
        if item.kind != "conditional_runtime_skip"
    )

    by_id: dict[str, IgnoredTestFact] = {}
    for record in records:
        if record.discovery_id in by_id:
            previous = by_id[record.discovery_id]
            diagnostics.append(
                InventoryDiagnostic(
                    "error",
                    "duplicate_ignore_discovery_id",
                    record.path,
                    record.line,
                    "ignore observations map the same discovery identity twice: "
                    f"{previous.ignore_mechanism}, {record.ignore_mechanism}",
                )
            )
            continue
        by_id[record.discovery_id] = record
    canonical_records = tuple(sorted(by_id.values(), key=lambda item: record_sort_key(item.to_dict())))
    canonical_diagnostics = tuple(
        sorted(diagnostics, key=lambda item: diagnostic_sort_key(item.to_dict()))
    )

    files_by_surface = {
        "rust": {
            fact.path for fact in scanner_facts if fact.source_kind.startswith("rust_")
        },
        "node": {fact.path for fact in scanner_facts if fact.source_kind == "vscode_test"},
        "playwright": {fact.path for fact in playwright_facts},
        "shell": {fact.path for fact in scanner_facts if fact.source_kind == "gate_script"},
        "conformance": {
            fact.path for fact in scanner_facts if fact.source_kind == "conformance_case"
        },
    }
    scanned_counts = {key: len(value) for key, value in files_by_surface.items()}
    if playwright_scanned_files is not None:
        scanned_counts["playwright"] = playwright_scanned_files
    if vscode_scanned_files is not None:
        scanned_counts["node"] = vscode_scanned_files
    scanned_counts["rust"] += excluded_rust_scanned_files
    scanned_counts["node"] += excluded_node_scanned_files
    surface_records = {
        "rust": [row for row in canonical_records if row.discovery_source_kind.startswith("rust_")],
        "node": [row for row in canonical_records if row.discovery_source_kind == "vscode_test"],
        "playwright": [
            row for row in canonical_records if row.discovery_source_kind == "playwright_test"
        ],
        "shell": [],
        "conformance": [],
    }
    surfaces = tuple(
        {
            "surface": surface,
            "scanned_files": scanned_counts[surface],
            "records": len(rows),
            "ignored": sum(row.ignore_state == "ignored" for row in rows),
            "conditional": sum(row.ignore_state == "conditional" for row in rows),
            "coverage": "limitation" if surface in {"shell", "conformance"} else "mechanical",
            "note": SURFACE_NOTES[surface],
        }
        for surface, rows in sorted(surface_records.items())
    )
    return InventoryAnalysis(
        records=canonical_records,
        diagnostics=canonical_diagnostics,
        surface_summary=surfaces,
        limitations=LIMITATIONS,
    )
