"""Single-command orchestration for the resumable cross-source catalogue campaign."""

from __future__ import annotations

import json
import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .code_spec_live import load_code_spec_metadata
from .coverage_trace_structural import structural_route_reason
from .test_catalog_models import InferredTestFact


EXECUTABLE_ROUTES = {
    "rust": frozenset({"rust_unit_test", "rust_integration_test"}),
    "direct": frozenset({"structured_text_test", "conformance_case"}),
    "vscode": frozenset({"vscode_test"}),
}
PILOT_MAPPED_LIMITS = {
    "rust": 16,
    "direct": 8,
    "vscode": 8,
}


def partition_campaign_facts(
    facts: Sequence[InferredTestFact],
) -> dict[str, list[str]]:
    routes: dict[str, list[str]] = {
        "structural": [],
        "rust": [],
        "direct": [],
        "vscode": [],
    }
    assigned: set[str] = set()
    for fact in facts:
        record = {
            "discovery_id": fact.stable_id,
            "discovery_source_kind": fact.source_kind,
            "ignore_state": fact.ignore_state,
        }
        if structural_route_reason(record) is not None:
            route = "structural"
        else:
            matching = [
                name
                for name, source_kinds in EXECUTABLE_ROUTES.items()
                if fact.source_kind in source_kinds
            ]
            if len(matching) != 1:
                raise ValueError(
                    f"{fact.stable_id} has no unique campaign route "
                    f"for {fact.source_kind}"
                )
            route = matching[0]
        if fact.stable_id in assigned:
            raise ValueError(f"campaign partition duplicates {fact.stable_id}")
        assigned.add(fact.stable_id)
        routes[route].append(fact.stable_id)
    if assigned != {fact.stable_id for fact in facts}:
        raise ValueError("campaign partition does not cover the live denominator")
    return {
        route: sorted(discovery_ids)
        for route, discovery_ids in routes.items()
    }


def select_pilot_ids(
    *,
    routes: Mapping[str, Sequence[str]],
    facts: Mapping[str, InferredTestFact],
    catalog_tests: Mapping[str, Mapping[str, Any]],
    preferred_rust_ids: Sequence[str] = (),
) -> dict[str, list[str]]:
    mapped = {
        str(record["discovery_id"])
        for record in catalog_tests.values()
        if record.get("subject_kind") == "generated_test"
        and isinstance(record.get("discovery_id"), str)
    }
    selected: dict[str, list[str]] = {}
    for route in ("rust", "direct", "vscode"):
        route_ids = list(routes[route])
        mapped_ids = [
            discovery_id for discovery_id in route_ids if discovery_id in mapped
        ]
        chosen = (
            [
                discovery_id
                for discovery_id in preferred_rust_ids
                if discovery_id in route_ids and discovery_id in mapped
            ][: PILOT_MAPPED_LIMITS[route]]
            if route == "rust" and preferred_rust_ids
            else mapped_ids[: PILOT_MAPPED_LIMITS[route]]
        )
        if route == "rust" and preferred_rust_ids and not chosen:
            raise ValueError("preferred Rust ground-truth sample is not live and mapped")
        if not chosen and route_ids:
            chosen = [route_ids[0]]
        # Every adapter subtype remains represented even when the existing
        # ground-truth catalog has no row for that subtype.
        represented_kinds = {
            facts[discovery_id].source_kind for discovery_id in chosen
        }
        for discovery_id in route_ids:
            source_kind = facts[discovery_id].source_kind
            if source_kind not in represented_kinds:
                chosen.append(discovery_id)
                represented_kinds.add(source_kind)
        selected[route] = sorted(set(chosen))
    empty = [route for route, discovery_ids in selected.items() if not discovery_ids]
    if empty:
        raise ValueError(f"pilot lacks executable routes: {empty}")
    return selected


def rank_traceable_rust_ground_truth(
    *,
    facts: Mapping[str, InferredTestFact],
    catalog_tests: Mapping[str, Mapping[str, Any]],
    code_mappings: Mapping[str, Mapping[str, Any]],
    code_facts: Mapping[str, Mapping[str, Any]],
    limit: int,
) -> list[str]:
    """Rank positive pilot rows from two existing authoritative mappings."""

    if limit < 1:
        raise ValueError("traceable ground-truth limit must be positive")
    paths_by_invariant: dict[str, list[str]] = {}
    for code_id, mapping in code_mappings.items():
        code_fact = code_facts.get(code_id)
        if mapping.get("disposition") != "specified" or code_fact is None:
            continue
        path = code_fact.get("path")
        invariant_ids = mapping.get("invariant_ids")
        if (
            not isinstance(path, str)
            or not path
            or not isinstance(invariant_ids, Sequence)
            or isinstance(invariant_ids, (str, bytes))
        ):
            continue
        for invariant_id in invariant_ids:
            if isinstance(invariant_id, str) and invariant_id:
                paths_by_invariant.setdefault(invariant_id, []).append(path)
    by_area: dict[str, list[tuple[int, int, str, str]]] = {}
    for test_id, record in catalog_tests.items():
        discovery_id = record.get("discovery_id")
        invariant_ids = record.get("invariants")
        if (
            record.get("subject_kind") != "generated_test"
            or not isinstance(discovery_id, str)
            or discovery_id not in facts
            or facts[discovery_id].source_kind
            not in {"rust_unit_test", "rust_integration_test"}
            or not isinstance(invariant_ids, Sequence)
            or isinstance(invariant_ids, (str, bytes))
        ):
            continue
        test_path = facts[discovery_id].path
        crate_prefix = _crate_prefix(test_path)
        mapped_paths = [
            path
            for invariant_id in invariant_ids
            if isinstance(invariant_id, str)
            for path in paths_by_invariant.get(invariant_id, [])
        ]
        same_path = sum(path == test_path for path in mapped_paths)
        same_crate = sum(
            _crate_prefix(path) == crate_prefix for path in mapped_paths
        )
        if same_path == 0:
            continue
        area = str(record.get("area", "unmapped"))
        by_area.setdefault(area, []).append(
            (-same_path, -same_crate, str(test_id), discovery_id)
        )
    for rows in by_area.values():
        rows.sort()
    selected: list[str] = []
    while len(selected) < limit and any(by_area.values()):
        for area in sorted(by_area):
            if by_area[area] and len(selected) < limit:
                selected.append(by_area[area].pop(0)[3])
    if not selected:
        raise ValueError(
            "no Rust catalog mapping shares an invariant with a specified "
            "code fact in its source file"
        )
    return sorted(selected)


def _crate_prefix(path: str) -> str:
    parts = path.split("/")
    return "/".join(parts[:2]) if len(parts) >= 2 and parts[0] == "crates" else path


def campaign_commands(
    *,
    root: Path,
    artifact_root: Path,
    cargo_target_dir: Path,
    llvm_tools_dir: Path,
    temporary_dir: Path,
    mode: str,
    pilot_ids: Mapping[str, Sequence[str]] | None,
    resume: bool,
) -> list[tuple[str, list[str]]]:
    if mode not in {"pilot", "full"}:
        raise ValueError(f"unsupported campaign mode {mode!r}")
    python = sys.executable
    resume_args = ["--resume"] if resume else []
    structural = artifact_root / "structural-trace.json"
    rust = artifact_root / "rust-trace.json"
    direct = artifact_root / "direct-trace.json"
    vscode = artifact_root / "vscode-trace.json"
    merged = artifact_root / "merged-trace.json"
    candidates = artifact_root / "mapping-candidates.json"
    proposals = (
        root / "verification/test-catalog-proposals.toml"
        if mode == "full"
        else artifact_root / "test-catalog-proposals.toml"
    )
    decisions = (
        root / "verification/test-catalog-mapping-decisions.toml"
        if mode == "full"
        else artifact_root / "test-catalog-mapping-decisions.toml"
    )
    rust_selection = (
        ["--all-rust"]
        if mode == "full"
        else [
            item
            for discovery_id in pilot_ids["rust"]
            for item in ("--discovery-id", discovery_id)
        ]
    )
    direct_selection = (
        ["--all-direct"]
        if mode == "full"
        else [
            item
            for discovery_id in pilot_ids["direct"]
            for item in ("--discovery-id", discovery_id)
        ]
    )
    vscode_selection = (
        ["--all-vscode"]
        if mode == "full"
        else [
            item
            for discovery_id in pilot_ids["vscode"]
            for item in ("--discovery-id", discovery_id)
        ]
    )
    merge_mode = "full_campaign" if mode == "full" else "focused_shard"
    commands = [
        (
            "structural trace",
            [
                python,
                str(root / "scripts/trace_structural_test_facts.py"),
                "--root",
                str(root),
                "--out",
                str(structural),
            ],
        ),
        (
            "Rust trace",
            [
                python,
                str(root / "scripts/trace_test_coverage.py"),
                "--root",
                str(root),
                *rust_selection,
                "--trace-root",
                str(artifact_root / "rust-attempts"),
                "--out",
                str(rust),
                "--cargo-target-dir",
                str(cargo_target_dir),
                "--llvm-tools-dir",
                str(llvm_tools_dir),
                "--temporary-dir",
                str(temporary_dir / "rust"),
                *resume_args,
            ],
        ),
        (
            "direct trace",
            [
                python,
                str(root / "scripts/trace_direct_test_coverage.py"),
                "--root",
                str(root),
                *direct_selection,
                "--trace-root",
                str(artifact_root / "direct-attempts"),
                "--out",
                str(direct),
                "--cargo-target-dir",
                str(cargo_target_dir),
                "--llvm-tools-dir",
                str(llvm_tools_dir),
                "--temporary-dir",
                str(temporary_dir / "direct"),
                *resume_args,
            ],
        ),
        (
            "VS Code trace",
            [
                python,
                str(root / "scripts/trace_vscode_test_coverage.py"),
                "--root",
                str(root),
                *vscode_selection,
                "--trace-root",
                str(artifact_root / "vscode-attempts"),
                "--out",
                str(vscode),
                "--cargo-target-dir",
                str(cargo_target_dir),
                "--temporary-dir",
                str(temporary_dir / "vscode"),
                *resume_args,
            ],
        ),
        (
            "trace merge",
            [
                python,
                str(root / "scripts/merge_test_coverage_traces.py"),
                "--root",
                str(root),
                "--input",
                str(structural),
                "--input",
                str(rust),
                "--input",
                str(direct),
                "--input",
                str(vscode),
                "--out",
                str(merged),
                "--mode",
                merge_mode,
                *(
                    ["--require-complete-denominator"]
                    if mode == "full"
                    else []
                ),
            ],
        ),
        (
            "candidate generation",
            [
                python,
                str(root / "scripts/generate_test_mapping_candidates.py"),
                "--root",
                str(root),
                "--trace",
                str(merged),
                "--candidates-out",
                str(candidates),
                "--proposals-out",
                str(proposals),
                "--decisions",
                str(decisions),
            ],
        ),
        (
            "numbered review card",
            [
                python,
                str(root / "scripts/prepare_test_mapping_review.py"),
                "--root",
                str(root),
                "--proposals",
                str(proposals),
                "--json-out",
                str(artifact_root / "mapping-review-card.json"),
                "--markdown-out",
                str(artifact_root / "mapping-review-card.md"),
            ],
        ),
    ]
    if any("test-all" in argument for _, command in commands for argument in command):
        raise ValueError("catalogue campaign must not contain the final full test gate")
    return commands


def execute_campaign_commands(
    commands: Sequence[tuple[str, Sequence[str]]],
    *,
    root: Path,
) -> None:
    for label, command in commands:
        print(f"catalogue campaign stage: {label}", flush=True)
        completed = subprocess.run(
            list(command),
            cwd=root,
            check=False,
        )
        if completed.returncode != 0:
            raise ValueError(
                f"catalogue campaign stage failed ({completed.returncode}): {label}"
            )
