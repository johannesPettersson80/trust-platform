"""Extract assertions for the exact Phase 18 runtime-cloud campaign."""

from __future__ import annotations

import argparse
import os
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .ads_behavior_proof_campaign import _write_json
from .exact_behavior_assertions import (
    build_assertion_facts as _build_assertion_facts,
)
from .mapping_assertions import invoke_rust_assertion_inventory
from .runtime_cloud_behavior_campaign import (
    DEFAULT_GAP_REPORT,
    DEFAULT_SCAN_REPORT,
)
from .runtime_cloud_behavior_proof import build_assignment_plan


DEFAULT_OUTPUT = Path(
    "target/gate-artifacts/verification/p18-runtime-cloud-23/"
    "assertions.json"
)


def build_assertion_facts(
    assignments: Sequence[Mapping[str, Any]],
) -> list[dict[str, str]]:
    """Convert the exact assignment into one Rust assertion request per test."""

    return _build_assertion_facts(assignments, label="runtime-cloud")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--gap-report", type=Path, default=DEFAULT_GAP_REPORT)
    parser.add_argument("--scan-report", type=Path, default=DEFAULT_SCAN_REPORT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args(argv)
    root = args.root.resolve()

    def rooted(path: Path) -> Path:
        return path if path.is_absolute() else root / path

    assignment = build_assignment_plan(
        root=root,
        gap_report=rooted(args.gap_report),
        scan_report=rooted(args.scan_report),
    )
    facts = build_assertion_facts(assignment["assignments"])
    inventories = invoke_rust_assertion_inventory(
        root,
        facts,
        environment=os.environ,
        workspace_root=root,
    )
    output = rooted(args.output)
    _write_json(
        output,
        {"schema_version": 2, "inventories": inventories},
    )
    print(
        f"runtime-cloud assertions extracted: {len(inventories)} -> "
        f"{output.relative_to(root)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
