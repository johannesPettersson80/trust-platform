"""Identity-bound review registries for specification documents and public prose."""

from __future__ import annotations

import re
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .metadata_validator.constants import AREAS


DOCUMENT_REVIEWS_PATH = "verification/spec-document-reviews.toml"
PUBLIC_PROSE_REVIEWS_PATH = "verification/public-prose-reviews.toml"
SCHEMA_VERSION = 1
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

AUTHORITY_LEVELS = (
    "normative_product",
    "normative_candidate",
    "reviewed_decision",
    "reviewed_deviation",
    "public_claim",
    "implementation_guide",
    "internal_governance",
    "historical_evidence",
    "example_context",
)
FRESHNESS_STATES = ("current", "historical", "review_due")
VISIBILITIES = ("public", "internal")
CLASSIFICATION_BASES = ("registered_metadata", "explicit_review")
CONFLICT_DISPOSITIONS = (
    "registered_conflicts_reviewed",
    "not_oracle_authority",
    "potential_conflict_reported",
)
CHECKLIST_STALENESS = (
    "not_applicable",
    "active_current",
    "historical_non_authoritative",
)
REMOVED_BEHAVIOR_DISPOSITIONS = (
    "reviewed_current",
    "not_oracle_authority",
    "potentially_stale_reported",
)
CLAIM_DISPOSITIONS = (
    "registered_claim",
    "structural_nonclaim",
    "claim_without_invariant_or_oracle",
    "claim_with_mapping",
)
CLAIM_RATIONALES = (
    "registered_public_claim_metadata",
    "document_structure",
    "conservative_unbound_public_prose",
    "explicit_invariant_or_oracle_binding",
)

DOCUMENT_FIELDS = {
    "schema_version",
    "document_id",
    "path",
    "content_sha256",
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
PUBLIC_BLOCK_FIELDS = {
    "schema_version",
    "block_id",
    "document_id",
    "path",
    "block_kind",
    "visible_text_sha256",
    "disposition",
    "invariant_ids",
    "oracle_refs",
    "rationale_code",
    "last_reviewed",
}


def load_spec_source_reviews(
    root: Path,
    *,
    documents: Sequence[Mapping[str, Any]],
    public_blocks: Sequence[Mapping[str, Any]],
    spec_sources: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]], list[str]]:
    """Load both complete registries and bind every row to a live scanner fact."""

    root = root.resolve()
    failures: list[str] = []
    document_rows = _load_rows(root / DOCUMENT_REVIEWS_PATH, "document_reviews", failures)
    block_rows = _load_rows(root / PUBLIC_PROSE_REVIEWS_PATH, "public_block_reviews", failures)
    documents_by_id = _index_facts(documents, "document_id", "document", failures)
    blocks_by_id = _index_facts(public_blocks, "block_id", "public block", failures)
    sources_by_path: dict[str, list[Mapping[str, Any]]] = {}
    for source in spec_sources.values():
        path = source.get("path")
        if isinstance(path, str):
            sources_by_path.setdefault(path, []).append(source)

    document_reviews = _validate_document_rows(
        document_rows,
        documents_by_id,
        sources_by_path,
        failures,
    )
    block_reviews = _validate_block_rows(
        block_rows,
        blocks_by_id,
        spec_sources,
        invariants,
        failures,
    )
    _exhaustive_ids(
        set(documents_by_id),
        set(document_reviews),
        "document review",
        failures,
    )
    _exhaustive_ids(
        set(blocks_by_id),
        set(block_reviews),
        "public-block review",
        failures,
    )
    return document_reviews, block_reviews, sorted(set(failures))


def _load_rows(path: Path, field: str, failures: list[str]) -> list[dict[str, Any]]:
    try:
        payload = tomllib.loads(path.read_text())
    except (OSError, tomllib.TOMLDecodeError) as exc:
        failures.append(f"could not load {path.name}: {exc}")
        return []
    if set(payload) != {field}:
        failures.append(f"{path.name} must contain only [[{field}]] rows")
    rows = payload.get(field)
    if not isinstance(rows, list):
        failures.append(f"{path.name} {field} must be an array")
        return []
    return [dict(row) for row in rows if isinstance(row, Mapping)]


def _index_facts(
    rows: Sequence[Mapping[str, Any]],
    field: str,
    label: str,
    failures: list[str],
) -> dict[str, Mapping[str, Any]]:
    result: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        value = row.get(field)
        if not isinstance(value, str) or not value:
            failures.append(f"live {label} has an invalid {field}")
            continue
        if value in result:
            failures.append(f"duplicate live {label} identity {value}")
        result[value] = row
    return result


def _validate_document_rows(
    rows: list[dict[str, Any]],
    facts: Mapping[str, Mapping[str, Any]],
    sources_by_path: Mapping[str, list[Mapping[str, Any]]],
    failures: list[str],
) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    ids = [row.get("document_id") for row in rows]
    if ids != sorted(ids, key=lambda value: value if isinstance(value, str) else repr(value)):
        failures.append("document reviews must use canonical document-ID order")
    for index, row in enumerate(rows):
        label = f"document_reviews[{index}]"
        _exact_fields(row, DOCUMENT_FIELDS, label, failures)
        document_id = row.get("document_id")
        if not isinstance(document_id, str) or not document_id:
            failures.append(f"{label} document_id must be a non-empty string")
            continue
        if document_id in result:
            failures.append(f"duplicate document review {document_id}")
            continue
        fact = facts.get(document_id)
        if fact is None:
            failures.append(f"document review {document_id} is absent from live scanner facts")
            continue
        for field in ("path", "content_sha256"):
            if row.get(field) != fact.get(field):
                failures.append(f"document review {document_id} {field} does not match live fact")
        _schema_and_date(row, label, failures)
        areas = _canonical_strings(row.get("areas"), allow_empty=False)
        owners = _canonical_strings(row.get("owners"), allow_empty=False)
        if areas is None or any(area not in AREAS for area in areas):
            failures.append(f"document review {document_id} areas must be canonical known IDs")
        if owners is None:
            failures.append(f"document review {document_id} owners must be canonical")
        authorities = _canonical_strings(row.get("authority_levels"), allow_empty=False)
        if authorities is None or any(item not in AUTHORITY_LEVELS for item in authorities):
            failures.append(
                f"document review {document_id} authority_levels must be canonical known values"
            )
        _enum(row, "freshness", FRESHNESS_STATES, label, failures)
        _enum(row, "visibility", VISIBILITIES, label, failures)
        _enum(row, "classification_basis", CLASSIFICATION_BASES, label, failures)
        _enum(row, "conflict_disposition", CONFLICT_DISPOSITIONS, label, failures)
        _enum(row, "checklist_staleness", CHECKLIST_STALENESS, label, failures)
        _enum(
            row,
            "removed_behavior_disposition",
            REMOVED_BEHAVIOR_DISPOSITIONS,
            label,
            failures,
        )
        if not isinstance(row.get("oracle_usable"), bool):
            failures.append(f"document review {document_id} oracle_usable must be boolean")
        sources = sources_by_path.get(str(row.get("path")), [])
        if row.get("classification_basis") == "registered_metadata":
            if not sources:
                failures.append(f"document review {document_id} claims absent registered metadata")
            expected_areas = sorted({str(source.get("area")) for source in sources})
            expected_owners = sorted({str(source.get("owner")) for source in sources})
            expected_authorities = sorted({str(source.get("authority")) for source in sources})
            expected_oracle = any(
                source.get("source_status") == "active"
                and source.get("oracle_eligible") is True
                for source in sources
            )
            if areas != expected_areas:
                failures.append(f"document review {document_id} areas drift from registered metadata")
            if owners != expected_owners:
                failures.append(f"document review {document_id} owners drift from registered metadata")
            if authorities != expected_authorities:
                failures.append(f"document review {document_id} authority drifts from registered metadata")
            if row.get("oracle_usable") is not expected_oracle:
                failures.append(f"document review {document_id} oracle posture drifts from registered metadata")
        elif sources:
            failures.append(f"document review {document_id} must use registered_metadata basis")
        elif row.get("oracle_usable") is not False:
            failures.append(f"unregistered document review {document_id} cannot be oracle usable")
        path = str(row.get("path"))
        is_checklist = "/checklists/" in path
        if is_checklist and row.get("checklist_staleness") == "not_applicable":
            failures.append(f"checklist document review {document_id} needs a staleness disposition")
        if not is_checklist and row.get("checklist_staleness") != "not_applicable":
            failures.append(f"non-checklist document review {document_id} has checklist disposition")
        if row.get("conflict_disposition") == "registered_conflicts_reviewed" and not sources:
            failures.append(f"document review {document_id} lacks registered conflicts to review")
        if row.get("conflict_disposition") == "not_oracle_authority" and row.get("oracle_usable") is not False:
            failures.append(f"document review {document_id} is an oracle but claims no oracle authority")
        if row.get("removed_behavior_disposition") == "not_oracle_authority" and row.get("oracle_usable") is not False:
            failures.append(f"document review {document_id} is an oracle but claims no oracle authority")
        result[document_id] = row
    return result


def _validate_block_rows(
    rows: list[dict[str, Any]],
    facts: Mapping[str, Mapping[str, Any]],
    spec_sources: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    failures: list[str],
) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    ids = [row.get("block_id") for row in rows]
    if ids != sorted(ids, key=lambda value: value if isinstance(value, str) else repr(value)):
        failures.append("public-block reviews must use canonical block-ID order")
    for index, row in enumerate(rows):
        label = f"public_block_reviews[{index}]"
        _exact_fields(row, PUBLIC_BLOCK_FIELDS, label, failures)
        block_id = row.get("block_id")
        if not isinstance(block_id, str) or not block_id:
            failures.append(f"{label} block_id must be a non-empty string")
            continue
        if block_id in result:
            failures.append(f"duplicate public-block review {block_id}")
            continue
        fact = facts.get(block_id)
        if fact is None:
            failures.append(f"public-block review {block_id} is absent from live scanner facts")
            continue
        for field in (
            "document_id",
            "path",
            "block_kind",
            "visible_text_sha256",
        ):
            if row.get(field) != fact.get(field):
                failures.append(f"public-block review {block_id} {field} does not match live fact")
        _schema_and_date(row, label, failures)
        _enum(row, "disposition", CLAIM_DISPOSITIONS, label, failures)
        _enum(row, "rationale_code", CLAIM_RATIONALES, label, failures)
        invariant_ids = _canonical_strings(row.get("invariant_ids"))
        oracle_refs = _canonical_strings(row.get("oracle_refs"))
        if invariant_ids is None:
            failures.append(f"public-block review {block_id} invariant_ids must be canonical")
            invariant_ids = []
        if oracle_refs is None:
            failures.append(f"public-block review {block_id} oracle_refs must be canonical")
            oracle_refs = []
        for invariant_id in invariant_ids:
            if invariant_id not in invariants:
                failures.append(f"public-block review {block_id} names unknown invariant {invariant_id}")
        for oracle_ref in oracle_refs:
            source = spec_sources.get(oracle_ref)
            if (
                source is None
                or source.get("source_status") != "active"
                or source.get("oracle_eligible") is not True
                or source.get("authority") == "public_claim"
            ):
                failures.append(f"public-block review {block_id} names ineligible oracle {oracle_ref}")
        disposition = row.get("disposition")
        if disposition == "structural_nonclaim":
            if row.get("block_kind") not in {"heading", "directive"}:
                failures.append(f"public-block review {block_id} structural disposition is invalid")
            if invariant_ids or oracle_refs:
                failures.append(f"public-block review {block_id} structural disposition forbids mappings")
            if row.get("rationale_code") != "document_structure":
                failures.append(f"public-block review {block_id} structural rationale is invalid")
        elif disposition == "claim_without_invariant_or_oracle":
            if invariant_ids or oracle_refs:
                failures.append(f"public-block review {block_id} unbound disposition forbids mappings")
            if row.get("rationale_code") != "conservative_unbound_public_prose":
                failures.append(f"public-block review {block_id} unbound rationale is invalid")
        elif disposition == "claim_with_mapping":
            if not invariant_ids and not oracle_refs:
                failures.append(f"public-block review {block_id} mapped disposition needs a mapping")
            if row.get("rationale_code") != "explicit_invariant_or_oracle_binding":
                failures.append(f"public-block review {block_id} mapped rationale is invalid")
        elif disposition == "registered_claim":
            if row.get("rationale_code") != "registered_public_claim_metadata":
                failures.append(f"public-block review {block_id} registered rationale is invalid")
        result[block_id] = row
    return result


def _schema_and_date(row: Mapping[str, Any], label: str, failures: list[str]) -> None:
    if row.get("schema_version") != SCHEMA_VERSION:
        failures.append(f"{label} must use schema_version = {SCHEMA_VERSION}")
    if not isinstance(row.get("last_reviewed"), str) or not DATE_RE.fullmatch(row["last_reviewed"]):
        failures.append(f"{label} last_reviewed must use YYYY-MM-DD")


def _exact_fields(
    row: Mapping[str, Any],
    expected: set[str],
    label: str,
    failures: list[str],
) -> None:
    if set(row) != expected:
        failures.append(
            f"{label} fields drift: expected {sorted(expected)}, found {sorted(row)}"
        )


def _canonical_strings(value: object, *, allow_empty: bool = True) -> list[str] | None:
    if not isinstance(value, list) or (not allow_empty and not value):
        return None
    if any(not isinstance(item, str) or not item for item in value):
        return None
    if value != sorted(set(value)):
        return None
    return list(value)


def _enum(
    row: Mapping[str, Any],
    field: str,
    allowed: tuple[str, ...],
    label: str,
    failures: list[str],
) -> None:
    if row.get(field) not in allowed:
        failures.append(f"{label} {field} must be one of {list(allowed)}")


def _exhaustive_ids(
    expected: set[str],
    actual: set[str],
    label: str,
    failures: list[str],
) -> None:
    missing = sorted(expected - actual)
    invented = sorted(actual - expected)
    if missing:
        failures.append(f"{label} registry is missing live IDs: {', '.join(missing[:8])}")
    if invented:
        failures.append(f"{label} registry invents IDs: {', '.join(invented[:8])}")
