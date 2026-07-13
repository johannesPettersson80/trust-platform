"""Mechanical specification-source and public-prose association analysis."""

from __future__ import annotations

import hashlib
import re
import subprocess
from itertools import combinations
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .metadata_validator.constants import AREAS
from .metadata_validator.integrity import OPEN_GAP_RESOLUTIONS
from .spec_source_scope import OBVIOUS_SPEC_TOPICS


SCOPE = {
    "document_basis": "tracked_spec_and_public_document_scanner_facts",
    "source_mapping_basis": "exact_registered_source_path_only",
    "required_topic_basis": "all_committed_required_spec_records",
    "public_prose_basis": "readme_and_tracked_docs_public_with_recursive_tracked_includes",
    "public_claim_binding_basis": "exact_registered_path_and_claim_text_occurrence",
    "public_prose_denominator_exhaustive": True,
    "source_classification_complete": False,
    "semantic_claim_review_complete": False,
    "conflict_review_complete": False,
    "checklist_row_staleness_complete": False,
    "removed_behavior_reference_review_complete": False,
    "lexical_candidates_create_mappings": False,
}


def analyze_spec_sources(
    root: Path,
    *,
    scan: object,
    spec_sources: Mapping[str, Mapping[str, Any]],
    required_specs: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    obvious_topics: Sequence[object] = OBVIOUS_SPEC_TOPICS,
) -> dict[str, Any]:
    """Join scanner facts to reviewed metadata without inferring semantics."""

    root = root.resolve()
    documents = sorted(_items(scan, "documents"), key=lambda row: _value(row, "path"))
    public_blocks = sorted(
        _items(scan, "public_blocks"),
        key=lambda row: (
            _value(row, "path"),
            _value(row, "line_start", 0),
            _value(row, "block_id"),
        ),
    )
    diagnostics = sorted(
        _items(scan, "diagnostics"),
        key=lambda row: (
            _value(row, "path"),
            _value(row, "line", 0),
            _value(row, "kind"),
        ),
    )

    document_by_path: dict[str, list[object]] = {}
    for document in documents:
        document_by_path.setdefault(str(_value(document, "path")), []).append(document)

    source_bindings: list[dict[str, Any]] = []
    findings: list[dict[str, Any]] = []
    source_ids_by_document: dict[str, list[str]] = {}
    source_reviews_due = 0
    for source_id in sorted(spec_sources):
        source = spec_sources[source_id]
        locator_kind = source.get("locator_kind")
        if locator_kind is None and isinstance(source.get("path"), str):
            locator_kind = "tracked_file"
        path = source.get("path")
        matches = document_by_path.get(path, []) if isinstance(path, str) else []
        if locator_kind == "external_reference":
            document_id = None
            binding_state = "external_reference"
        elif len(matches) == 1:
            document_id = str(_value(matches[0], "document_id"))
            binding_state = "bound"
            source_ids_by_document.setdefault(document_id, []).append(source_id)
        else:
            document_id = None
            binding_state = "missing" if not matches else "ambiguous"
            findings.append(
                _finding(
                    "registered_source_" + binding_state,
                    "error",
                    path if isinstance(path, str) else "verification/spec-sources.toml",
                    source_id,
                    f"registered source resolves to {len(matches)} scanner documents",
                )
            )
        last_reviewed = source.get("last_reviewed")
        last_change = _git_last_change_date(root, path) if isinstance(path, str) else None
        review_due = False
        if isinstance(last_reviewed, str) and last_change is not None:
            review_due = last_change > last_reviewed
            if review_due:
                source_reviews_due += 1
                findings.append(
                    _finding(
                        "source_review_due",
                        "warning",
                        str(path),
                        source_id,
                        f"source changed on {last_change} after review date {last_reviewed}",
                    )
                )
        conflicts = source.get("conflicts_with", [])
        if not isinstance(conflicts, list):
            conflicts = []
        for conflict_id in conflicts:
            if conflict_id not in spec_sources:
                findings.append(
                    _finding(
                        "source_conflict_ref_missing",
                        "error",
                        str(path or "verification/spec-sources.toml"),
                        source_id,
                        f"conflicts_with references unknown source {conflict_id}",
                    )
                )
            else:
                reverse = spec_sources[conflict_id].get("conflicts_with", [])
                if not isinstance(reverse, list) or source_id not in reverse:
                    findings.append(
                        _finding(
                            "source_conflict_asymmetric",
                            "warning",
                            str(path or "verification/spec-sources.toml"),
                            source_id,
                            f"conflict with {conflict_id} is not declared in both directions",
                        )
                    )
        expected_local_path = source.get("expected_local_path")
        availability = (
            "external_bytes_unbound"
            if locator_kind == "external_reference"
            else "not_applicable"
        )
        source_bindings.append(
            {
                "source_id": source_id,
                "locator_kind": locator_kind,
                "path": path,
                "external_ref": source.get("external_ref"),
                "expected_local_path": expected_local_path,
                "document_id": document_id,
                "area": source.get("area"),
                "authority": source.get("authority"),
                "visibility": source.get("visibility"),
                "source_status": source.get("source_status"),
                "oracle_eligible": source.get("oracle_eligible"),
                "last_reviewed": last_reviewed,
                "last_change_date": last_change,
                "review_due": review_due,
                "binding_state": binding_state,
                "version": source.get("version"),
                "publication_date": source.get("publication_date"),
                "absence_blocks_proof": source.get("absence_blocks_proof"),
                "availability": availability,
                "conflicts_with": sorted(str(item) for item in conflicts),
            }
        )

    for left_id, right_id in combinations(sorted(spec_sources), 2):
        left = spec_sources[left_id]
        right = spec_sources[right_id]
        if left.get("authority") != right.get("authority"):
            continue
        left_covers = {item for item in left.get("covers", []) if isinstance(item, str)}
        right_covers = {item for item in right.get("covers", []) if isinstance(item, str)}
        shared = sorted(left_covers & right_covers)
        if shared:
            findings.append(
                _finding(
                    "source_potential_overlap",
                    "warning",
                    "verification/spec-sources.toml",
                    f"{left_id}:{right_id}",
                    "equal-authority sources share reviewed covers: " + ", ".join(shared),
                )
            )

    document_rows: list[dict[str, Any]] = []
    for document in documents:
        document_id = str(_value(document, "document_id"))
        registered = sorted(source_ids_by_document.get(document_id, []))
        references = _items(document, "local_references")
        for reference_index, reference in enumerate(references):
            target = _value(reference, "target_path")
            fragment = _value(reference, "fragment")
            if _value(reference, "exists") is False:
                code = "local_reference_missing"
                message = f"local reference target does not exist: {target!r}"
            elif _value(reference, "tracked") is False:
                code = "local_reference_untracked"
                message = f"local reference target is not tracked: {target!r}"
            elif fragment is not None and _value(reference, "fragment_exists") is False:
                code = "local_reference_fragment_missing"
                message = f"local reference fragment does not exist: {target!r}#{fragment}"
            else:
                continue
            findings.append(
                _finding(
                    code,
                    "warning",
                    str(_value(document, "path")),
                    f"{document_id}:{_value(reference, 'source_line', 0)}:"
                    f"{_value(reference, 'kind', 'reference')}:{reference_index}:{target}",
                    message,
                )
            )
        public_entry_paths = sorted(
            str(item) for item in _items(document, "public_entry_paths")
        )
        source_kind = _value(document, "source_kind")
        format_value = _value(document, "format", "markdown")
        in_spec_scope = bool(_value(document, "in_spec_document_scope", True))
        primary_public = bool(_value(document, "primary_public_surface", source_kind == "public_surface"))
        document_rows.append(
            {
                "document_id": document_id,
                "path": str(_value(document, "path")),
                "format": str(format_value),
                "content_sha256": str(_value(document, "content_sha256")),
                "title": _value(document, "title"),
                "in_spec_document_scope": in_spec_scope,
                "primary_public_surface": primary_public,
                "public_entry_paths": public_entry_paths,
                "local_reference_count": len(references),
                "registered_source_ids": registered,
                "review_state": "registered_metadata" if registered else "unreviewed_candidate",
            }
        )

    required_rows: list[dict[str, Any]] = []
    for topic_id in sorted(required_specs):
        topic = required_specs[topic_id]
        status = topic.get("status")
        source_ref = topic.get("source_ref") if isinstance(topic.get("source_ref"), str) else None
        gap_ref = topic.get("spec_gap_ref") if isinstance(topic.get("spec_gap_ref"), str) else None
        if status == "mapped" and source_ref in spec_sources:
            mapping_state = "mapped"
        elif (
            status == "spec_gap"
            and gap_ref in spec_gaps
            and spec_gaps[gap_ref].get("resolution_status") in OPEN_GAP_RESOLUTIONS
        ):
            mapping_state = "gap_open"
        else:
            mapping_state = "broken"
            if status == "mapped":
                code = "required_topic_missing_source"
                detail = f"mapped required topic references unknown source {source_ref!r}"
            else:
                code = "required_topic_missing_gap"
                detail = f"spec-gap required topic references missing or non-open gap {gap_ref!r}"
            findings.append(
                _finding(
                    code,
                    "warning",
                    "verification/spec-matrix.toml",
                    topic_id,
                    detail,
                )
            )
        required_rows.append(
            {
                "topic_id": topic_id,
                "area": topic.get("area"),
                "tag": topic.get("tag"),
                "title": topic.get("title"),
                "owner": topic.get("owner"),
                "status": status,
                "source_ref": source_ref,
                "spec_gap_ref": gap_ref,
                "mapping_state": mapping_state,
            }
        )

    obvious_rows, obvious_findings = _obvious_spec_topic_rows(
        obvious_topics,
        spec_sources=spec_sources,
        spec_gaps=spec_gaps,
    )
    findings.extend(obvious_findings)

    claim_sources = {
        source_id: source
        for source_id, source in spec_sources.items()
        if source.get("authority") == "public_claim"
    }
    claim_ids_by_block: dict[str, list[str]] = {}
    public_claim_rows: list[dict[str, Any]] = []
    for claim_id in sorted(claim_sources):
        claim = claim_sources[claim_id]
        path = claim.get("path")
        surface_ref = claim.get("surface_ref")
        surface_path = (
            surface_ref.partition("#")[0]
            if isinstance(surface_ref, str)
            else None
        )
        text = claim.get("claim_text")
        normalized_claim = _normalize_prose(text) if isinstance(text, str) else ""
        matches: list[str] = []
        if isinstance(path, str) and normalized_claim:
            for block in public_blocks:
                entry_paths = {str(item) for item in _items(block, "public_entry_paths")}
                entry_paths.update(str(item) for item in _items(block, "surface_paths"))
                if _value(block, "path") != path and path not in entry_paths:
                    continue
                if _value(block, "path") != surface_path and surface_path not in entry_paths:
                    continue
                block_text = _block_text(root, block)
                if _contains_exact_text(block_text, normalized_claim):
                    block_id = str(_value(block, "block_id"))
                    matches.append(block_id)
                    claim_ids_by_block.setdefault(block_id, []).append(claim_id)
        matches.sort()
        binding_state = "bound" if len(matches) == 1 else "missing" if not matches else "ambiguous"
        if binding_state != "bound":
            findings.append(
                _finding(
                    "public_claim_" + binding_state,
                    "error",
                    str(path or "verification/spec-sources.toml"),
                    claim_id,
                    f"registered public claim resolves to {len(matches)} exact prose blocks",
                )
            )
        public_claim_rows.append(
            {
                "claim_id": claim_id,
                "path": path,
                "surface_ref": surface_ref,
                "surface_path": surface_path,
                "claim_text_sha256": hashlib.sha256(normalized_claim.encode()).hexdigest(),
                "block_ids": matches,
                "binding_state": binding_state,
            }
        )

    public_block_rows: list[dict[str, Any]] = []
    surface_paths: set[str] = set()
    for block in public_blocks:
        block_id = str(_value(block, "block_id"))
        surfaces = sorted(str(item) for item in _items(block, "public_entry_paths"))
        if not surfaces:
            surfaces = sorted(str(item) for item in _items(block, "surface_paths"))
        if not surfaces:
            surfaces = [str(_value(block, "path"))]
        surface_paths.update(surfaces)
        registered = sorted(claim_ids_by_block.get(block_id, []))
        heading = [str(item) for item in _items(block, "heading_path")]
        public_block_rows.append(
            {
                "block_id": block_id,
                "document_id": str(_value(block, "document_id")),
                "path": str(_value(block, "path")),
                "line_start": int(_value(block, "line_start")),
                "line_end": int(_value(block, "line_end")),
                "heading_path": heading,
                "block_kind": str(_value(block, "block_kind", "paragraph")),
                "source_text_sha256": str(_value(block, "text_sha256")),
                "visible_text_sha256": str(
                    _value(block, "visible_text_sha256", _value(block, "text_sha256"))
                ),
                "public_entry_paths": surfaces,
                "registered_claim_ids": registered,
                "review_state": "registered_claim" if registered else "unreviewed_candidate",
            }
        )

    for diagnostic in diagnostics:
        severity = str(_value(diagnostic, "severity"))
        kind = str(_value(diagnostic, "kind"))
        line = _value(diagnostic, "line", None)
        record_id = f"{kind}:{_value(diagnostic, 'path')}:{line if line is not None else 0}"
        findings.append(
            _finding(
                "scanner_" + kind,
                "error" if severity == "error" else "warning",
                str(_value(diagnostic, "path")),
                record_id,
                str(_value(diagnostic, "message")),
            )
        )

    findings.extend(_duplicate_decision_heading_findings(documents))
    findings.sort(key=lambda row: (row["severity"], row["code"], row["path"], row["record_id"]))
    summary = {
        "documents_total": len(document_rows),
        "registered_sources": len(source_bindings),
        "bound_sources": sum(row["binding_state"] == "bound" for row in source_bindings),
        "external_sources": sum(
            row["binding_state"] == "external_reference" for row in source_bindings
        ),
        "unbound_sources": sum(
            row["binding_state"] not in {"bound", "external_reference"}
            for row in source_bindings
        ),
        "unreviewed_documents": sum(
            row["review_state"] == "unreviewed_candidate" for row in document_rows
        ),
        "required_topics_total": len(required_rows),
        "required_topics_mapped": sum(row["mapping_state"] == "mapped" for row in required_rows),
        "required_topics_gap_open": sum(row["mapping_state"] == "gap_open" for row in required_rows),
        "required_topics_broken": sum(row["mapping_state"] == "broken" for row in required_rows),
        "obvious_spec_topics_total": len(obvious_rows),
        "obvious_spec_source_present": sum(
            row["reviewed_posture"] == "source_present" for row in obvious_rows
        ),
        "obvious_spec_gap": sum(
            row["reviewed_posture"] == "gap_open" for row in obvious_rows
        ),
        "obvious_spec_partial": sum(
            row["reviewed_posture"]
            not in {"source_present", "gap_open", "unrepresented"}
            for row in obvious_rows
        ),
        "obvious_spec_unrepresented": sum(
            row["reviewed_posture"] == "unrepresented" for row in obvious_rows
        ),
        "obvious_spec_reference_broken": sum(
            row["reference_health"] == "broken" for row in obvious_rows
        ),
        "public_surfaces": len(surface_paths),
        "public_prose_blocks": len(public_block_rows),
        "registered_public_claims": len(public_claim_rows),
        "bound_public_claims": sum(row["binding_state"] == "bound" for row in public_claim_rows),
        "unbound_public_claims": sum(row["binding_state"] != "bound" for row in public_claim_rows),
        "unreviewed_public_blocks": sum(
            row["review_state"] == "unreviewed_candidate" for row in public_block_rows
        ),
        "scanner_diagnostics": len(diagnostics),
        "source_reviews_due": source_reviews_due,
        "blocking_findings": sum(row["severity"] == "error" for row in findings),
        "warning_findings": sum(row["severity"] == "warning" for row in findings),
    }
    return {
        "scope": dict(SCOPE),
        "documents": document_rows,
        "source_bindings": source_bindings,
        "required_topics": required_rows,
        "obvious_missing_specs": obvious_rows,
        "public_prose_blocks": public_block_rows,
        "registered_public_claims": public_claim_rows,
        "findings": findings,
        "summary": summary,
    }


def _obvious_spec_topic_rows(
    topics: Sequence[object],
    *,
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    rows: list[dict[str, Any]] = []
    findings: list[dict[str, str]] = []
    for topic in topics:
        topic_id = str(_value(topic, "topic_id"))
        areas = [str(item) for item in _items(topic, "areas")]
        eligible = [str(item) for item in _items(topic, "eligible_source_ids")]
        nonoracle = [str(item) for item in _items(topic, "nonoracle_source_ids")]
        gaps = [str(item) for item in _items(topic, "open_spec_gap_ids")]
        public = [str(item) for item in _items(topic, "public_claim_context_ids")]
        problems: list[tuple[str, str]] = []
        if not areas or areas != sorted(set(areas)) or any(area not in AREAS for area in areas):
            problems.append(("obvious_topic_area_invalid", f"areas are not canonical known area IDs: {areas!r}"))
        for source_id in eligible:
            source = spec_sources.get(source_id)
            if (
                source is None
                or source.get("source_status") != "active"
                or source.get("oracle_eligible") is not True
                or source.get("authority") == "public_claim"
            ):
                problems.append(
                    (
                        "obvious_topic_eligible_source_invalid",
                        f"eligible source is absent, inactive, non-eligible, or public: {source_id}",
                    )
                )
        for source_id in nonoracle:
            source = spec_sources.get(source_id)
            if (
                source is None
                or source.get("oracle_eligible") is not False
                or source.get("authority") == "public_claim"
            ):
                problems.append(
                    (
                        "obvious_topic_nonoracle_source_invalid",
                        f"non-oracle context is absent, eligible, or public: {source_id}",
                    )
                )
        for source_id in public:
            source = spec_sources.get(source_id)
            if source is None or source.get("authority") != "public_claim":
                problems.append(
                    (
                        "obvious_topic_public_context_invalid",
                        f"public-claim context is absent or not a public claim: {source_id}",
                    )
                )
        for gap_id in gaps:
            gap = spec_gaps.get(gap_id)
            if (
                gap is None
                or gap.get("status") != "spec_gap"
                or gap.get("resolution_status") not in OPEN_GAP_RESOLUTIONS
            ):
                problems.append(
                    (
                        "obvious_topic_gap_invalid",
                        f"gap is absent or not open spec-gap metadata: {gap_id}",
                    )
                )
        for index, (code, message) in enumerate(problems):
            findings.append(
                _finding(
                    code,
                    "error",
                    "scripts/verification/spec_source_scope.py",
                    f"{topic_id}:{index}",
                    message,
                )
            )
        rows.append(
            {
                "topic_id": topic_id,
                "board_topic": str(_value(topic, "board_topic")),
                "areas": areas,
                "reviewed_posture": str(_value(topic, "reviewed_posture")),
                "eligible_source_ids": eligible,
                "nonoracle_source_ids": nonoracle,
                "open_spec_gap_ids": gaps,
                "public_claim_context_ids": public,
                "reference_health": "healthy" if not problems else "broken",
            }
        )
    return rows, findings


def _items(value: object, field: str) -> tuple[Any, ...]:
    result = _value(value, field, ())
    if isinstance(result, Sequence) and not isinstance(result, (str, bytes, bytearray)):
        return tuple(result)
    return ()


def _value(value: object, field: str, default: Any = None) -> Any:
    if isinstance(value, Mapping):
        return value.get(field, default)
    return getattr(value, field, default)


def _normalize_prose(value: str) -> str:
    return re.sub(r"\s+", " ", value).strip()


def _block_text(root: Path, block: object) -> str:
    explicit = _value(block, "visible_text")
    if not isinstance(explicit, str):
        explicit = _value(block, "text")
    if isinstance(explicit, str):
        return _normalize_prose(explicit)
    path = _value(block, "path")
    start = _value(block, "line_start")
    end = _value(block, "line_end")
    if not isinstance(path, str) or not isinstance(start, int) or not isinstance(end, int):
        return ""
    try:
        lines = (root / path).read_text().splitlines()
    except OSError:
        return ""
    if start < 1 or end < start or end > len(lines):
        return ""
    return _normalize_prose("\n".join(lines[start - 1 : end]))


def _contains_exact_text(block_text: str, claim_text: str) -> bool:
    if not block_text or not claim_text:
        return False
    return claim_text in block_text


def _git_last_change_date(root: Path, path: str) -> str | None:
    result = subprocess.run(
        ["git", "-C", str(root), "log", "-1", "--format=%cs", "--", path],
        check=False,
        capture_output=True,
        text=True,
    )
    value = result.stdout.strip()
    return value if result.returncode == 0 and re.fullmatch(r"\d{4}-\d{2}-\d{2}", value) else None


def _duplicate_decision_heading_findings(
    documents: Sequence[object],
) -> list[dict[str, str]]:
    decision_paths = {
        "docs/IEC_DECISIONS.md",
        "docs/IEC_DEVIATIONS.md",
        "docs/PLCOPEN_DECISIONS.md",
        "docs/PLCOPEN_DEVIATIONS.md",
    }
    headings: dict[str, list[tuple[str, int]]] = {}
    for document in documents:
        path = str(_value(document, "path"))
        if path not in decision_paths:
            continue
        for heading in _items(document, "headings"):
            if _value(heading, "level") != 2:
                continue
            value = _normalize_prose(str(_value(heading, "visible_text")))
            headings.setdefault(value, []).append((path, int(_value(heading, "line"))))
    findings: list[dict[str, str]] = []
    for heading, occurrences in sorted(headings.items()):
        if len(occurrences) < 2:
            continue
        paths = ", ".join(f"{path}:{line}" for path, line in occurrences)
        findings.append(
            _finding(
                "duplicate_decision_heading",
                "warning",
                occurrences[0][0],
                hashlib.sha256(heading.encode()).hexdigest()[:16],
                f"exact decision/deviation heading is duplicated at {paths}",
            )
        )
    return findings


def _finding(code: str, severity: str, path: str, record_id: str, message: str) -> dict[str, str]:
    return {
        "code": code,
        "severity": severity,
        "path": path,
        "record_id": record_id,
        "message": message,
    }
