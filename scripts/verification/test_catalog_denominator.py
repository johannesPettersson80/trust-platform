"""Exhaustive reviewed disposition contract for existing-test scanner facts."""

from __future__ import annotations

import hashlib
import json
import re
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .test_catalog_models import InferredTestFact
from .test_catalog_json_schema import validate_json_schema_instance


DENOMINATOR_PATH = "verification/test-catalog-denominator.toml"
DENOMINATOR_SCHEMA_PATH = "verification/schemas/test-catalog-denominator.schema.json"
NONMAPPING_REASON_CODES = (
    "no_reviewed_spec_or_invariant_binding",
    "ignored_test_registry_owned",
    "fuzz_program_owned",
    "gate_inventory_owned",
)
NONMAPPING_REASON_DEFINITIONS = {
    "no_reviewed_spec_or_invariant_binding": (
        "The fact remains part of its native test suite, but no hand-reviewed "
        "catalog row authorizes a specification, invariant, oracle, or expected-result "
        "claim for it. It is retired from mapping debt without being deleted."
    ),
    "ignored_test_registry_owned": (
        "The ignored or conditional fact is governed by its exact Phase 3 ignored-test "
        "registry row and has no separate hand-reviewed invariant catalog mapping."
    ),
    "fuzz_program_owned": (
        "The fuzz target is governed by the dedicated Phase 9 fuzz-program and "
        "crash-regression ledgers rather than an inferred invariant catalog mapping."
    ),
    "gate_inventory_owned": (
        "The gate script or workflow job is governed by the Phase 5 gate inventory "
        "and suite-routing contracts rather than a product-test invariant mapping."
    ),
}
DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
DATE_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$")
MAPPED_FIELDS = {
    "discovery_id",
    "discovery_source_kind",
    "path",
    "name",
    "ignore_state",
    "disposition",
    "catalog_test_id",
    "last_reviewed",
}
NONMAPPING_FIELDS = {
    "discovery_id",
    "discovery_source_kind",
    "path",
    "name",
    "ignore_state",
    "disposition",
    "reason_code",
    "last_reviewed",
}
ROOT_FIELDS = {
    "schema_version",
    "id",
    "title",
    "denominator_basis",
    "review_basis",
    "reason_definitions",
    "last_reviewed",
    "reviews",
}
ROOT_CONSTS = {
    "schema_version": 1,
    "id": "TEST_CATALOG_DENOMINATOR_REVIEW_V1",
    "denominator_basis": "all_live_existing_test_scanner_facts",
    "review_basis": "explicit_per_discovery_id_no_semantic_inference",
}


def load_test_catalog_denominator(root: Path) -> dict[str, Any]:
    import tomllib

    return tomllib.loads((root / DENOMINATOR_PATH).read_text())


def validate_test_catalog_denominator_document(
    document: Mapping[str, Any],
    *,
    schema: Mapping[str, Any] | None = None,
) -> list[str]:
    """Validate closed document shape before the live scanner join."""

    failures: list[str] = []
    if schema is not None:
        failures.extend(validate_json_schema_instance(dict(document), dict(schema)))
    if set(document) != ROOT_FIELDS:
        failures.append("test-catalog denominator root fields drift from contract")
    for field, expected in ROOT_CONSTS.items():
        if document.get(field) != expected:
            failures.append(f"test-catalog denominator {field} must equal {expected!r}")
    if document.get("reason_definitions") != NONMAPPING_REASON_DEFINITIONS:
        failures.append("test-catalog denominator reason definitions drift from contract")
    if not isinstance(document.get("title"), str) or not document.get("title"):
        failures.append("test-catalog denominator title must be non-empty")
    reviewed = document.get("last_reviewed")
    if not isinstance(reviewed, str) or not DATE_RE.fullmatch(reviewed):
        failures.append("test-catalog denominator requires YYYY-MM-DD last_reviewed")
    reviews = document.get("reviews")
    if not isinstance(reviews, list) or not reviews:
        failures.append("test-catalog denominator reviews must be a non-empty array")
    return sorted(set(failures))


def analyze_test_catalog_denominator(
    *,
    facts: Sequence[InferredTestFact],
    tests: Sequence[Mapping[str, Any]],
    reviews: Sequence[Mapping[str, Any]],
    ignored_tests: Sequence[Mapping[str, Any]] | Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    """Validate the exact catalog-mapped/reviewed-nonmapping partition."""

    facts_by_id: dict[str, list[InferredTestFact]] = defaultdict(list)
    for fact in facts:
        facts_by_id[fact.stable_id].append(fact)
    duplicate_facts = sorted(key for key, rows in facts_by_id.items() if len(rows) != 1)
    if duplicate_facts:
        raise ValueError(
            "scanner duplicates discovery id: "
            + ", ".join(duplicate_facts)
        )

    catalog_by_discovery: dict[str, str] = {}
    catalog_ids: set[str] = set()
    for record in tests:
        test_id = _required_string(record, "id", "catalog record")
        if test_id in catalog_ids:
            raise ValueError(f"catalog duplicates test ID {test_id}")
        catalog_ids.add(test_id)
        if record.get("subject_kind") != "generated_test":
            continue
        discovery_id = _required_string(record, "discovery_id", test_id)
        if discovery_id not in facts_by_id:
            raise ValueError(
                f"{test_id} discovery_id is absent from current scanner facts: {discovery_id}"
            )
        previous = catalog_by_discovery.get(discovery_id)
        if previous is not None:
            raise ValueError(
                f"scanner fact {discovery_id} is classified by both {previous} and {test_id}"
            )
        catalog_by_discovery[discovery_id] = test_id

    ignored_by_discovery: dict[str, Mapping[str, Any]] = {}
    ignored_records = ignored_tests.values() if isinstance(ignored_tests, Mapping) else ignored_tests
    for record in ignored_records:
        discovery_id = _required_string(record, "discovery_id", "ignored registry row")
        if discovery_id in ignored_by_discovery:
            raise ValueError(f"ignored registry duplicates discovery ID {discovery_id}")
        ignored_by_discovery[discovery_id] = record

    review_ids = [row.get("discovery_id") for row in reviews]
    if review_ids != sorted(review_ids, key=lambda value: str(value)):
        raise ValueError("test-catalog denominator reviews must use canonical discovery_id order")

    seen: set[str] = set()
    reason_counts: Counter[str] = Counter()
    source_counts: Counter[str] = Counter()
    mapped_source_counts: Counter[str] = Counter()
    ignored_owned = 0
    normalized: list[dict[str, Any]] = []
    for index, review in enumerate(reviews):
        if not isinstance(review, Mapping):
            raise ValueError(f"test-catalog denominator review {index} must be a table")
        discovery_id = _required_string(review, "discovery_id", f"review {index}")
        if not DISCOVERY_ID_RE.fullmatch(discovery_id):
            raise ValueError(f"invalid denominator discovery ID {discovery_id!r}")
        if discovery_id in seen:
            raise ValueError(f"test-catalog denominator duplicate review for {discovery_id}")
        seen.add(discovery_id)
        fact_rows = facts_by_id.get(discovery_id)
        if not fact_rows:
            raise ValueError(f"denominator review {discovery_id} is absent from live scanner facts")
        fact = fact_rows[0]
        _validate_fact_binding(review, fact)
        source_counts[fact.source_kind] += 1
        reviewed = review.get("last_reviewed")
        if not isinstance(reviewed, str) or not DATE_RE.fullmatch(reviewed):
            raise ValueError(f"denominator review {discovery_id} requires YYYY-MM-DD last_reviewed")

        catalog_test_id = catalog_by_discovery.get(discovery_id)
        if catalog_test_id is not None:
            if review.get("disposition") != "catalog_mapped":
                raise ValueError(
                    f"catalog-mapped fact must use catalog_mapped disposition: {discovery_id}"
                )
            if set(review) != MAPPED_FIELDS:
                raise ValueError(f"catalog-mapped review {discovery_id} fields drift")
            if review.get("catalog_test_id") != catalog_test_id:
                raise ValueError(f"review {discovery_id} catalog_test_id does not match")
            mapped_source_counts[fact.source_kind] += 1
        else:
            if review.get("disposition") != "reviewed_nonmapping":
                raise ValueError(
                    f"nonmapped fact must use reviewed_nonmapping disposition: {discovery_id}"
                )
            if "catalog_test_id" in review:
                raise ValueError(f"nonmapping review {discovery_id} forbids catalog_test_id")
            if set(review) != NONMAPPING_FIELDS:
                raise ValueError(f"nonmapping review {discovery_id} fields drift")
            reason = review.get("reason_code")
            if reason not in NONMAPPING_REASON_CODES:
                raise ValueError(f"unsupported reason_code {reason!r} for {discovery_id}")
            _validate_reason_binding(
                discovery_id=discovery_id,
                fact=fact,
                reason=str(reason),
                ignored=ignored_by_discovery.get(discovery_id),
            )
            if reason == "ignored_test_registry_owned":
                ignored_owned += 1
            reason_counts[str(reason)] += 1
        normalized.append(dict(review))

    missing = sorted(set(facts_by_id) - seen)
    if missing:
        raise ValueError(
            f"test-catalog denominator has {len(missing)} unreviewed live scanner facts: "
            + ", ".join(missing[:8])
        )
    missing_catalog = sorted(set(catalog_by_discovery) - seen)
    if missing_catalog:
        raise ValueError(
            "catalog mappings lack denominator reviews: " + ", ".join(missing_catalog)
        )

    source_kinds = sorted(source_counts)
    canonical = json.dumps(normalized, separators=(",", ":"), sort_keys=True)
    mapped_count = len(catalog_by_discovery)
    return {
        "review_digest": "sha256:" + hashlib.sha256(canonical.encode()).hexdigest(),
        "summary": {
            "scanner_facts": len(facts_by_id),
            "catalog_mapped_facts": mapped_count,
            "reviewed_nonmapping_facts": len(normalized) - mapped_count,
            "unreviewed_facts": 0,
            "exhaustive": True,
            "ignored_registry_owned_facts": ignored_owned,
            "by_nonmapping_reason": {
                reason: reason_counts[reason] for reason in NONMAPPING_REASON_CODES
            },
            "by_source_kind": [
                {
                    "source_kind": source_kind,
                    "scanner_facts": source_counts[source_kind],
                    "catalog_mapped": mapped_source_counts[source_kind],
                    "reviewed_nonmapping": (
                        source_counts[source_kind] - mapped_source_counts[source_kind]
                    ),
                }
                for source_kind in source_kinds
            ],
        },
    }


def _validate_fact_binding(review: Mapping[str, Any], fact: InferredTestFact) -> None:
    for field, actual in (
        ("discovery_source_kind", fact.source_kind),
        ("path", fact.path),
        ("name", fact.name),
        ("ignore_state", fact.ignore_state),
    ):
        if review.get(field) != actual:
            raise ValueError(
                f"denominator review {fact.stable_id} {field} does not match live scanner fact"
            )


def _validate_reason_binding(
    *,
    discovery_id: str,
    fact: InferredTestFact,
    reason: str,
    ignored: Mapping[str, Any] | None,
) -> None:
    if fact.ignore_state != "not_ignored":
        if reason != "ignored_test_registry_owned":
            raise ValueError(f"ignored fact {discovery_id} requires ignored registry rationale")
        if ignored is None or ignored.get("ignore_state") != fact.ignore_state:
            raise ValueError(f"ignored fact {discovery_id} lacks exact ignored registry binding")
    elif reason == "ignored_test_registry_owned":
        raise ValueError(f"non-ignored fact {discovery_id} cannot use ignored registry rationale")
    if reason == "fuzz_program_owned" and fact.source_kind != "fuzz_target":
        raise ValueError(f"fuzz_program_owned requires fuzz_target source kind: {discovery_id}")
    if fact.source_kind == "fuzz_target" and reason != "fuzz_program_owned":
        raise ValueError(f"fuzz target {discovery_id} requires fuzz_program_owned rationale")
    gate_kinds = {"gate_script", "github_workflow_job"}
    if reason == "gate_inventory_owned" and fact.source_kind not in gate_kinds:
        raise ValueError(f"gate_inventory_owned requires a gate source kind: {discovery_id}")
    if fact.source_kind in gate_kinds and reason != "gate_inventory_owned":
        raise ValueError(f"gate fact {discovery_id} requires gate_inventory_owned rationale")


def _required_string(record: Mapping[str, Any], field: str, label: str) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
