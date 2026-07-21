"""Joins Phase 17 tooling facts to existing gate and fuzz inventories."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Mapping, Sequence

from .code_spec_common import CodeFact


@dataclass(frozen=True)
class ToolingJoinResult:
    bindings: Mapping[str, tuple[str, ...]] = field(default_factory=dict)
    failures: tuple[str, ...] = ()


def join_tooling_inventories(
    *,
    facts: Mapping[str, CodeFact],
    source_paths: Sequence[str],
    gate_records: Mapping[str, Mapping[str, Any]],
    fuzz_targets: Sequence[Mapping[str, Any]],
) -> ToolingJoinResult:
    paths = set(source_paths)
    facts_by_path: dict[str, list[str]] = {}
    for code_id, fact in facts.items():
        facts_by_path.setdefault(fact.path, []).append(code_id)
    bindings: dict[str, set[str]] = {}
    failures: list[str] = []

    gate_paths: dict[str, str] = {}
    for gate_id, record in sorted(gate_records.items()):
        if record.get("source_kind") != "gate_script":
            continue
        path = record.get("path")
        if not isinstance(path, str) or not path:
            failures.append(f"{gate_id}: gate inventory record lacks path")
            continue
        if path in gate_paths:
            failures.append(
                f"gate inventory path {path} is duplicated by {gate_paths[path]} and {gate_id}"
            )
        gate_paths[path] = gate_id
        code_ids = facts_by_path.get(path, [])
        if path not in paths or not code_ids:
            failures.append(f"{gate_id}: gate inventory path has no tooling fact: {path}")
        for code_id in code_ids:
            bindings.setdefault(code_id, set()).add(f"gate:{gate_id}")

    fuzz_paths: dict[str, str] = {}
    for index, target in enumerate(fuzz_targets):
        if target.get("target_kind") != "cargo_fuzz":
            continue
        target_id = target.get("id")
        path = target.get("path")
        if not isinstance(target_id, str) or not isinstance(path, str):
            failures.append(f"fuzz target[{index}] lacks id or path")
            continue
        if path in fuzz_paths:
            failures.append(
                f"cargo-fuzz path {path} is duplicated by {fuzz_paths[path]} and {target_id}"
            )
        fuzz_paths[path] = target_id
        code_ids = facts_by_path.get(path, [])
        if path not in paths or not code_ids:
            failures.append(f"{target_id}: registered cargo-fuzz path has no tooling fact: {path}")
        for code_id in code_ids:
            bindings.setdefault(code_id, set()).add(f"fuzz:{target_id}")

    observed_fuzz_paths = {
        path
        for path in paths
        if path.startswith("fuzz/fuzz_targets/") or "/fuzz/fuzz_targets/" in path
    }
    for path in sorted(observed_fuzz_paths - set(fuzz_paths)):
        failures.append(f"unregistered cargo-fuzz source: {path}")

    return ToolingJoinResult(
        bindings={
            code_id: tuple(sorted(values)) for code_id, values in sorted(bindings.items())
        },
        failures=tuple(sorted(set(failures))),
    )
