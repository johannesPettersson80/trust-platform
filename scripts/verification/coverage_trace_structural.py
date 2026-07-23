"""Deterministic non-execution trace shard for structural and ignored facts."""

from __future__ import annotations

import json
import platform
import re
import subprocess
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .coverage_trace import validate_trace_document
from .coverage_trace_inputs import trace_input_digest


STRUCTURAL_SOURCE_KINDS = frozenset(
    {"fuzz_target", "gate_script", "github_workflow_job"}
)


def structural_route_reason(fact: Mapping[str, Any]) -> str | None:
    if fact.get("ignore_state") == "ignored":
        return "declared_ignored"
    source_kind = fact.get("discovery_source_kind")
    if source_kind in STRUCTURAL_SOURCE_KINDS:
        return f"structural_{source_kind}"
    return None


def build_structural_trace_document(
    *,
    facts: Sequence[Mapping[str, Any]],
    scanner_input_digest: str,
    code_map: Mapping[str, Any],
    source_commit: str,
    platform_name: str,
    mode: str,
) -> dict[str, Any]:
    records: list[dict[str, Any]] = []
    seen: set[str] = set()
    for fact in sorted(facts, key=lambda row: str(row.get("discovery_id", ""))):
        discovery_id = _required_string(fact, "discovery_id", "structural fact")
        if discovery_id in seen:
            raise ValueError(f"structural trace duplicates {discovery_id}")
        seen.add(discovery_id)
        reason = structural_route_reason(fact)
        if reason is None:
            raise ValueError(
                f"{discovery_id} is executable and cannot enter structural trace"
            )
        records.append(
            {
                "discovery_id": discovery_id,
                "discovery_source_kind": _required_string(
                    fact, "discovery_source_kind", discovery_id
                ),
                "path": _required_string(fact, "path", discovery_id),
                "name": _required_string(fact, "name", discovery_id),
                "command": _required_string(
                    fact, "command_hint", discovery_id
                ),
                "command_authority": "unavailable",
                "trace_status": "skipped",
                "attribution_kind": "unavailable",
                "covered_code_ids": [],
                "limitations": [reason],
            }
        )
    document = {
        "schema_version": 1,
        "generator": "test-coverage-trace",
        "generator_version": "1",
        "source_commit": source_commit,
        "platform": platform_name,
        "scope": {
            "mode": mode,
            "requested_discovery_ids": sorted(seen),
            "source_kinds": sorted(
                {
                    str(record["discovery_source_kind"])
                    for record in records
                }
            ),
        },
        "input_digest": trace_input_digest(
            scanner_input_digest,
            list(facts),
            code_map,
        ),
        "code_inventory_digests": {},
        "tests": records,
    }
    live_facts = {
        str(fact["discovery_id"]): fact
        for fact in facts
    }
    failures = validate_trace_document(
        document,
        live_facts=live_facts,
        allow_full_campaign=mode == "full_campaign",
    )
    if failures:
        raise ValueError("; ".join(failures))
    return document


def git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    head = completed.stdout.strip()
    if completed.returncode != 0 or re.fullmatch(r"[0-9a-f]{40}", head) is None:
        raise ValueError("structural trace cannot resolve source commit")
    return head


def current_platform() -> str:
    return f"{platform.system().lower()}-{platform.machine().lower()}"


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
