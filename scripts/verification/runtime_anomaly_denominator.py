"""Exhaustive reviewed denominator for Phase 8 runtime-anomaly associations."""

from __future__ import annotations

import hashlib
import json
import re
import tomllib
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .test_catalog_models import InferredTestFact
from .test_catalog_json_schema import validate_json_schema_instance


NONMAPPING_REASON_CODES = (
    "outside_runtime_safety_scope",
    "no_taxonomy_stimulus_or_response",
    "supporting_internal_contract_only",
    "different_safety_domain",
)
NONMAPPING_REASON_DEFINITIONS = {
    "outside_runtime_safety_scope": (
        "The reviewed test belongs to a compiler, language-service, developer-tool, "
        "or verification-helper boundary and does not exercise a shipped runtime "
        "response to one of the nineteen Phase 8 stimuli."
    ),
    "no_taxonomy_stimulus_or_response": (
        "The reviewed test exercises ordinary behavior or a non-anomaly input and "
        "does not both create a Phase 8 stimulus and assert its runtime response."
    ),
    "supporting_internal_contract_only": (
        "The reviewed test checks an internal helper, representation, parser, or "
        "component contract without independently exercising the observable runtime "
        "response required for a Phase 8 association."
    ),
    "different_safety_domain": (
        "The reviewed test exercises a security, authorization, compatibility, "
        "release, or product contract outside the closed Phase 8 anomaly taxonomy."
    ),
}
REVIEW_PATH = "verification/runtime-anomaly-denominator.toml"
REVIEW_SCHEMA_PATH = "verification/schemas/runtime-anomaly-denominator.schema.json"
ROOT_FIELDS = {
    "schema_version",
    "id",
    "title",
    "area",
    "denominator_basis",
    "review_basis",
    "reason_definitions",
    "last_reviewed",
    "reviews",
}
ROOT_CONSTS = {
    "schema_version": 1,
    "id": "RUNTIME_ANOMALY_DENOMINATOR_REVIEW_V1",
    "area": "runtime_safety",
    "denominator_basis": "all_live_production_rust_test_facts",
    "review_basis": "explicit_per_discovery_id_no_lexical_inference",
}
DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
DATE_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$")
MAPPED_FIELDS = {
    "discovery_id",
    "discovery_source_kind",
    "path",
    "name",
    "disposition",
    "mapping_id",
    "last_reviewed",
}
NONMAPPING_FIELDS = {
    "discovery_id",
    "discovery_source_kind",
    "path",
    "name",
    "disposition",
    "reason_code",
    "last_reviewed",
}


def load_runtime_anomaly_denominator_review(root: Path) -> dict[str, Any]:
    """Load the committed per-fact denominator review."""

    return tomllib.loads((root / REVIEW_PATH).read_text())


def validate_runtime_anomaly_denominator_document(
    root: Path,
    document: Mapping[str, Any],
) -> list[str]:
    """Validate the closed ledger shape independently of the live scanner join."""

    failures: list[str] = []
    try:
        schema = json.loads((root / REVIEW_SCHEMA_PATH).read_text())
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return [f"runtime-anomaly denominator schema cannot be read: {exc}"]
    failures.extend(validate_json_schema_instance(dict(document), schema))
    if set(document) != ROOT_FIELDS:
        failures.append("runtime-anomaly denominator root fields drift from contract")
    for field, expected in ROOT_CONSTS.items():
        if document.get(field) != expected:
            failures.append(f"runtime-anomaly denominator {field} must equal {expected!r}")
    if document.get("reason_definitions") != NONMAPPING_REASON_DEFINITIONS:
        failures.append("runtime-anomaly denominator reason definitions drift from contract")
    title = document.get("title")
    if not isinstance(title, str) or not title:
        failures.append("runtime-anomaly denominator title must be non-empty")
    last_reviewed = document.get("last_reviewed")
    if not isinstance(last_reviewed, str) or not DATE_RE.fullmatch(last_reviewed):
        failures.append("runtime-anomaly denominator requires YYYY-MM-DD last_reviewed")
    reviews = document.get("reviews")
    if not isinstance(reviews, list) or not reviews:
        failures.append("runtime-anomaly denominator reviews must be a non-empty array")
    return sorted(set(failures))


def analyze_runtime_anomaly_denominator(
    *,
    facts: Sequence[InferredTestFact],
    mappings: Sequence[Mapping[str, Any]],
    reviews: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Validate and summarize explicit per-fact review decisions."""

    facts_by_id: dict[str, list[InferredTestFact]] = defaultdict(list)
    for fact in facts:
        facts_by_id[fact.stable_id].append(fact)
    duplicates = sorted(key for key, rows in facts_by_id.items() if len(rows) != 1)
    if duplicates:
        raise ValueError("live scanner facts contain duplicate discovery IDs: " + ", ".join(duplicates))

    mappings_by_discovery: dict[str, Mapping[str, Any]] = {}
    mapping_ids: set[str] = set()
    for mapping in mappings:
        mapping_id = _required_string(mapping, "id", "runtime-anomaly mapping")
        discovery_id = _required_string(mapping, "discovery_id", mapping_id)
        if mapping_id in mapping_ids:
            raise ValueError(f"runtime-anomaly mappings duplicate mapping ID {mapping_id}")
        if discovery_id in mappings_by_discovery:
            raise ValueError(f"runtime-anomaly mappings duplicate discovery ID {discovery_id}")
        mapping_ids.add(mapping_id)
        mappings_by_discovery[discovery_id] = mapping

    review_ids = [review.get("discovery_id") for review in reviews]
    if review_ids != sorted(review_ids, key=lambda value: str(value)):
        raise ValueError("runtime-anomaly reviews must use canonical discovery_id order")

    seen: set[str] = set()
    reason_counts: Counter[str] = Counter()
    normalized: list[dict[str, Any]] = []
    for index, review in enumerate(reviews):
        if not isinstance(review, Mapping):
            raise ValueError(f"runtime-anomaly review {index} must be a table")
        discovery_id = _required_string(review, "discovery_id", f"review {index}")
        if not DISCOVERY_ID_RE.fullmatch(discovery_id):
            raise ValueError(f"runtime-anomaly review has invalid discovery_id {discovery_id!r}")
        if discovery_id in seen:
            raise ValueError(f"runtime-anomaly duplicate review for {discovery_id}")
        seen.add(discovery_id)
        fact_rows = facts_by_id.get(discovery_id)
        if not fact_rows:
            raise ValueError(f"runtime-anomaly review {discovery_id} is absent from live scanner facts")
        fact = fact_rows[0]
        _validate_fact_binding(review, fact)
        last_reviewed = review.get("last_reviewed")
        if not isinstance(last_reviewed, str) or not DATE_RE.fullmatch(last_reviewed):
            raise ValueError(f"runtime-anomaly review {discovery_id} requires YYYY-MM-DD last_reviewed")

        mapped = mappings_by_discovery.get(discovery_id)
        disposition = review.get("disposition")
        if mapped is not None:
            if disposition != "mapped":
                raise ValueError(
                    f"runtime-anomaly mapped scanner fact must use mapped disposition: {discovery_id}"
                )
            if set(review) != MAPPED_FIELDS:
                raise ValueError(f"runtime-anomaly mapped review {discovery_id} fields drift")
            if review.get("mapping_id") != mapped.get("id"):
                raise ValueError(f"runtime-anomaly review {discovery_id} mapping_id does not match")
        else:
            if disposition != "reviewed_nonmapping":
                raise ValueError(
                    f"runtime-anomaly unmapped scanner fact must use reviewed_nonmapping: {discovery_id}"
                )
            if "mapping_id" in review:
                raise ValueError(f"runtime-anomaly nonmapping review {discovery_id} forbids mapping_id")
            if set(review) != NONMAPPING_FIELDS:
                raise ValueError(f"runtime-anomaly nonmapping review {discovery_id} fields drift")
            reason_code = review.get("reason_code")
            if reason_code not in NONMAPPING_REASON_CODES:
                raise ValueError(
                    f"runtime-anomaly review {discovery_id} has unsupported reason_code {reason_code!r}"
                )
            reason_counts[str(reason_code)] += 1
        normalized.append(dict(review))

    missing = sorted(set(facts_by_id) - seen)
    if missing:
        raise ValueError(
            f"runtime-anomaly denominator has {len(missing)} unreviewed live scanner facts: "
            + ", ".join(missing[:8])
        )
    missing_mapping_reviews = sorted(set(mappings_by_discovery) - seen)
    if missing_mapping_reviews:
        raise ValueError(
            "runtime-anomaly mappings lack denominator reviews: "
            + ", ".join(missing_mapping_reviews)
        )

    mapped_count = len(mappings_by_discovery)
    nonmapping_count = len(normalized) - mapped_count
    canonical = json.dumps(normalized, separators=(",", ":"), sort_keys=True)
    return {
        "review_digest": "sha256:" + hashlib.sha256(canonical.encode()).hexdigest(),
        "summary": {
            "scanner_denominator": len(facts),
            "mapped_facts": mapped_count,
            "reviewed_nonmapping_facts": nonmapping_count,
            "unreviewed_facts": 0,
            "exhaustive": True,
            "by_nonmapping_reason": {
                code: reason_counts[code] for code in NONMAPPING_REASON_CODES
            },
        },
    }


def _validate_fact_binding(review: Mapping[str, Any], fact: InferredTestFact) -> None:
    for field, actual in (
        ("discovery_source_kind", fact.source_kind),
        ("path", fact.path),
        ("name", fact.name),
    ):
        if review.get(field) != actual:
            raise ValueError(
                f"runtime-anomaly review {fact.stable_id} {field} does not match live scanner fact"
            )


def _required_string(record: Mapping[str, Any], field: str, label: str) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
