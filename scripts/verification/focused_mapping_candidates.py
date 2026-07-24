"""No-full-test-scan production generation for focused mapping batches."""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import tempfile
import tomllib
from collections.abc import Mapping, Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .code_spec_live import CodeSpecMetadata
from .code_spec_risk import index_risk_acknowledgements
from .focused_mapping_context import (
    AuthenticatedAuthorityProjection,
    authenticate_projected_code_spec_authority,
    build_focused_mapping_context,
    canonical_input_file,
    validate_focused_rust_trace,
)
from .coverage_trace_evidence import (
    capture_raw_trace_evidence,
    validate_external_evidence,
)
from .focused_trace_authentication import authenticate_focused_trace
from .mapping_assertions import invoke_assertion_inventories
from .mapping_candidates import analyze_mapping_candidates
from .focused_mapping_queue import merge_focused_queue
from .mapping_live_code_facts import validated_live_code_facts
from .mapping_proposals import (
    build_mapping_proposals,
    validate_mapping_quarantine,
)
from .mapping_review import (
    suppressed_proposal_digests,
    validate_mapping_decisions,
)
from .mapping_toml import render_decisions_toml, render_proposals_toml
from .mapping_transaction import apply_mapping_transaction
from .test_catalog_json_schema import validate_json_schema_instance


def generate_focused_mapping_candidates(
    *,
    root: Path,
    trace_path: Path,
    candidates_path: Path,
    proposals_path: Path,
    decisions_path: Path,
    rust_discovery_ids: Sequence[str],
    authority_plan_path: Path | None,
    trace_root: Path,
    cargo_target_dir: Path,
    llvm_tools_dir: Path,
    temporary_dir: Path,
    timestamp: str | None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Generate quarantine planes from selected facts and a frozen census."""

    root = root.resolve()
    generated = timestamp or datetime.now(timezone.utc).replace(
        microsecond=0
    ).isoformat().replace("+00:00", "Z")
    requested = sorted(rust_discovery_ids)
    if not requested or len(requested) != len(set(requested)):
        raise ValueError("focused mapping discovery IDs must be non-empty and unique")
    trace_path = canonical_input_file(
        root,
        trace_path,
        "focused Rust trace",
        required_parent="target/gate-artifacts",
    )
    candidates_path = _governed_output_path(
        root,
        candidates_path,
        "focused candidate report",
        required_parent="target/gate-artifacts",
    )
    proposals_path = _canonical_output_path(
        root,
        proposals_path,
        "verification/test-catalog-proposals.toml",
        "mapping proposal queue",
    )
    decisions_path = _canonical_output_path(
        root,
        decisions_path,
        "verification/test-catalog-mapping-decisions.toml",
        "mapping decision ledger",
    )
    if len({trace_path, candidates_path, proposals_path, decisions_path}) != 4:
        raise ValueError("focused mapping input and output targets must be distinct")
    trace_bytes = trace_path.read_bytes()
    candidate_original = _existing_bytes(candidates_path)
    proposal_original = proposals_path.read_bytes()
    decision_original = decisions_path.read_bytes()
    metadata_preconditions = _metadata_preconditions(root)
    raw_evidence = capture_raw_trace_evidence(
        root=root,
        trace_root=trace_root,
        llvm_tools_dir=llvm_tools_dir,
    )
    candidate_outputs = {
        candidates_path.relative_to(root).as_posix(),
        proposals_path.relative_to(root).as_posix(),
        decisions_path.relative_to(root).as_posix(),
    }
    _validate_candidate_output_separation(
        outputs=candidate_outputs,
        authenticated_inputs=set(raw_evidence.repository_originals),
        label="raw trace evidence",
    )
    trace = _json_document(trace_bytes, "focused Rust trace")
    selected_preconditions = _selected_input_preconditions(
        root=root,
        requested=requested,
        trace=trace,
        metadata_preconditions=metadata_preconditions,
    )
    _merge_preconditions(metadata_preconditions, selected_preconditions)
    _merge_preconditions(
        metadata_preconditions, raw_evidence.repository_originals
    )
    _validate_candidate_output_separation(
        outputs=candidate_outputs,
        authenticated_inputs={
            trace_path.relative_to(root).as_posix(),
            *metadata_preconditions,
        },
        label="focused inputs",
    )
    trace_schema = _json_document(
        metadata_preconditions[
            "verification/schemas/test-coverage-trace.schema.json"
        ],
        "test coverage trace schema",
    )
    failures = validate_json_schema_instance(trace, trace_schema)
    if failures:
        raise ValueError("; ".join(failures))
    current_commit = _git_head(root)
    if trace.get("source_commit") != current_commit:
        raise ValueError(
            "trace source commit is stale: "
            f"trace={trace.get('source_commit')} current={current_commit}"
        )
    trace_records = trace.get("tests")
    if not isinstance(trace_records, list) or any(
        not isinstance(record, Mapping) for record in trace_records
    ):
        raise ValueError("focused mapping trace tests must be an object array")
    traced_ids = [
        str(record.get("discovery_id"))
        for record in trace_records
        if isinstance(record.get("discovery_id"), str)
    ]
    if len(traced_ids) != len(trace_records) or sorted(traced_ids) != requested:
        raise ValueError(
            "focused mapping trace must contain exactly the selected discovery IDs"
        )
    if len(traced_ids) != len(set(traced_ids)):
        raise ValueError("focused mapping trace duplicates a discovery ID")

    metadata = _metadata_from_preconditions(metadata_preconditions)
    snapshot_directory = tempfile.TemporaryDirectory(
        prefix="trust-focused-mapping-"
    )
    snapshot_root = Path(snapshot_directory.name)
    _materialize_source_snapshot(snapshot_root, selected_preconditions)
    context = build_focused_mapping_context(
        root=root,
        discovery_ids=requested,
        tests=metadata.tests,
        denominator_document=tomllib.loads(
            metadata_preconditions[
                "verification/test-catalog-denominator.toml"
            ].decode()
        ),
        denominator_schema_document=json.loads(
            metadata_preconditions[
                "verification/schemas/test-catalog-denominator.schema.json"
            ]
        ),
        ignored_document=tomllib.loads(
            metadata_preconditions["verification/ignored-tests.toml"].decode()
        ),
        source_root=snapshot_root,
    )
    validate_focused_rust_trace(
        root=root,
        trace=trace,
        context=context,
        metadata=metadata,
    )
    authenticated_trace = authenticate_focused_trace(
        root=root,
        trace=trace,
        context=context,
        metadata=metadata,
        trace_root=trace_root,
        cargo_target_dir=cargo_target_dir,
        llvm_tools_dir=llvm_tools_dir,
        temporary_dir=temporary_dir,
        evidence=raw_evidence,
    )
    _merge_preconditions(
        metadata_preconditions,
        authenticated_trace.source_originals,
    )
    projection: AuthenticatedAuthorityProjection | None = None
    if authority_plan_path is not None:
        projection = authenticate_projected_code_spec_authority(
            root=root,
            metadata=metadata,
            plan_path=authority_plan_path,
        )
        _validate_authority_candidate_scope(
            projection=projection,
            requested=requested,
            trace_records=trace_records,
            canonical_mappings=metadata.mappings,
        )
        _validate_candidate_output_separation(
            outputs=candidate_outputs,
            authenticated_inputs=(
                set(projection.application.expected_originals)
                | set(projection.application.replacements)
            ),
            label="authority artifacts",
        )
        metadata = projection.metadata
    trace_digest = _canonical_digest(trace)
    analysis = analyze_mapping_candidates(
        trace_records=trace_records,
        code_mappings=metadata.mappings,
        invariants=metadata.invariants,
        live_code_facts=validated_live_code_facts(
            snapshot_root,
            trace,
            metadata.mappings,
            workspace_root=root,
        ),
    )
    candidate_document = _build_candidate_document(
        source_commit=str(trace["source_commit"]),
        trace_digest=trace_digest,
        analysis=analysis,
    )
    candidate_schema = _json_document(
        metadata_preconditions[
            "verification/schemas/mapping-candidates.schema.json"
        ],
        "mapping candidate schema",
    )
    failures = validate_json_schema_instance(candidate_document, candidate_schema)
    if failures:
        raise ValueError("; ".join(failures))

    catalog_discovery_ids = {
        str(test["discovery_id"])
        for test in metadata.tests.values()
        if test.get("subject_kind") == "generated_test"
        and isinstance(test.get("discovery_id"), str)
    }
    selected = _select_quarantine_candidates(
        candidate_document["candidates"],
        catalog_discovery_ids=catalog_discovery_ids,
        denominator_reviews=context.reviews_by_id,
    )
    selected_ids = [str(candidate["discovery_id"]) for candidate in selected]
    assertion_inventories = invoke_assertion_inventories(
        snapshot_root,
        [context.selected_facts[discovery_id] for discovery_id in selected_ids],
        environment=os.environ,
        workspace_root=root,
    )
    trace_by_id = {
        str(record["discovery_id"]): record for record in trace_records
    }
    proposals = build_mapping_proposals(
        root=snapshot_root,
        candidates=selected,
        trace_records=trace_by_id,
        facts=context.selected_facts,
        invariants=metadata.invariants,
        assertion_inventories=assertion_inventories,
    )
    decision_document, proposals = _load_and_filter_mapping_decisions(
        root=root,
        decisions_path=decisions_path,
        source_commit=str(trace["source_commit"]),
        proposals=proposals,
        invariants=metadata.invariants,
        decision_bytes=decision_original,
        decision_schema_bytes=metadata_preconditions[
            "verification/schemas/test-catalog-mapping-decisions.schema.json"
        ],
        proposal_schema_bytes=metadata_preconditions[
            "verification/schemas/test-catalog-proposals.schema.json"
        ],
    )
    proposal_schema = _json_document(
        metadata_preconditions[
            "verification/schemas/test-catalog-proposals.schema.json"
        ],
        "mapping proposal schema",
    )
    existing_proposal_document = tomllib.loads(proposal_original.decode())
    existing_failures = validate_json_schema_instance(
        existing_proposal_document, proposal_schema
    )
    if existing_failures:
        raise ValueError(
            "existing proposal queue is invalid: "
            + "; ".join(existing_failures)
        )
    candidate_document, proposals = merge_focused_queue(
        existing_queue=existing_proposal_document,
        focused_candidate_document=candidate_document,
        focused_proposals=proposals,
        selected_discovery_ids=requested,
        code_mappings=metadata.mappings,
    )
    merged_candidate_failures = validate_json_schema_instance(
        candidate_document, candidate_schema
    )
    if merged_candidate_failures:
        raise ValueError("; ".join(merged_candidate_failures))
    proposal_document = _build_proposal_document(
        source_commit=str(trace["source_commit"]),
        trace_digest=trace_digest,
        generated=generated,
        candidate_document=candidate_document,
        proposals=proposals,
    )
    quarantine_failures = validate_mapping_quarantine(
        root=snapshot_root,
        proposals=proposals,
        facts=context.selected_facts,
        tests=metadata.tests,
        denominator_reviews=context.reviews_by_id,
        invariants=metadata.invariants,
    )
    quarantine_failures.extend(
        validate_json_schema_instance(proposal_document, proposal_schema)
    )
    if quarantine_failures:
        raise ValueError("; ".join(sorted(set(quarantine_failures))))
    snapshot_directory.cleanup()
    replacements = {
        _relative_to_root(root, candidates_path): (
            json.dumps(candidate_document, indent=2, sort_keys=True) + "\n"
        ).encode(),
        _relative_to_root(root, proposals_path): render_proposals_toml(
            proposal_document
        ).encode(),
        _relative_to_root(root, decisions_path): render_decisions_toml(
            decision_document
        ).encode(),
    }
    expected_originals: dict[str, bytes | None] = {}
    _merge_preconditions(expected_originals, metadata_preconditions)
    _merge_preconditions(
        expected_originals,
        {trace_path.relative_to(root).as_posix(): trace_bytes},
    )
    _merge_preconditions(
        expected_originals,
        {
            candidates_path.relative_to(root).as_posix(): candidate_original,
            proposals_path.relative_to(root).as_posix(): proposal_original,
            decisions_path.relative_to(root).as_posix(): decision_original,
        },
    )
    if projection is not None:
        _merge_preconditions(
            expected_originals, projection.expected_originals
        )
    apply_mapping_transaction(
        root=root,
        replacements=replacements,
        expected_originals=expected_originals,
        validate=lambda: [
            *_validate_generated_files(
                candidates_path=candidates_path,
                proposals_path=proposals_path,
                decisions_path=decisions_path,
                candidate_document=candidate_document,
                proposal_document=proposal_document,
                decision_document=decision_document,
            ),
            *validate_external_evidence(raw_evidence),
        ],
    )
    return candidate_document, proposal_document


def _select_quarantine_candidates(
    candidates: Sequence[Mapping[str, Any]],
    *,
    catalog_discovery_ids: set[str],
    denominator_reviews: Mapping[str, Mapping[str, Any]],
) -> list[Mapping[str, Any]]:
    selected = []
    for candidate in candidates:
        discovery_id = candidate.get("discovery_id")
        review = denominator_reviews.get(str(discovery_id))
        if (
            isinstance(discovery_id, str)
            and discovery_id not in catalog_discovery_ids
            and review is not None
            and review.get("disposition") == "reviewed_nonmapping"
            and not candidate.get("unmapped_code_ids")
        ):
            selected.append(candidate)
    return sorted(selected, key=lambda row: str(row["discovery_id"]))


def _load_and_filter_mapping_decisions(
    *,
    root: Path,
    decisions_path: Path,
    source_commit: str,
    proposals: list[dict[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    decision_bytes: bytes | None,
    decision_schema_bytes: bytes,
    proposal_schema_bytes: bytes,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    if decision_bytes is None:
        return _initial_decision_document(source_commit), proposals
    decision_document = tomllib.loads(decision_bytes.decode())
    decision_schema = _json_document(
        decision_schema_bytes,
        "mapping decision schema",
    )
    decision_failures = validate_json_schema_instance(
        decision_document, decision_schema
    )
    snapshots = decision_document.get("proposal_snapshots", [])
    decisions = decision_document.get("decisions", [])
    if (
        not isinstance(snapshots, list)
        or any(not isinstance(snapshot, Mapping) for snapshot in snapshots)
        or not isinstance(decisions, list)
        or any(not isinstance(decision, Mapping) for decision in decisions)
    ):
        raise ValueError(
            "mapping decisions must contain snapshot and decision tables"
        )
    snapshot_by_id = {
        str(snapshot["id"]): snapshot
        for snapshot in snapshots
        if isinstance(snapshot.get("id"), str)
    }
    if len(snapshot_by_id) != len(snapshots):
        raise ValueError("mapping decision snapshots duplicate or lack IDs")
    proposal_schema = _json_document(
        proposal_schema_bytes,
        "mapping proposal schema",
    )
    snapshot_schema = {
        "$ref": "#/$defs/proposal",
        "$defs": proposal_schema.get("$defs", {}),
    }
    for snapshot in snapshots:
        decision_failures.extend(
            validate_json_schema_instance(snapshot, snapshot_schema)
        )
    decision_failures.extend(
        validate_mapping_decisions(
            decisions=decisions,
            proposals=snapshot_by_id,
            invariants=invariants,
            candidate_generator_id="test-catalog-proposals",
        )
    )
    if decision_failures:
        raise ValueError("; ".join(sorted(set(decision_failures))))
    suppressed = suppressed_proposal_digests(decisions)
    return decision_document, [
        proposal
        for proposal in proposals
        if proposal["proposal_digest"] not in suppressed
    ]


def _build_candidate_document(
    *,
    source_commit: str,
    trace_digest: str,
    analysis: Mapping[str, Any],
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "generator": "test-mapping-candidates",
        "generator_version": "1",
        "source_commit": source_commit,
        "trace_digest": trace_digest,
        "candidates": list(analysis.get("candidates", [])),
        "demands": list(analysis.get("demands", [])),
        "spec_demand_observations": list(
            analysis.get("spec_demand_observations", [])
        ),
        "demand_ranking": list(analysis.get("demand_ranking", [])),
        "structural_demand_ranking": list(
            analysis.get("structural_demand_ranking", [])
        ),
        "unmapped_code_demand_ranking": list(
            analysis.get("unmapped_code_demand_ranking", [])
        ),
    }


def _build_proposal_document(
    *,
    source_commit: str,
    trace_digest: str,
    generated: str,
    candidate_document: Mapping[str, Any],
    proposals: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "generator": "test-catalog-proposals",
        "generator_version": "1",
        "source_commit": source_commit,
        "trace_digest": trace_digest,
        "candidate_report_digest": _canonical_digest(candidate_document),
        "last_generated": generated,
        "demands": list(candidate_document["demands"]),
        "demand_ranking": list(candidate_document["demand_ranking"]),
        "structural_demand_ranking": list(
            candidate_document["structural_demand_ranking"]
        ),
        "unmapped_code_demand_ranking": list(
            candidate_document["unmapped_code_demand_ranking"]
        ),
        "proposals": list(proposals),
    }


def _initial_decision_document(source_commit: str) -> dict[str, Any]:
    if re.fullmatch(r"[0-9a-f]{40}", source_commit) is None:
        raise ValueError("mapping decision initialization requires source commit")
    return {
        "schema_version": 1,
        "generator": "test-catalog-mapping-review",
        "generator_version": "1",
        "source_commit": source_commit,
        "proposal_snapshots": [],
        "decisions": [],
    }


def _validate_generated_files(
    *,
    candidates_path: Path,
    proposals_path: Path,
    decisions_path: Path,
    candidate_document: Mapping[str, Any],
    proposal_document: Mapping[str, Any],
    decision_document: Mapping[str, Any],
) -> list[str]:
    failures: list[str] = []
    try:
        if _load_json(candidates_path) != dict(candidate_document):
            failures.append("focused candidate report differs after atomic replacement")
        if tomllib.loads(proposals_path.read_text()) != dict(proposal_document):
            failures.append("focused proposal queue differs after atomic replacement")
        if tomllib.loads(decisions_path.read_text()) != dict(decision_document):
            failures.append("focused decision ledger differs after atomic replacement")
    except (OSError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        failures.append(
            f"focused mapping candidate replacement cannot be read: {error}"
        )
    return failures


def _canonical_digest(document: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        document, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _load_json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} root must be an object")
    return document


def _relative_to_root(root: Path, path: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(root).as_posix()
    except ValueError as error:
        raise ValueError(
            f"focused mapping candidate output escapes repository: {path}"
        ) from error


def _canonical_output_path(
    root: Path,
    path: Path,
    expected_relative: str,
    label: str,
) -> Path:
    expected = root / expected_relative
    candidate = path if path.is_absolute() else root / path
    if candidate != expected:
        raise ValueError(f"{label} must equal {expected_relative}")
    return canonical_input_file(root, candidate, label)


def _governed_output_path(
    root: Path,
    path: Path,
    label: str,
    *,
    required_parent: str,
) -> Path:
    candidate = path if path.is_absolute() else root / path
    try:
        relative = candidate.relative_to(root)
    except ValueError as error:
        raise ValueError(f"{label} escapes repository") from error
    if not relative.is_relative_to(required_parent):
        raise ValueError(f"{label} must stay under {required_parent}")
    cursor = root
    for part in relative.parts:
        cursor = cursor / part
        if cursor.is_symlink():
            raise ValueError(f"{label} must not traverse symlinks")
        if not cursor.exists():
            break
    existing_parent = candidate.parent
    while not existing_parent.exists():
        existing_parent = existing_parent.parent
    try:
        existing_parent.resolve(strict=True).relative_to(root)
    except (OSError, ValueError) as error:
        raise ValueError(f"{label} parent escapes repository") from error
    if candidate.exists() and not candidate.is_file():
        raise ValueError(f"{label} existing target is not a regular file")
    return candidate


def _metadata_preconditions(root: Path) -> dict[str, bytes]:
    core_relatives = [
        "verification/code-spec-map.toml",
        "verification/code-spec-debt-baseline.toml",
        "verification/spec-sources.toml",
        "verification/spec-gaps.toml",
        "verification/test-catalog.toml",
        "verification/test-catalog-denominator.toml",
        "verification/ignored-tests.toml",
        "verification/code-spec-risk-acknowledgements.toml",
        "verification/matrix.toml",
        "verification/schemas/test-coverage-trace.schema.json",
        "verification/schemas/mapping-candidates.schema.json",
        "verification/schemas/test-catalog-proposals.schema.json",
        "verification/schemas/test-catalog-mapping-decisions.schema.json",
        "verification/schemas/test-catalog-denominator.schema.json",
    ]
    result = {}
    for relative in core_relatives:
        path = canonical_input_file(
            root,
            root / relative,
            f"focused mapping metadata {relative}",
        )
        result[relative] = path.read_bytes()
    invariant_root = root / "verification/invariants"
    if invariant_root.is_symlink() or not invariant_root.is_dir():
        raise ValueError("invariant root must be a regular repository directory")
    invariant_relatives = [
        path.relative_to(root).as_posix()
        for path in sorted(invariant_root.rglob("*.toml"))
    ]
    spec_sources_document = tomllib.loads(
        result["verification/spec-sources.toml"].decode()
    )
    spec_sources = spec_sources_document.get("spec_sources")
    if not isinstance(spec_sources, list) or any(
        not isinstance(source, Mapping) for source in spec_sources
    ):
        raise ValueError("specification source inventory must contain object rows")
    tracked_spec_relatives = [
        str(source["path"])
        for source in spec_sources
        if source.get("locator_kind") == "tracked_file"
        and isinstance(source.get("path"), str)
    ]
    for relative in sorted(set(invariant_relatives + tracked_spec_relatives)):
        path = canonical_input_file(
            root,
            root / relative,
            f"focused mapping metadata {relative}",
        )
        result[relative] = path.read_bytes()
    return result


def _metadata_from_preconditions(
    originals: Mapping[str, bytes],
) -> CodeSpecMetadata:
    def document(relative: str) -> dict[str, Any]:
        try:
            value = tomllib.loads(originals[relative].decode())
        except (KeyError, UnicodeError, tomllib.TOMLDecodeError) as error:
            raise ValueError(
                f"focused metadata snapshot is invalid: {relative}"
            ) from error
        if not isinstance(value, dict):
            raise ValueError(f"focused metadata snapshot is not a table: {relative}")
        return value

    code_map = document("verification/code-spec-map.toml")
    debt = document("verification/code-spec-debt-baseline.toml")
    spec_sources_document = document("verification/spec-sources.toml")
    spec_gaps_document = document("verification/spec-gaps.toml")
    catalog = document("verification/test-catalog.toml")
    risk_document = document(
        "verification/code-spec-risk-acknowledgements.toml"
    )
    risk_rows, failures = index_risk_acknowledgements(risk_document)
    if failures:
        raise ValueError("; ".join(failures))
    invariants = {}
    for relative, content in sorted(originals.items()):
        if not (
            relative.startswith("verification/invariants/")
            and relative.endswith(".toml")
        ):
            continue
        record = tomllib.loads(content.decode())
        invariant_id = record.get("id")
        if not isinstance(invariant_id, str) or not invariant_id:
            raise ValueError(f"focused invariant snapshot lacks ID: {relative}")
        if invariant_id in invariants:
            raise ValueError(f"focused invariant snapshot duplicates {invariant_id}")
        invariants[invariant_id] = record
    return CodeSpecMetadata(
        code_map=code_map,
        debt_baseline=debt,
        mappings=_index_snapshot_rows(
            code_map, "mappings", "code_id", "code-spec map"
        ),
        spec_sources=_index_snapshot_rows(
            spec_sources_document, "spec_sources", "id", "spec source"
        ),
        spec_gaps=_index_snapshot_rows(
            spec_gaps_document, "spec_gaps", "id", "spec gap"
        ),
        invariants=dict(sorted(invariants.items())),
        tests=_index_snapshot_rows(catalog, "tests", "id", "test catalog"),
        risk_ack_document=risk_document,
        risk_acknowledgements=risk_rows,
    )


def _index_snapshot_rows(
    document: Mapping[str, Any],
    field: str,
    id_field: str,
    label: str,
) -> dict[str, Mapping[str, Any]]:
    rows = document.get(field)
    if not isinstance(rows, list):
        raise ValueError(f"focused {label} snapshot lacks {field}")
    indexed = {}
    for row in rows:
        if not isinstance(row, Mapping):
            raise ValueError(f"focused {label} snapshot row is malformed")
        row_id = row.get(id_field)
        if not isinstance(row_id, str) or not row_id or row_id in indexed:
            raise ValueError(f"focused {label} snapshot identity is malformed")
        indexed[row_id] = row
    return dict(sorted(indexed.items()))


def _selected_input_preconditions(
    *,
    root: Path,
    requested: Sequence[str],
    trace: Mapping[str, Any],
    metadata_preconditions: Mapping[str, bytes],
) -> dict[str, bytes]:
    """Capture selected test, assertion, Cargo, and covered source bytes."""

    denominator = tomllib.loads(
        metadata_preconditions[
            "verification/test-catalog-denominator.toml"
        ].decode()
    )
    reviews = denominator.get("reviews")
    if not isinstance(reviews, list):
        raise ValueError("focused denominator reviews are malformed")
    paths = {
        str(review["path"])
        for review in reviews
        if isinstance(review, Mapping)
        and review.get("discovery_id") in requested
        and isinstance(review.get("path"), str)
    }
    if len(paths) != len(requested):
        raise ValueError("focused selected source paths are incomplete")
    code_map = tomllib.loads(
        metadata_preconditions["verification/code-spec-map.toml"].decode()
    )
    mapping_rows = code_map.get("mappings")
    if not isinstance(mapping_rows, list):
        raise ValueError("focused code map mappings are malformed")
    mapping_paths = {
        str(row["code_id"]): str(row["path"])
        for row in mapping_rows
        if isinstance(row, Mapping)
        and isinstance(row.get("code_id"), str)
        and isinstance(row.get("path"), str)
    }
    records = trace.get("tests")
    if isinstance(records, list):
        for record in records:
            if not isinstance(record, Mapping):
                continue
            covered = record.get("covered_code_ids")
            if not isinstance(covered, list):
                continue
            for code_id in covered:
                if isinstance(code_id, str) and code_id in mapping_paths:
                    paths.add(mapping_paths[code_id])
    cargo_manifests = {
        Path(*Path(path).parts[:2], "Cargo.toml").as_posix()
        for path in paths
        if len(Path(path).parts) >= 2 and Path(path).parts[0] == "crates"
    }
    result = {}
    for relative in sorted(paths | cargo_manifests):
        source = canonical_input_file(
            root,
            root / relative,
            f"focused selected input {relative}",
        )
        result[relative] = source.read_bytes()
    return result


def _materialize_source_snapshot(
    snapshot_root: Path,
    originals: Mapping[str, bytes],
) -> None:
    """Materialize captured source bytes without consulting the live checkout."""

    for relative, content in sorted(originals.items()):
        target = snapshot_root / relative
        try:
            target.resolve().relative_to(snapshot_root.resolve())
        except ValueError as error:
            raise ValueError(
                f"focused source snapshot path escapes root: {relative}"
            ) from error
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content)


def _merge_preconditions(
    target: dict[str, bytes | None],
    additions: Mapping[str, bytes | None],
) -> None:
    for path, content in additions.items():
        if path in target and target[path] != content:
            raise ValueError(f"focused candidate precondition collides at {path}")
        target[path] = content


def _validate_candidate_output_separation(
    *,
    outputs: set[str],
    authenticated_inputs: set[str],
    label: str,
) -> None:
    collisions = sorted(outputs & authenticated_inputs)
    if collisions:
        raise ValueError(
            "focused candidate outputs collide with authenticated "
            f"{label}: {collisions}"
        )


def _validate_authority_candidate_scope(
    *,
    projection: AuthenticatedAuthorityProjection,
    requested: Sequence[str],
    trace_records: Sequence[Mapping[str, Any]],
    canonical_mappings: Mapping[str, Mapping[str, Any]],
) -> None:
    covered_by_discovery = {
        str(record["discovery_id"]): {
            str(code_id)
            for code_id in record.get("covered_code_ids", [])
            if isinstance(code_id, str)
            and canonical_mappings.get(code_id, {}).get("disposition")
            == "spec_gap"
        }
        for record in trace_records
    }
    manifest_rows = projection.application.manifest.get("rows")
    if not isinstance(manifest_rows, list) or not manifest_rows:
        raise ValueError("authority candidate projection requires rows")
    authority_discoveries: set[str] = set()
    for row in manifest_rows:
        if not isinstance(row, Mapping):
            raise ValueError("authority candidate projection row is malformed")
        code_id = row.get("code_id")
        discoveries = row.get("discovery_ids")
        if not isinstance(code_id, str) or not isinstance(discoveries, list):
            raise ValueError("authority candidate projection scope is malformed")
        for discovery_id in discoveries:
            if (
                not isinstance(discovery_id, str)
                or discovery_id not in covered_by_discovery
                or code_id not in covered_by_discovery[discovery_id]
            ):
                raise ValueError(
                    "authority candidate projection escapes selected "
                    "trace-covered gap facts"
                )
            authority_discoveries.add(discovery_id)
    if authority_discoveries != set(requested):
        raise ValueError(
            "authority candidate discoveries differ from exact focused trace scope"
        )


def _existing_bytes(path: Path) -> bytes | None:
    return path.read_bytes() if path.is_file() else None


def _json_document(content: bytes, label: str) -> dict[str, Any]:
    document = json.loads(content)
    if not isinstance(document, dict):
        raise ValueError(f"{label} root must be an object")
    return document


def _git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()
