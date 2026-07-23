"""Deterministic assembly of Istanbul-attributed test trace documents."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .code_spec_common import CodeSourceSpan
from .coverage_trace import validate_trace_document
from .coverage_trace_attribution import attributed_code_ids
from .coverage_trace_istanbul import parse_istanbul_coverage


def build_istanbul_trace_document(
    *,
    root: Path,
    requested: Sequence[Mapping[str, Any]],
    observations: Mapping[str, Mapping[str, Any]],
    coverage_documents: Mapping[str, Mapping[str, Any]],
    sources: Mapping[str, bytes],
    spans: Mapping[str, CodeSourceSpan],
    source_commit: str,
    platform: str,
    input_digest: str,
    code_inventory_digest: str,
    mode: str,
) -> dict[str, Any]:
    """Join exact successful test observations with per-test Istanbul coverage."""

    requested_by_id = _index(requested, "trace requests")
    if set(observations) != set(requested_by_id):
        raise ValueError(
            "Istanbul observation set does not match requests: "
            f"missing={sorted(set(requested_by_id) - set(observations))}, "
            f"extra={sorted(set(observations) - set(requested_by_id))}"
        )
    if set(coverage_documents) != set(requested_by_id):
        raise ValueError(
            "Istanbul coverage set does not match requests: "
            f"missing={sorted(set(requested_by_id) - set(coverage_documents))}, "
            f"extra={sorted(set(coverage_documents) - set(requested_by_id))}"
        )
    records: list[dict[str, Any]] = []
    for discovery_id in sorted(requested_by_id):
        request = requested_by_id[discovery_id]
        observation = observations[discovery_id]
        if observation.get("exit_code") != 0:
            raise ValueError(f"{discovery_id} observation did not exit successfully")
        duration_ms = observation.get("duration_ms")
        if (
            not isinstance(duration_ms, int)
            or isinstance(duration_ms, bool)
            or duration_ms < 0
        ):
            raise ValueError(f"{discovery_id} observation duration is invalid")
        coverage = coverage_documents[discovery_id]
        regions = parse_istanbul_coverage(
            coverage,
            root=root,
            sources=sources,
        )
        records.append(
            {
                "discovery_id": discovery_id,
                "discovery_source_kind": _required_string(
                    request, "discovery_source_kind", discovery_id
                ),
                "path": _required_string(request, "path", discovery_id),
                "name": _required_string(request, "name", discovery_id),
                "command": _required_string(
                    observation, "command", discovery_id
                ),
                "command_authority": "verified_synthesized",
                "trace_status": "observed",
                "attribution_kind": "istanbul_source_coverage",
                "attempt_id": _required_string(
                    observation, "attempt_id", discovery_id
                ),
                "exit_code": 0,
                "duration_ms": duration_ms,
                "profile_digest": _canonical_digest(coverage),
                "covered_code_ids": list(attributed_code_ids(regions, spans)),
                "limitations": [
                    "Execution coverage ranks candidates and creates no semantic proof."
                ],
            }
        )
    document = {
        "schema_version": 1,
        "generator": "test-coverage-trace",
        "generator_version": "1",
        "source_commit": source_commit,
        "platform": platform,
        "scope": {
            "mode": mode,
            "requested_discovery_ids": sorted(requested_by_id),
            "source_kinds": sorted(
                {
                    _required_string(
                        request, "discovery_source_kind", discovery_id
                    )
                    for discovery_id, request in requested_by_id.items()
                }
            ),
        },
        "input_digest": input_digest,
        "code_inventory_digests": {
            "typescript_product": code_inventory_digest
        },
        "tests": records,
    }
    live_facts = {
        discovery_id: {
            "discovery_id": discovery_id,
            "discovery_source_kind": request["discovery_source_kind"],
            "path": request["path"],
            "name": request["name"],
        }
        for discovery_id, request in requested_by_id.items()
    }
    failures = validate_trace_document(
        document,
        live_facts=live_facts,
        allow_full_campaign=mode == "full_campaign",
    )
    if failures:
        raise ValueError("; ".join(failures))
    return document


def _index(
    records: Sequence[Mapping[str, Any]],
    label: str,
) -> dict[str, Mapping[str, Any]]:
    indexed: dict[str, Mapping[str, Any]] = {}
    for record in records:
        discovery_id = _required_string(record, "discovery_id", label)
        if discovery_id in indexed:
            raise ValueError(f"{label} duplicate {discovery_id}")
        indexed[discovery_id] = record
    return indexed


def _canonical_digest(document: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        document,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
