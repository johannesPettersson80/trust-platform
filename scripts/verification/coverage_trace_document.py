"""Deterministic assembly of execution-attributed Rust trace documents."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any, Callable

from .code_spec_common import CodeSourceSpan
from .coverage_trace import validate_trace_document
from .coverage_trace_attribution import (
    attributed_code_ids,
    parse_llvm_export,
)
from .coverage_trace_resume import profile_digest


def build_rust_trace_document(
    *,
    root: Path,
    trace_root: Path,
    requested: Sequence[Mapping[str, Any]],
    coverage_exports: Mapping[str, Mapping[str, Any]],
    sources: Mapping[str, bytes],
    spans: Mapping[str, CodeSourceSpan],
    source_commit: str,
    platform: str,
    input_digest: str,
    code_inventory_digests: Mapping[str, str],
    mode: str,
    input_provenance: Mapping[str, Any] | None = None,
    coverage_export_observer: (
        Callable[[Mapping[str, Any]], None] | None
    ) = None,
    span_resolver: (
        Callable[[Sequence[str]], Mapping[str, CodeSourceSpan]] | None
    ) = None,
) -> dict[str, Any]:
    """Join exact completed attempts with LLVM exports and governed code spans."""

    requested_by_id: dict[str, Mapping[str, Any]] = {}
    for request in requested:
        discovery_id = _required_string(request, "discovery_id", "trace request")
        if discovery_id in requested_by_id:
            raise ValueError(f"trace requests duplicate {discovery_id}")
        requested_by_id[discovery_id] = request
    attempts = _load_attempts(trace_root)
    if set(attempts) != set(requested_by_id):
        raise ValueError(
            "trace attempt set does not match requests: "
            f"missing={sorted(set(requested_by_id) - set(attempts))}, "
            f"extra={sorted(set(attempts) - set(requested_by_id))}"
        )
    if set(coverage_exports) != set(requested_by_id):
        raise ValueError(
            "coverage export set does not match requests: "
            f"missing={sorted(set(requested_by_id) - set(coverage_exports))}, "
            f"extra={sorted(set(coverage_exports) - set(requested_by_id))}"
        )

    records: list[dict[str, Any]] = []
    live_spans = dict(spans)
    resolved_span_paths = {span.path for span in live_spans.values()}
    for discovery_id in sorted(requested_by_id):
        request = requested_by_id[discovery_id]
        attempt_dir, attempt = attempts[discovery_id]
        exit_code = attempt.get("exit_code")
        if (
            attempt.get("status") != "completed"
            or not isinstance(exit_code, int)
            or isinstance(exit_code, bool)
        ):
            raise ValueError(f"{discovery_id} attempt did not complete")
        if attempt.get("binary_id") != request.get("nextest_binary_id"):
            raise ValueError(f"{discovery_id} attempt binary identity drifted")
        if attempt.get("test_name") != request.get("nextest_test_name"):
            raise ValueError(f"{discovery_id} attempt test identity drifted")
        duration_ms = attempt.get("duration_ms")
        if (
            not isinstance(duration_ms, int)
            or isinstance(duration_ms, bool)
            or duration_ms < 0
        ):
            raise ValueError(f"{discovery_id} attempt duration is invalid")
        source_kind = _required_string(
            request, "discovery_source_kind", discovery_id
        )
        current_profile_digest = profile_digest(attempt_dir / "profiles")
        recorded_profile_digest = attempt.get("profile_digest")
        if recorded_profile_digest is not None:
            if recorded_profile_digest != current_profile_digest:
                raise ValueError(
                    f"{discovery_id} attempt profile digest drifted"
                )
        elif source_kind in {"structured_text_test", "conformance_case"}:
            raise ValueError(
                f"{discovery_id} direct attempt lacks profile digest"
            )
        record = {
            "discovery_id": discovery_id,
            "discovery_source_kind": source_kind,
            "path": _required_string(request, "path", discovery_id),
            "name": _required_string(request, "name", discovery_id),
            "command": _required_string(request, "command", discovery_id),
            "command_authority": _required_string(
                request, "command_authority", discovery_id
            ),
            "attempt_id": _required_string(attempt, "attempt_id", discovery_id),
            "exit_code": exit_code,
            "duration_ms": duration_ms,
            "profile_digest": current_profile_digest,
        }
        if exit_code == 0:
            coverage_export = coverage_exports[discovery_id]
            if coverage_export_observer is not None:
                coverage_export_observer(coverage_export)
            regions = parse_llvm_export(
                coverage_export,
                root=root,
                sources=sources,
            )
            region_paths = {region.path for region in regions}
            missing_span_paths = sorted(region_paths - resolved_span_paths)
            if missing_span_paths and span_resolver is not None:
                additional = span_resolver(missing_span_paths)
                overlap = sorted(set(live_spans) & set(additional))
                if overlap:
                    raise ValueError(
                        f"focused span resolver duplicates code IDs: {overlap}"
                    )
                invalid_paths = sorted(
                    {
                        span.path
                        for span in additional.values()
                        if span.path not in missing_span_paths
                    }
                )
                if invalid_paths:
                    raise ValueError(
                        "focused span resolver returned unrequested paths: "
                        f"{invalid_paths}"
                    )
                live_spans.update(additional)
                resolved_span_paths.update(missing_span_paths)
            record.update(
                {
                    "trace_status": "observed",
                    "attribution_kind": "llvm_source_coverage",
                    "covered_code_ids": list(
                        attributed_code_ids(regions, live_spans)
                    ),
                    "limitations": [
                        "Execution coverage ranks candidates and creates no "
                        "semantic proof."
                    ],
                }
            )
        else:
            record.update(
                {
                    "trace_status": "failed",
                    "attribution_kind": "unavailable",
                    "covered_code_ids": [],
                    "limitations": [
                        "Exact command exited nonzero; no semantic candidate "
                        "was generated."
                    ],
                }
            )
        records.append(record)
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
        "code_inventory_digests": dict(sorted(code_inventory_digests.items())),
        "tests": records,
    }
    if input_provenance is not None:
        document["input_provenance"] = dict(input_provenance)
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


def _load_attempts(
    trace_root: Path,
) -> dict[str, tuple[Path, Mapping[str, Any]]]:
    attempts: dict[str, tuple[Path, Mapping[str, Any]]] = {}
    attempts_root = trace_root / "attempts"
    if not attempts_root.is_dir():
        return attempts
    for path in sorted(attempts_root.glob("*/attempt.json")):
        try:
            document = json.loads(path.read_text())
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise ValueError(f"cannot load trace attempt {path}") from error
        if not isinstance(document, Mapping):
            raise ValueError(f"trace attempt {path} must be an object")
        discovery_id = _required_string(document, "discovery_id", path.as_posix())
        if discovery_id in attempts:
            raise ValueError(f"trace attempts duplicate {discovery_id}")
        attempts[discovery_id] = (path.parent, document)
    return attempts


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
