"""Rebuild a focused Rust trace from live nextest identity and raw LLVM evidence."""

from __future__ import annotations

import json
import os
import subprocess
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .code_spec_live import CodeSpecMetadata
from .coverage_trace_attribution import executed_repository_paths
from .coverage_trace_cli import (
    instrumented_nextest_list_environment,
    requested_tests_document,
    resolve_nextest_list_shard,
    rust_package_target_inventory,
    targeted_nextest_list_shards,
    trace_environment,
)
from .coverage_trace_direct import parse_llvm_cov_environment
from .coverage_trace_document import build_rust_trace_document
from .coverage_trace_evidence import (
    RawTraceEvidence,
    capture_raw_trace_evidence,
    validate_external_evidence,
)
from .coverage_trace_resume import reusable_completed_attempt_ids
from .coverage_trace_tools import export_attempt_coverage
from .focused_mapping_context import (
    FocusedMappingContext,
    resolved_command_features,
)
from .mapping_live_code_facts import (
    focused_rust_observed_source_projection,
    focused_rust_path_inventory,
)


@dataclass(frozen=True)
class AuthenticatedFocusedTrace:
    evidence: RawTraceEvidence
    source_originals: Mapping[str, bytes]
    rebuilt: Mapping[str, Any]


def authenticate_focused_trace(
    *,
    root: Path,
    trace: Mapping[str, Any],
    context: FocusedMappingContext,
    metadata: CodeSpecMetadata,
    trace_root: Path,
    cargo_target_dir: Path,
    llvm_tools_dir: Path,
    temporary_dir: Path,
    evidence: RawTraceEvidence | None = None,
) -> AuthenticatedFocusedTrace:
    """Require live runtime identity and reproduce every observed code ID."""

    root = root.resolve()
    evidence = evidence or capture_raw_trace_evidence(
        root=root,
        trace_root=trace_root,
        llvm_tools_dir=llvm_tools_dir,
    )
    provenance = trace.get("input_provenance")
    if not isinstance(provenance, Mapping):
        raise ValueError("focused trace lacks raw evidence provenance")
    if provenance.get("trace_root") != evidence.trace_root.relative_to(root).as_posix():
        raise ValueError("focused trace root differs from raw evidence")
    if provenance.get("raw_evidence_digest") != evidence.digest:
        raise ValueError("focused trace raw evidence digest drifted")
    if provenance.get("llvm_tool_digests") != evidence.tool_digests:
        raise ValueError("focused trace LLVM tool identity drifted")
    if provenance.get("llvm_tool_paths") != evidence.tool_paths:
        raise ValueError("focused trace LLVM tool path drifted")
    provenance_resolved = provenance.get("resolved")
    if not isinstance(provenance_resolved, list) or any(
        not isinstance(record, Mapping) for record in provenance_resolved
    ):
        raise ValueError("focused trace resolved provenance is malformed")
    feature_sets = {
        resolved_command_features(record)
        for record in provenance_resolved
    }
    if len(feature_sets) != 1:
        raise ValueError(
            "focused trace resolved commands use inconsistent Cargo features"
        )
    cargo_features = next(iter(feature_sets))

    environment = trace_environment(
        base_environment=os.environ,
        cargo_target_dir=cargo_target_dir,
        temporary_dir=temporary_dir,
    )
    live_facts = [
        {
            "discovery_id": fact.stable_id,
            "discovery_source_kind": fact.source_kind,
            "path": fact.path,
            "name": fact.name,
            "ignore_state": fact.ignore_state,
            "package": fact.package,
        }
        for fact in context.selected_live_facts
    ]
    metadata_result = _run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        root=root,
        environment=environment,
    )
    try:
        cargo_metadata = json.loads(metadata_result.stdout)
    except json.JSONDecodeError as error:
        raise ValueError("cargo metadata emitted invalid JSON") from error
    if not isinstance(cargo_metadata, Mapping):
        raise ValueError("cargo metadata root must be an object")
    llvm_cov_environment = parse_llvm_cov_environment(
        _run(
            ["cargo", "llvm-cov", "show-env"],
            root=root,
            environment=environment,
        ).stdout
    )
    list_environment = instrumented_nextest_list_environment(
        base_environment=environment,
        llvm_cov_environment=llvm_cov_environment,
        cargo_target_dir=cargo_target_dir,
    )
    package_targets = rust_package_target_inventory(cargo_metadata)
    for fact in live_facts:
        package = str(fact["package"])
        if package not in package_targets:
            raise ValueError(f"cargo metadata lacks scanner package {package}")
        fact.update(package_targets[package])
    resolved: list[dict[str, Any]] = []
    unsupported: list[dict[str, Any]] = []
    for shard in targeted_nextest_list_shards(
        live_facts,
        cargo_features=cargo_features,
        root=root,
    ):
        listed = _run(
            list(shard["command"]),
            root=root,
            environment=list_environment,
        )
        try:
            nextest_document = json.loads(listed.stdout)
        except json.JSONDecodeError as error:
            raise ValueError("cargo nextest list emitted invalid JSON") from error
        if not isinstance(nextest_document, Mapping):
            raise ValueError("cargo nextest list root must be an object")
        shard_resolved, shard_unsupported = resolve_nextest_list_shard(
            shard=shard,
            nextest_document=nextest_document,
            cargo_features=cargo_features,
        )
        resolved.extend(shard_resolved)
        unsupported.extend(shard_unsupported)
    resolved.sort(key=lambda row: str(row["discovery_id"]))
    if unsupported:
        raise ValueError("live nextest identity no longer resolves selected tests")
    if provenance.get("resolved") != resolved:
        raise ValueError("focused trace live nextest identity drifted")
    request_bytes = (
        json.dumps(requested_tests_document(resolved), indent=2, sort_keys=True)
        + "\n"
    ).encode()
    request_relative = (
        evidence.trace_root / "requested-tests.json"
    ).relative_to(root).as_posix()
    if evidence.repository_originals.get(request_relative) != request_bytes:
        raise ValueError("focused trace requested-tests evidence is not canonical")
    reusable_completed_attempt_ids(
        trace_root=evidence.trace_root,
        resolved=resolved,
        purge_incomplete=False,
        require_profile_digest=True,
    )
    exports = export_attempt_coverage(
        root=root,
        trace_root=evidence.trace_root,
        llvm_tools_dir=llvm_tools_dir.resolve(),
    )
    observed_paths = sorted(
        {
            path
            for discovery_id in exports
            for path in executed_repository_paths(exports[discovery_id], root=root)
        }
    )
    sources, projection_digest = focused_rust_observed_source_projection(
        root,
        observed_paths,
        metadata.mappings,
    )
    source_originals = dict(sources)
    inventory = focused_rust_path_inventory(
        root,
        sorted(sources),
        metadata.mappings,
    )
    for relative, expected in source_originals.items():
        if (root / relative).read_bytes() != expected:
            raise ValueError(
                f"focused covered source changed during reconstruction: {relative}"
            )
    rebuilt = build_rust_trace_document(
        root=root,
        trace_root=evidence.trace_root,
        requested=resolved,
        coverage_exports=exports,
        sources=sources,
        spans=inventory.spans,
        source_commit=str(trace.get("source_commit")),
        platform=str(trace.get("platform")),
        input_digest=str(trace.get("input_digest")),
        code_inventory_digests={"rust_observed_paths": projection_digest},
        mode="focused_shard",
        input_provenance=dict(provenance),
    )
    if dict(trace) != rebuilt:
        raise ValueError("focused trace differs from raw execution reconstruction")
    external_failures = validate_external_evidence(evidence)
    if external_failures:
        raise ValueError("; ".join(external_failures))
    return AuthenticatedFocusedTrace(
        evidence=evidence,
        source_originals=source_originals,
        rebuilt=rebuilt,
    )


def _run(
    command: Sequence[str],
    *,
    root: Path,
    environment: Mapping[str, str],
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        list(command),
        cwd=root,
        env=dict(environment),
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise ValueError(f"command failed ({completed.returncode}): {detail}")
    return completed
