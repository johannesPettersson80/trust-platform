"""Fail-closed payload, schema, and Markdown contract for source audits."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from datetime import datetime
from typing import Any

from .metadata_validator.constants import AREAS
from .spec_source_analysis import SCOPE
from .spec_source_models import BLOCK_KINDS
from .spec_source_reviews import (
    AUTHORITY_LEVELS,
    CHECKLIST_STALENESS,
    CLAIM_DISPOSITIONS,
    CLAIM_RATIONALES,
    CLASSIFICATION_BASES,
    CONFLICT_DISPOSITIONS,
    FRESHNESS_STATES,
    REMOVED_BEHAVIOR_DISPOSITIONS,
    VISIBILITIES,
)
from .spec_source_scope import OBVIOUS_SPEC_TOPICS, REVIEWED_POSTURES
from .spec_source_report import (
    BOUNDARIES,
    GENERATOR,
    GENERATOR_VERSION,
    LIMITATIONS,
    render_markdown,
)
from .test_catalog_validation import check_supported_schema_keywords, is_safe_relative_path


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
HEX_DIGEST_RE = re.compile(r"^[0-9a-f]{64}$")
TOP_FIELDS = {
    "schema_version",
    "generator",
    "generator_version",
    "report_status",
    "input_digest",
    "command",
    "commit",
    "timestamp",
    "platform",
    "input_paths",
    "output_paths",
    "scope",
    "boundaries",
    "documents",
    "source_bindings",
    "required_topics",
    "obvious_missing_specs",
    "public_prose_blocks",
    "registered_public_claims",
    "findings",
    "summary",
    "limitations",
}
OUTPUT_FIELDS = {"json", "markdown"}
DOCUMENT_FIELDS = {
    "document_id",
    "path",
    "format",
    "content_sha256",
    "title",
    "in_spec_document_scope",
    "primary_public_surface",
    "public_entry_paths",
    "local_reference_count",
    "registered_source_ids",
    "review_state",
    "review",
}
DOCUMENT_REVIEW_FIELDS = {
    "areas",
    "authority_levels",
    "owners",
    "freshness",
    "visibility",
    "oracle_usable",
    "classification_basis",
    "conflict_disposition",
    "checklist_staleness",
    "removed_behavior_disposition",
    "last_reviewed",
}
SOURCE_FIELDS = {
    "source_id",
    "locator_kind",
    "path",
    "external_ref",
    "expected_local_path",
    "document_id",
    "area",
    "authority",
    "visibility",
    "source_status",
    "oracle_eligible",
    "last_reviewed",
    "last_change_date",
    "review_due",
    "binding_state",
    "version",
    "publication_date",
    "absence_blocks_proof",
    "availability",
    "conflicts_with",
}
TOPIC_FIELDS = {
    "topic_id",
    "area",
    "tag",
    "title",
    "owner",
    "status",
    "source_ref",
    "spec_gap_ref",
    "mapping_state",
}
OBVIOUS_FIELDS = {
    "topic_id",
    "board_topic",
    "areas",
    "reviewed_posture",
    "eligible_source_ids",
    "nonoracle_source_ids",
    "open_spec_gap_ids",
    "public_claim_context_ids",
    "reference_health",
}
BLOCK_FIELDS = {
    "block_id",
    "document_id",
    "path",
    "line_start",
    "line_end",
    "heading_path",
    "block_kind",
    "source_text_sha256",
    "visible_text_sha256",
    "public_entry_paths",
    "registered_claim_ids",
    "review_state",
    "claim_review",
}
CLAIM_REVIEW_FIELDS = {
    "disposition",
    "invariant_ids",
    "oracle_refs",
    "rationale_code",
    "last_reviewed",
}
CLAIM_FIELDS = {
    "claim_id",
    "path",
    "surface_ref",
    "surface_path",
    "claim_text_sha256",
    "block_ids",
    "binding_state",
}
FINDING_FIELDS = {"code", "severity", "path", "record_id", "message"}
SUMMARY_FIELDS = {
    "documents_total",
    "registered_sources",
    "bound_sources",
    "external_sources",
    "unbound_sources",
    "unreviewed_documents",
    "classified_documents",
    "required_topics_total",
    "required_topics_mapped",
    "required_topics_gap_open",
    "required_topics_broken",
    "obvious_spec_topics_total",
    "obvious_spec_source_present",
    "obvious_spec_gap",
    "obvious_spec_partial",
    "obvious_spec_unrepresented",
    "obvious_spec_reference_broken",
    "public_surfaces",
    "public_prose_blocks",
    "registered_public_claims",
    "bound_public_claims",
    "unbound_public_claims",
    "unreviewed_public_blocks",
    "structurally_nonclaim_blocks",
    "public_claim_blocks",
    "claims_without_invariant_oracle",
    "mapped_public_claim_blocks",
    "scanner_diagnostics",
    "source_reviews_due",
    "blocking_findings",
    "warning_findings",
}


def validate_report_payload(
    payload: Mapping[str, Any],
    *,
    expected_analysis: Mapping[str, Any] | None = None,
) -> list[str]:
    failures: list[str] = []
    _fields(payload, TOP_FIELDS, "report", failures)
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if payload.get(field) != expected:
            failures.append(f"{field} must equal {expected!r}")
    commit = payload.get("commit")
    if not isinstance(commit, str) or not COMMIT_RE.fullmatch(commit):
        failures.append("commit must identify a clean full Git SHA")
    digest = payload.get("input_digest")
    if not isinstance(digest, str) or not DIGEST_RE.fullmatch(digest):
        failures.append("input_digest must be sha256:<64 lowercase hex>")
    if not _timestamp(payload.get("timestamp")):
        failures.append("timestamp must be ISO-8601 with a timezone")
    if not isinstance(payload.get("platform"), str) or not payload.get("platform"):
        failures.append("platform must be a non-empty string")

    inputs = payload.get("input_paths")
    if not _string_array(inputs, allow_empty=False) or inputs != sorted(set(inputs)):
        failures.append("input_paths must be a sorted unique non-empty string array")
    elif any(not is_safe_relative_path(path) for path in inputs):
        failures.append("input_paths must be normalized workspace-relative paths")
    outputs = payload.get("output_paths")
    if not isinstance(outputs, Mapping):
        failures.append("output_paths must be an object")
    else:
        _fields(outputs, OUTPUT_FIELDS, "output_paths", failures)
        output_values = [outputs.get(field) for field in sorted(OUTPUT_FIELDS)]
        if any(not isinstance(value, str) or not is_safe_relative_path(value) for value in output_values):
            failures.append("output paths must be normalized and workspace-relative")
        if len({_text(value) for value in output_values}) != len(output_values):
            failures.append("output paths must be distinct")
        if isinstance(inputs, list) and any(value in inputs for value in output_values):
            failures.append("output paths must not collide with bound inputs")
    _validate_command(payload, failures)
    if payload.get("scope") != SCOPE:
        failures.append("scope does not match the specification-source audit contract")
    if payload.get("boundaries") != BOUNDARIES:
        failures.append("boundaries do not match the report-only source audit contract")
    if payload.get("limitations") != list(LIMITATIONS):
        failures.append("limitations do not match the source audit contract")

    documents = _rows(payload, "documents", DOCUMENT_FIELDS, failures)
    sources = _rows(payload, "source_bindings", SOURCE_FIELDS, failures)
    topics = _rows(payload, "required_topics", TOPIC_FIELDS, failures)
    obvious = _rows(payload, "obvious_missing_specs", OBVIOUS_FIELDS, failures)
    blocks = _rows(payload, "public_prose_blocks", BLOCK_FIELDS, failures)
    claims = _rows(payload, "registered_public_claims", CLAIM_FIELDS, failures)
    findings = _rows(payload, "findings", FINDING_FIELDS, failures)
    _validate_documents(documents, failures)
    _validate_sources(sources, documents, failures)
    _validate_topics(topics, failures)
    _validate_obvious_topics(obvious, failures)
    _validate_blocks(blocks, documents, failures)
    _validate_claims(claims, blocks, failures)
    _validate_findings(findings, failures)
    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        failures.append("summary must be an object")
        summary_value: object = summary
    else:
        _fields(summary, SUMMARY_FIELDS, "summary", failures)
        expected_summary = _summary(
            documents, sources, topics, obvious, blocks, claims, findings, summary
        )
        if dict(summary) != expected_summary:
            failures.append("summary does not match source-audit rows")
        summary_value = dict(summary)

    if expected_analysis is not None:
        actual = {
            "scope": payload.get("scope"),
            "documents": documents,
            "source_bindings": sources,
            "required_topics": topics,
            "obvious_missing_specs": obvious,
            "public_prose_blocks": blocks,
            "registered_public_claims": claims,
            "findings": findings,
            "summary": summary_value,
        }
        if actual != dict(expected_analysis):
            failures.append("report rows do not match current specification-source analysis")
    return sorted(set(failures))


def validate_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(dict(schema), "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("specification-source schema root must be a closed object")
    if set(schema.get("required", [])) != TOP_FIELDS:
        failures.append("specification-source schema root required fields drift")
    properties = schema.get("properties", {})
    if not isinstance(properties, Mapping) or set(properties) != TOP_FIELDS:
        failures.append("specification-source schema root properties drift")
        properties = {}
    for field, expected in (
        ("schema_version", 1),
        ("generator", GENERATOR),
        ("generator_version", GENERATOR_VERSION),
        ("report_status", "complete"),
    ):
        if _property(properties, field).get("const") != expected:
            failures.append(f"specification-source schema const for {field} drifts")
    if _property(properties, "commit").get("pattern") != "^[0-9a-f]{40}$":
        failures.append("specification-source schema clean-commit pattern drifts")
    definitions = schema.get("$defs", {})
    if not isinstance(definitions, Mapping):
        failures.append("specification-source schema definitions must be an object")
        definitions = {}
    expected_defs = {
        "output_paths": OUTPUT_FIELDS,
        "scope": set(SCOPE),
        "boundaries": set(BOUNDARIES),
        "document": DOCUMENT_FIELDS,
        "document_review": DOCUMENT_REVIEW_FIELDS,
        "source_binding": SOURCE_FIELDS,
        "required_topic": TOPIC_FIELDS,
        "obvious_spec_topic": OBVIOUS_FIELDS,
        "public_block": BLOCK_FIELDS,
        "claim_review": CLAIM_REVIEW_FIELDS,
        "public_claim": CLAIM_FIELDS,
        "finding": FINDING_FIELDS,
        "summary": SUMMARY_FIELDS,
    }
    for name, fields in expected_defs.items():
        definition = definitions.get(name, {})
        if not isinstance(definition, Mapping):
            failures.append(f"specification-source schema {name} must be an object")
            definition = {}
        if definition.get("type") != "object" or definition.get("additionalProperties") is not False:
            failures.append(f"specification-source schema {name} must be a closed object")
        if set(definition.get("required", [])) != fields:
            failures.append(f"specification-source schema {name} required fields drift")
        if set(definition.get("properties", {})) != fields:
            failures.append(f"specification-source schema {name} properties drift")
    scope_properties = _definition_properties(definitions, "scope")
    for field, expected in SCOPE.items():
        if _property(scope_properties, field).get("const") != expected:
            failures.append(f"specification-source schema scope const for {field} drifts")
    boundary_properties = _definition_properties(definitions, "boundaries")
    for field, expected in BOUNDARIES.items():
        if _property(boundary_properties, field).get("const") != expected:
            failures.append(f"specification-source schema boundary const for {field} drifts")
    enum_expectations = {
        ("document", "review_state"): ["reviewed_classification", "unreviewed_candidate"],
        ("document_review", "freshness"): list(FRESHNESS_STATES),
        ("document_review", "visibility"): list(VISIBILITIES),
        ("document_review", "classification_basis"): list(CLASSIFICATION_BASES),
        ("document_review", "conflict_disposition"): list(CONFLICT_DISPOSITIONS),
        ("document_review", "checklist_staleness"): list(CHECKLIST_STALENESS),
        ("document_review", "removed_behavior_disposition"): list(
            REMOVED_BEHAVIOR_DISPOSITIONS
        ),
        ("source_binding", "binding_state"): ["bound", "missing", "ambiguous", "external_reference"],
        ("source_binding", "locator_kind"): ["tracked_file", "external_reference"],
        ("source_binding", "availability"): ["not_applicable", "external_bytes_unbound"],
        ("required_topic", "mapping_state"): ["mapped", "gap_open", "broken"],
        ("obvious_spec_topic", "reference_health"): ["healthy", "broken"],
        ("public_block", "review_state"): ["reviewed_disposition", "unreviewed_candidate"],
        ("claim_review", "disposition"): list(CLAIM_DISPOSITIONS),
        ("claim_review", "rationale_code"): list(CLAIM_RATIONALES),
        ("public_claim", "binding_state"): ["bound", "missing", "ambiguous"],
        ("finding", "severity"): ["error", "warning"],
    }
    for (definition, field), expected in enum_expectations.items():
        values = _property(_definition_properties(definitions, definition), field).get("enum")
        if values != expected:
            failures.append(f"specification-source schema {definition}.{field} enum drifts")
    obvious_properties = _definition_properties(definitions, "obvious_spec_topic")
    expected_topic_ids = [topic.topic_id for topic in OBVIOUS_SPEC_TOPICS]
    if _property(obvious_properties, "topic_id").get("enum") != expected_topic_ids:
        failures.append("specification-source schema obvious-topic ID enum drifts")
    if _property(obvious_properties, "reviewed_posture").get("enum") != list(
        REVIEWED_POSTURES
    ):
        failures.append("specification-source schema reviewed-posture enum drifts")
    area_items = _property(obvious_properties, "areas").get("items", {})
    area_enum = area_items.get("enum") if isinstance(area_items, Mapping) else None
    if area_enum != sorted(AREAS):
        failures.append("specification-source schema obvious-topic area enum drifts")
    document_review_properties = _definition_properties(definitions, "document_review")
    review_area_items = _property(document_review_properties, "areas").get("items", {})
    review_area_enum = (
        review_area_items.get("enum") if isinstance(review_area_items, Mapping) else None
    )
    if review_area_enum != sorted(AREAS):
        failures.append("specification-source schema document-review area enum drifts")
    authority_items = _property(document_review_properties, "authority_levels").get(
        "items", {}
    )
    authority_enum = (
        authority_items.get("enum") if isinstance(authority_items, Mapping) else None
    )
    if authority_enum != list(AUTHORITY_LEVELS):
        failures.append("specification-source schema document-review authority enum drifts")
    block_properties = _definition_properties(definitions, "public_block")
    if _property(block_properties, "block_kind").get("enum") != list(BLOCK_KINDS):
        failures.append("specification-source schema public-block kind enum drifts")
    _closed_objects(schema, "$", failures)
    return sorted(set(failures))


def validate_markdown_binding(
    payload: Mapping[str, Any],
    json_bytes: bytes,
    markdown: str,
) -> list[str]:
    failures: list[str] = []
    canonical = (json.dumps(dict(payload), indent=2, sort_keys=True) + "\n").encode()
    if json_bytes != canonical:
        failures.append("specification-source JSON is not canonical")
    try:
        expected = render_markdown(payload, json_digest=hashlib.sha256(json_bytes).hexdigest())
    except (KeyError, TypeError, ValueError) as exc:
        failures.append(f"specification-source Markdown cannot be reconstructed: {exc}")
    else:
        if markdown != expected:
            failures.append("specification-source Markdown does not exactly match JSON payload")
    return sorted(set(failures))


def _validate_command(payload: Mapping[str, Any], failures: list[str]) -> None:
    outputs = payload.get("output_paths")
    timestamp = payload.get("timestamp")
    if not isinstance(outputs, Mapping) or not isinstance(timestamp, str):
        return
    expected = [
        "python3",
        "scripts/report_spec_source_audit.py",
        "--json-out",
        outputs.get("json"),
        "--markdown-out",
        outputs.get("markdown"),
        "--timestamp",
        timestamp,
    ]
    if payload.get("command") != expected:
        failures.append("command does not match canonical specification-source invocation")


def _validate_documents(rows: list[dict[str, Any]], failures: list[str]) -> None:
    if rows != sorted(rows, key=lambda row: _text(row.get("path"))):
        failures.append("documents must be canonical path order")
    _unique(rows, "document_id", "document", failures)
    for row in rows:
        if not _safe_path(row.get("path")):
            failures.append("document path must be normalized and workspace-relative")
        if not isinstance(row.get("content_sha256"), str) or not HEX_DIGEST_RE.fullmatch(row["content_sha256"]):
            failures.append(f"document {row.get('document_id')} has invalid content digest")
        if not _nonnegative_int(row.get("local_reference_count")):
            failures.append(f"document {row.get('document_id')} has invalid reference count")
        if not _canonical_strings(row.get("registered_source_ids")):
            failures.append(f"document {row.get('document_id')} source IDs must be canonical")
        review = row.get("review")
        expected = "reviewed_classification" if isinstance(review, Mapping) else "unreviewed_candidate"
        if row.get("review_state") != expected:
            failures.append(f"document {row.get('document_id')} review state is inconsistent")
        if isinstance(review, Mapping):
            _fields(review, DOCUMENT_REVIEW_FIELDS, f"document {row.get('document_id')} review", failures)
            for field in ("areas", "authority_levels", "owners"):
                if not _canonical_strings(review.get(field)) or not review.get(field):
                    failures.append(f"document {row.get('document_id')} review {field} is invalid")
            if isinstance(review.get("areas"), list) and any(
                item not in AREAS for item in review["areas"]
            ):
                failures.append(f"document {row.get('document_id')} review areas are unknown")
            if isinstance(review.get("authority_levels"), list) and any(
                item not in AUTHORITY_LEVELS for item in review["authority_levels"]
            ):
                failures.append(f"document {row.get('document_id')} authority levels are invalid")
            for field, allowed in (
                ("freshness", FRESHNESS_STATES),
                ("visibility", VISIBILITIES),
                ("classification_basis", CLASSIFICATION_BASES),
                ("conflict_disposition", CONFLICT_DISPOSITIONS),
                ("checklist_staleness", CHECKLIST_STALENESS),
                ("removed_behavior_disposition", REMOVED_BEHAVIOR_DISPOSITIONS),
            ):
                if review.get(field) not in allowed:
                    failures.append(f"document {row.get('document_id')} review {field} is invalid")
            if not isinstance(review.get("oracle_usable"), bool):
                failures.append(f"document {row.get('document_id')} oracle_usable is invalid")
            if not isinstance(review.get("last_reviewed"), str) or not re.fullmatch(
                r"\d{4}-\d{2}-\d{2}", review["last_reviewed"]
            ):
                failures.append(f"document {row.get('document_id')} review date is invalid")
        elif review is not None:
            failures.append(f"document {row.get('document_id')} review must be an object or null")
        if row.get("format") not in ("markdown", "text"):
            failures.append(f"document {row.get('document_id')} format is invalid")
        if not isinstance(row.get("title"), (str, type(None))):
            failures.append(f"document {row.get('document_id')} title is invalid")
        if not isinstance(row.get("in_spec_document_scope"), bool) or not isinstance(
            row.get("primary_public_surface"), bool
        ):
            failures.append(f"document {row.get('document_id')} scope flags must be boolean")
        if not _canonical_strings(row.get("public_entry_paths")):
            failures.append(f"document {row.get('document_id')} public entries must be canonical")


def _validate_sources(
    rows: list[dict[str, Any]],
    documents: list[dict[str, Any]],
    failures: list[str],
) -> None:
    if rows != sorted(rows, key=lambda row: _text(row.get("source_id"))):
        failures.append("source bindings must be canonical source-ID order")
    _unique(rows, "source_id", "source binding", failures)
    document_ids = {
        row.get("document_id")
        for row in documents
        if isinstance(row.get("document_id"), str)
    }
    for row in rows:
        state = row.get("binding_state")
        locator = row.get("locator_kind")
        document_id = row.get("document_id")
        path = row.get("path")
        external_ref = row.get("external_ref")
        expected_local_path = row.get("expected_local_path")
        if locator == "tracked_file":
            if not _safe_path(path) or external_ref is not None or expected_local_path is not None:
                failures.append(f"source {row.get('source_id')} tracked locator is inconsistent")
            if state == "bound" and (
                not isinstance(document_id, str) or document_id not in document_ids
            ):
                failures.append(f"source {row.get('source_id')} binds an unknown document")
            if state == "bound" and document_id is None:
                failures.append(f"source {row.get('source_id')} bound state requires document_id")
            if state != "bound" and document_id is not None:
                failures.append(f"source {row.get('source_id')} unbound state forbids document_id")
        elif locator == "external_reference":
            if (
                path is not None
                or not isinstance(external_ref, str)
                or not external_ref
                or not _safe_path(expected_local_path)
            ):
                failures.append(f"source {row.get('source_id')} external locator is inconsistent")
            if state != "external_reference" or document_id is not None:
                failures.append(f"source {row.get('source_id')} external binding state is inconsistent")
        if not isinstance(row.get("oracle_eligible"), bool):
            failures.append(f"source {row.get('source_id')} oracle_eligible must be boolean")
        if not _canonical_strings(row.get("conflicts_with")):
            failures.append(f"source {row.get('source_id')} conflicts must be canonical")
        if not isinstance(row.get("review_due"), bool):
            failures.append(f"source {row.get('source_id')} review_due must be boolean")
        if row.get("availability") not in (
            "not_applicable",
            "external_bytes_unbound",
        ):
            failures.append(f"source {row.get('source_id')} availability is invalid")
        if locator == "tracked_file" and row.get("availability") != "not_applicable":
            failures.append(f"source {row.get('source_id')} tracked availability is inconsistent")
        if locator == "external_reference" and row.get("availability") != "external_bytes_unbound":
            failures.append(f"source {row.get('source_id')} external availability is inconsistent")


def _validate_topics(rows: list[dict[str, Any]], failures: list[str]) -> None:
    if rows != sorted(rows, key=lambda row: _text(row.get("topic_id"))):
        failures.append("required topics must be canonical topic-ID order")
    _unique(rows, "topic_id", "required topic", failures)
    for row in rows:
        state = row.get("mapping_state")
        if state == "mapped" and (not isinstance(row.get("source_ref"), str) or row.get("spec_gap_ref") is not None):
            failures.append(f"required topic {row.get('topic_id')} mapped state is inconsistent")
        if state == "gap_open" and (not isinstance(row.get("spec_gap_ref"), str) or row.get("source_ref") is not None):
            failures.append(f"required topic {row.get('topic_id')} gap-open state is inconsistent")


def _validate_obvious_topics(rows: list[dict[str, Any]], failures: list[str]) -> None:
    expected_ids = [topic.topic_id for topic in OBVIOUS_SPEC_TOPICS]
    actual_ids = [row.get("topic_id") for row in rows]
    if actual_ids != expected_ids:
        failures.append("obvious specification topics must match reviewed board order")
    for row in rows:
        if row.get("reviewed_posture") not in REVIEWED_POSTURES:
            failures.append(f"obvious topic {row.get('topic_id')} reviewed posture is invalid")
        areas = row.get("areas")
        if not _canonical_strings(areas) or not areas or any(area not in AREAS for area in areas):
            failures.append(f"obvious topic {row.get('topic_id')} areas are invalid")
        for field in (
            "eligible_source_ids",
            "nonoracle_source_ids",
            "open_spec_gap_ids",
            "public_claim_context_ids",
        ):
            if not _canonical_strings(row.get(field)):
                failures.append(f"obvious topic {row.get('topic_id')} {field} is invalid")


def _validate_blocks(
    rows: list[dict[str, Any]],
    documents: list[dict[str, Any]],
    failures: list[str],
) -> None:
    expected = sorted(
        rows,
        key=lambda row: (
            _text(row.get("path")),
            _integer_sort(row.get("line_start")),
            _text(row.get("block_id")),
        ),
    )
    if rows != expected:
        failures.append("public prose blocks must be canonical source order")
    _unique(rows, "block_id", "public block", failures)
    document_ids = {
        row.get("document_id")
        for row in documents
        if isinstance(row.get("document_id"), str)
    }
    for row in rows:
        if not isinstance(row.get("document_id"), str) or row.get("document_id") not in document_ids:
            failures.append(f"public block {row.get('block_id')} references unknown document")
        if not _safe_path(row.get("path")):
            failures.append(f"public block {row.get('block_id')} path is unsafe")
        start, end = row.get("line_start"), row.get("line_end")
        if not _nonnegative_int(start) or not _nonnegative_int(end) or start < 1 or end < start:
            failures.append(f"public block {row.get('block_id')} line range is invalid")
        for digest_field in ("source_text_sha256", "visible_text_sha256"):
            if not isinstance(row.get(digest_field), str) or not HEX_DIGEST_RE.fullmatch(row[digest_field]):
                failures.append(f"public block {row.get('block_id')} {digest_field} is invalid")
        if row.get("block_kind") not in BLOCK_KINDS:
            failures.append(f"public block {row.get('block_id')} block kind is invalid")
        for field in ("heading_path", "public_entry_paths", "registered_claim_ids"):
            if not _canonical_strings(row.get(field), preserve_order=(field == "heading_path")):
                failures.append(f"public block {row.get('block_id')} {field} is invalid")
        review = row.get("claim_review")
        expected_state = "reviewed_disposition" if isinstance(review, Mapping) else "unreviewed_candidate"
        if row.get("review_state") != expected_state:
            failures.append(f"public block {row.get('block_id')} review state is inconsistent")
        if isinstance(review, Mapping):
            _fields(review, CLAIM_REVIEW_FIELDS, f"public block {row.get('block_id')} review", failures)
            if review.get("disposition") not in CLAIM_DISPOSITIONS:
                failures.append(f"public block {row.get('block_id')} disposition is invalid")
            if review.get("rationale_code") not in CLAIM_RATIONALES:
                failures.append(f"public block {row.get('block_id')} rationale is invalid")
            for field in ("invariant_ids", "oracle_refs"):
                if not _canonical_strings(review.get(field)):
                    failures.append(f"public block {row.get('block_id')} {field} is invalid")
            if not isinstance(review.get("last_reviewed"), str) or not re.fullmatch(
                r"\d{4}-\d{2}-\d{2}", review["last_reviewed"]
            ):
                failures.append(f"public block {row.get('block_id')} review date is invalid")
            if review.get("disposition") == "structural_nonclaim" and row.get(
                "block_kind"
            ) not in ("heading", "directive"):
                failures.append(f"public block {row.get('block_id')} structural review is invalid")
            if review.get("disposition") in (
                "structural_nonclaim",
                "claim_without_invariant_or_oracle",
            ) and (review.get("invariant_ids") or review.get("oracle_refs")):
                failures.append(f"public block {row.get('block_id')} unbound review has mappings")
            if review.get("disposition") == "claim_with_mapping" and not (
                review.get("invariant_ids") or review.get("oracle_refs")
            ):
                failures.append(f"public block {row.get('block_id')} mapped review lacks mapping")
        elif review is not None:
            failures.append(f"public block {row.get('block_id')} review must be an object or null")


def _validate_claims(
    rows: list[dict[str, Any]],
    blocks: list[dict[str, Any]],
    failures: list[str],
) -> None:
    if rows != sorted(rows, key=lambda row: _text(row.get("claim_id"))):
        failures.append("public claims must be canonical claim-ID order")
    _unique(rows, "claim_id", "public claim", failures)
    block_ids = {
        row.get("block_id")
        for row in blocks
        if isinstance(row.get("block_id"), str)
    }
    for row in rows:
        ids = row.get("block_ids")
        if not _canonical_strings(ids):
            failures.append(f"public claim {row.get('claim_id')} block IDs must be canonical")
            ids = []
        if any(item not in block_ids for item in ids):
            failures.append(f"public claim {row.get('claim_id')} references unknown block")
        expected = "bound" if len(ids) == 1 else "missing" if not ids else "ambiguous"
        if row.get("binding_state") != expected:
            failures.append(f"public claim {row.get('claim_id')} binding state is inconsistent")
        if not isinstance(row.get("claim_text_sha256"), str) or not HEX_DIGEST_RE.fullmatch(row["claim_text_sha256"]):
            failures.append(f"public claim {row.get('claim_id')} digest is invalid")
        surface_ref = row.get("surface_ref")
        expected_surface = surface_ref.partition("#")[0] if isinstance(surface_ref, str) else None
        if not _safe_path(row.get("surface_path")) or row.get("surface_path") != expected_surface:
            failures.append(f"public claim {row.get('claim_id')} surface path is invalid")


def _validate_findings(rows: list[dict[str, Any]], failures: list[str]) -> None:
    expected = sorted(
        rows,
        key=lambda row: tuple(
            _text(row.get(field))
            for field in ("severity", "code", "path", "record_id")
        ),
    )
    if rows != expected:
        failures.append("findings must be canonical order")
    keys = [
        tuple(_text(row.get(field)) for field in ("code", "path", "record_id"))
        for row in rows
    ]
    if len(keys) != len(set(keys)):
        failures.append("findings must be unique by code/path/record")
    for row in rows:
        if row.get("severity") not in ("error", "warning"):
            failures.append(f"finding {row.get('record_id')} severity is invalid")


def _summary(
    documents: list[dict[str, Any]],
    sources: list[dict[str, Any]],
    topics: list[dict[str, Any]],
    obvious: list[dict[str, Any]],
    blocks: list[dict[str, Any]],
    claims: list[dict[str, Any]],
    findings: list[dict[str, Any]],
    reported: Mapping[str, Any],
) -> dict[str, int]:
    scanner_diagnostics = reported.get("scanner_diagnostics")
    if not _nonnegative_int(scanner_diagnostics):
        scanner_diagnostics = -1
    surfaces = {
        surface
        for row in blocks
        for surface in _string_values(row.get("public_entry_paths"))
    }
    return {
        "documents_total": len(documents),
        "registered_sources": len(sources),
        "bound_sources": sum(row.get("binding_state") == "bound" for row in sources),
        "external_sources": sum(row.get("binding_state") == "external_reference" for row in sources),
        "unbound_sources": sum(
            row.get("binding_state") != "bound"
            and row.get("binding_state") != "external_reference"
            for row in sources
        ),
        "unreviewed_documents": sum(row.get("review_state") == "unreviewed_candidate" for row in documents),
        "classified_documents": sum(
            row.get("review_state") == "reviewed_classification" for row in documents
        ),
        "required_topics_total": len(topics),
        "required_topics_mapped": sum(row.get("mapping_state") == "mapped" for row in topics),
        "required_topics_gap_open": sum(row.get("mapping_state") == "gap_open" for row in topics),
        "required_topics_broken": sum(row.get("mapping_state") == "broken" for row in topics),
        "obvious_spec_topics_total": len(obvious),
        "obvious_spec_source_present": sum(
            row.get("reviewed_posture") == "source_present" for row in obvious
        ),
        "obvious_spec_gap": sum(row.get("reviewed_posture") == "gap_open" for row in obvious),
        "obvious_spec_partial": sum(
            row.get("reviewed_posture") != "source_present"
            and row.get("reviewed_posture") != "gap_open"
            and row.get("reviewed_posture") != "unrepresented"
            for row in obvious
        ),
        "obvious_spec_unrepresented": sum(
            row.get("reviewed_posture") == "unrepresented" for row in obvious
        ),
        "obvious_spec_reference_broken": sum(
            row.get("reference_health") == "broken" for row in obvious
        ),
        "public_surfaces": len(surfaces),
        "public_prose_blocks": len(blocks),
        "registered_public_claims": len(claims),
        "bound_public_claims": sum(row.get("binding_state") == "bound" for row in claims),
        "unbound_public_claims": sum(row.get("binding_state") != "bound" for row in claims),
        "unreviewed_public_blocks": sum(row.get("review_state") == "unreviewed_candidate" for row in blocks),
        "structurally_nonclaim_blocks": sum(
            isinstance(row.get("claim_review"), Mapping)
            and row["claim_review"].get("disposition") == "structural_nonclaim"
            for row in blocks
        ),
        "public_claim_blocks": sum(
            isinstance(row.get("claim_review"), Mapping)
            and row["claim_review"].get("disposition") != "structural_nonclaim"
            for row in blocks
        ),
        "claims_without_invariant_oracle": sum(
            isinstance(row.get("claim_review"), Mapping)
            and row["claim_review"].get("disposition") != "structural_nonclaim"
            and not row["claim_review"].get("invariant_ids")
            and not row["claim_review"].get("oracle_refs")
            for row in blocks
        ),
        "mapped_public_claim_blocks": sum(
            isinstance(row.get("claim_review"), Mapping)
            and row["claim_review"].get("disposition") == "claim_with_mapping"
            for row in blocks
        ),
        "scanner_diagnostics": scanner_diagnostics,
        "source_reviews_due": sum(row.get("review_due") is True for row in sources),
        "blocking_findings": sum(row.get("severity") == "error" for row in findings),
        "warning_findings": sum(row.get("severity") == "warning" for row in findings),
    }


def _rows(payload: Mapping[str, Any], field: str, fields: set[str], failures: list[str]) -> list[dict[str, Any]]:
    value = payload.get(field)
    if not isinstance(value, list):
        failures.append(f"{field} must be an array")
        return []
    rows: list[dict[str, Any]] = []
    for index, item in enumerate(value):
        if not isinstance(item, Mapping):
            failures.append(f"{field}[{index}] must be an object")
            continue
        row = dict(item)
        _fields(row, fields, f"{field}[{index}]", failures)
        rows.append(row)
    return rows


def _fields(value: Mapping[str, Any], expected: set[str], label: str, failures: list[str]) -> None:
    actual = set(value)
    if actual != expected:
        failures.append(f"{label} fields drift: expected {sorted(expected)}, found {sorted(actual)}")


def _unique(rows: list[dict[str, Any]], field: str, label: str, failures: list[str]) -> None:
    values = [row.get(field) for row in rows]
    if any(not isinstance(value, str) or not value for value in values):
        failures.append(f"{label} IDs must be non-empty strings")
    text_values = [_text(value) for value in values]
    if len(text_values) != len(set(text_values)):
        failures.append(f"duplicate {label} ID")


def _safe_path(value: object) -> bool:
    return isinstance(value, str) and is_safe_relative_path(value)


def _string_array(value: object, *, allow_empty: bool = True) -> bool:
    return isinstance(value, list) and (allow_empty or bool(value)) and all(isinstance(item, str) and item for item in value)


def _canonical_strings(value: object, *, preserve_order: bool = False) -> bool:
    if not _string_array(value):
        return False
    assert isinstance(value, list)
    return len(value) == len(set(value)) and (preserve_order or value == sorted(value))


def _nonnegative_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _string_values(value: object) -> tuple[str, ...]:
    if not isinstance(value, list):
        return ()
    return tuple(item for item in value if isinstance(item, str))


def _text(value: object) -> str:
    return value if isinstance(value, str) else repr(value)


def _integer_sort(value: object) -> tuple[int, int]:
    if isinstance(value, int) and not isinstance(value, bool):
        return (0, value)
    return (1, 0)


def _timestamp(value: object) -> bool:
    if not isinstance(value, str) or not value:
        return False
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None


def _definition_properties(definitions: Mapping[str, Any], name: str) -> Mapping[str, Any]:
    definition = definitions.get(name, {})
    if not isinstance(definition, Mapping):
        return {}
    properties = definition.get("properties", {})
    return properties if isinstance(properties, Mapping) else {}


def _property(properties: Mapping[str, Any], name: str) -> Mapping[str, Any]:
    value = properties.get(name, {})
    return value if isinstance(value, Mapping) else {}


def _closed_objects(node: object, path: str, failures: list[str]) -> None:
    if isinstance(node, Mapping):
        if node.get("type") == "object" and node.get("additionalProperties") is not False:
            failures.append(f"{path} object must set additionalProperties=false")
        for key, value in node.items():
            _closed_objects(value, f"{path}.{key}", failures)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            _closed_objects(value, f"{path}[{index}]", failures)
