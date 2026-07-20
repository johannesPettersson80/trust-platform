"""Live, fail-closed ignored-test registry staleness validation."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

from .ignored_test_discovery import discover_repository_ignored_tests
from .ignored_test_models import InventoryDiagnostic
from .metadata_validator.constants import ROOT
from .metadata_validator.core import Validator
from .metadata_validator.ignored_tests import (
    load_checklist_row_ids,
    validate_ignored_test_records,
)


@dataclass(frozen=True)
class IgnoredTestStalenessResult:
    failures: tuple[str, ...]
    discovered: int
    registered: int
    unknown: int
    catalog_mapped: int


def validate_live_ignored_test_registry(root: Path) -> IgnoredTestStalenessResult:
    root = root.resolve()
    if root != ROOT.resolve():
        return IgnoredTestStalenessResult(
            failures=("--root must identify the repository that loaded verification modules",),
            discovered=0,
            registered=0,
            unknown=0,
            catalog_mapped=0,
        )

    validator = Validator()
    validator.load_records()
    validator.validate()
    failures = [
        f"metadata: {failure.path}: {failure.message}" for failure in validator.failures
    ]
    try:
        discovery = discover_repository_ignored_tests(root)
    except Exception as exc:
        failures.append(f"ignored-test discovery failed: {exc}")
        discovery_facts = []
    else:
        discovery_facts = discovery.facts
        failures.extend(blocking_discovery_failures(discovery.diagnostics))
        failures.extend(
            validate_ignored_test_records(
                root=root,
                ignored_tests=validator.ignored_tests,
                tests=validator.tests,
                checklist_row_ids=load_checklist_row_ids(root),
                facts=discovery_facts,
            )
        )

    records = list(validator.ignored_tests.values())
    return IgnoredTestStalenessResult(
        failures=tuple(sorted(set(failures))),
        discovered=len(discovery_facts),
        registered=len(records),
        unknown=sum(record.get("ignore_class") == "unknown" for record in records),
        catalog_mapped=sum(isinstance(record.get("test_id"), str) for record in records),
    )


def blocking_discovery_failures(
    diagnostics: Sequence[InventoryDiagnostic],
) -> list[str]:
    """Treat every error and every unresolved ignore/skip warning as incomplete."""

    return [
        f"discovery: {item.path}:{item.line}: {item.kind}: {item.message}"
        for item in diagnostics
        if item.severity == "error"
        or "ignore" in item.kind
        or "skip" in item.kind
    ]


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args(argv)
    result = validate_live_ignored_test_registry(args.root)
    if result.failures:
        print("ignored-test staleness validation failed:", file=sys.stderr)
        for failure in result.failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print(
        "ignored-test registry validated: "
        f"{result.discovered} discovered, {result.registered} registered, "
        f"{result.unknown} unknown (report-only), "
        f"{result.catalog_mapped} catalog-mapped"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
