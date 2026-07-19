"""Pure analysis for explicit runtime-anomaly test associations."""

from __future__ import annotations

from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from typing import Any

from .test_catalog_models import InferredTestFact


ASSOCIATION_KINDS = ("direct", "partial", "protective_red", "context_only")
MAPPING_STATES = (
    "mapped_runnable",
    "mapped_non_runnable_or_partial",
    "unmapped",
)
PRIMARY_SUITES = ("pr", "nightly", "release", "hardware_lab")
RUST_SOURCE_KINDS = {"rust_integration_test", "rust_unit_test"}
BOUNDARIES = {
    "association_only": True,
    "creates_invariant_coverage": False,
    "creates_proof": False,
    "executes_faults": False,
}


def analyze_runtime_anomaly_mapping(
    *,
    taxonomy: Mapping[str, Any],
    facts: Sequence[InferredTestFact],
    ignored_tests: Sequence[Mapping[str, Any]] | Mapping[str, Mapping[str, Any]],
    scanner_denominator: int,
) -> dict[str, Any]:
    """Join reviewed mappings to live Rust facts without lexical inference."""

    _validate_scanner_denominator(scanner_denominator, facts)
    facts_by_id = _facts_by_discovery_id(facts)
    ignored_by_id = _ignored_by_discovery_id(ignored_tests)
    classes = _classes_by_id(taxonomy)
    mappings = _mapping_records(taxonomy)

    mapping_rows: list[dict[str, Any]] = []
    mappings_by_class: dict[str, list[dict[str, Any]]] = defaultdict(list)
    mapping_ids: set[str] = set()
    for mapping in mappings:
        mapping_id = _required_string(mapping, "id", "runtime-anomaly mapping")
        if mapping_id in mapping_ids:
            raise ValueError(f"runtime-anomaly taxonomy duplicates mapping id {mapping_id}")
        mapping_ids.add(mapping_id)
        class_id = _required_string(mapping, "class_id", mapping_id)
        anomaly_class = classes.get(class_id)
        if anomaly_class is None:
            raise ValueError(f"{mapping_id} names unknown runtime-anomaly class {class_id}")

        association_kind = _required_string(mapping, "association_kind", mapping_id)
        if association_kind not in ASSOCIATION_KINDS:
            raise ValueError(
                f"{mapping_id} has unsupported association_kind {association_kind!r}"
            )
        discovery_id = _required_string(mapping, "discovery_id", mapping_id)
        matches = facts_by_id.get(discovery_id, [])
        if len(matches) != 1:
            if not matches:
                raise ValueError(f"{mapping_id} does not resolve a scanner fact: {discovery_id}")
            raise ValueError(
                f"{mapping_id} discovery_id {discovery_id} resolves to "
                f"{len(matches)} scanner facts"
            )
        fact = matches[0]
        _validate_mapping_fact_binding(mapping_id, mapping, fact)
        ignored_registry_id = _ignored_registry_id(
            mapping_id=mapping_id,
            fact=fact,
            ignored_by_id=ignored_by_id,
        )
        effectively_runnable = (
            association_kind == "direct" and fact.ignore_state == "not_ignored"
        )
        row = {
            "mapping_id": mapping_id,
            "class_id": class_id,
            "discovery_id": discovery_id,
            "discovery_source_kind": fact.source_kind,
            "path": fact.path,
            "name": fact.name,
            "association_kind": association_kind,
            "injection_mechanism": mapping.get("injection_mechanism"),
            "assertion_summary": mapping.get("assertion_summary"),
            "limitations": mapping.get("limitations"),
            "last_reviewed": mapping.get("last_reviewed"),
            "primary_suite": anomaly_class["primary_suite"],
            "ignore_state": fact.ignore_state,
            "ignored_registry_id": ignored_registry_id,
            "effectively_runnable": effectively_runnable,
        }
        mapping_rows.append(row)
        mappings_by_class[class_id].append(row)

    mapping_rows.sort(key=lambda row: row["mapping_id"])
    class_rows = _class_rows(taxonomy, mappings_by_class)
    gap_rows = [_gap_row(row) for row in class_rows if row["state"] != "mapped_runnable"]
    state_counts = Counter(row["state"] for row in class_rows)
    tier_counts = Counter(row["primary_suite"] for row in class_rows)
    association_counts = Counter(row["association_kind"] for row in mapping_rows)

    return {
        "scope": {
            "mapping_basis": "explicit_taxonomy_mapping_discovery_id_only",
            "scanner_population": "production_rust_test_facts",
            "scanner_denominator": scanner_denominator,
        },
        "boundaries": dict(BOUNDARIES),
        "summary": {
            "taxonomy_classes": len(class_rows),
            "mapping_records": len(mapping_rows),
            "scanner_denominator": scanner_denominator,
            "effectively_runnable_mappings": sum(
                row["effectively_runnable"] for row in mapping_rows
            ),
            "ignored_or_conditional_mappings": sum(
                row["ignore_state"] != "not_ignored" for row in mapping_rows
            ),
            "gap_classes": len(gap_rows),
            "by_state": {state: state_counts[state] for state in MAPPING_STATES},
            "by_primary_suite": {tier: tier_counts[tier] for tier in PRIMARY_SUITES},
            "by_association_kind": {
                kind: association_counts[kind] for kind in ASSOCIATION_KINDS
            },
        },
        "classes": class_rows,
        "mappings": mapping_rows,
        "gap_rows": gap_rows,
    }


def _validate_scanner_denominator(
    scanner_denominator: int,
    facts: Sequence[InferredTestFact],
) -> None:
    if (
        not isinstance(scanner_denominator, int)
        or isinstance(scanner_denominator, bool)
        or scanner_denominator < 0
    ):
        raise ValueError("scanner denominator must be a non-negative integer")
    if scanner_denominator != len(facts):
        raise ValueError(
            f"scanner denominator {scanner_denominator} does not match {len(facts)} facts"
        )


def _facts_by_discovery_id(
    facts: Sequence[InferredTestFact],
) -> dict[str, list[InferredTestFact]]:
    result: dict[str, list[InferredTestFact]] = defaultdict(list)
    for fact in facts:
        if fact.source_kind not in RUST_SOURCE_KINDS:
            raise ValueError(
                f"production Rust scanner facts contain unsupported source kind "
                f"{fact.source_kind!r} for {fact.stable_id}"
            )
        result[fact.stable_id].append(fact)
    return result


def _ignored_by_discovery_id(
    ignored_tests: Sequence[Mapping[str, Any]] | Mapping[str, Mapping[str, Any]],
) -> dict[str, list[Mapping[str, Any]]]:
    records = ignored_tests.values() if isinstance(ignored_tests, Mapping) else ignored_tests
    result: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    for record in records:
        discovery_id = record.get("discovery_id")
        if isinstance(discovery_id, str):
            result[discovery_id].append(record)
    return result


def _classes_by_id(taxonomy: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    classes = taxonomy.get("classes")
    if not isinstance(classes, list):
        raise ValueError("runtime-anomaly taxonomy classes must be a list")
    result: dict[str, Mapping[str, Any]] = {}
    for record in classes:
        if not isinstance(record, Mapping):
            raise ValueError("runtime-anomaly taxonomy class must be an object")
        class_id = _required_string(record, "id", "runtime-anomaly class")
        if class_id in result:
            raise ValueError(f"runtime-anomaly taxonomy duplicates class id {class_id}")
        primary_suite = record.get("primary_suite")
        if primary_suite not in PRIMARY_SUITES:
            raise ValueError(
                f"runtime-anomaly class {class_id} has unsupported primary_suite "
                f"{primary_suite!r}"
            )
        result[class_id] = record
    return result


def _mapping_records(taxonomy: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    mappings = taxonomy.get("mappings")
    if not isinstance(mappings, list):
        raise ValueError("runtime-anomaly taxonomy mappings must be a list")
    if not all(isinstance(record, Mapping) for record in mappings):
        raise ValueError("runtime-anomaly taxonomy mapping must be an object")
    return mappings


def _validate_mapping_fact_binding(
    mapping_id: str,
    mapping: Mapping[str, Any],
    fact: InferredTestFact,
) -> None:
    for field, actual in (
        ("path", fact.path),
        ("name", fact.name),
        ("discovery_source_kind", fact.source_kind),
    ):
        if mapping.get(field) != actual:
            raise ValueError(
                f"{mapping_id} {field} does not match scanner fact: "
                f"taxonomy {mapping.get(field)!r}, scanner {actual!r}"
            )


def _ignored_registry_id(
    *,
    mapping_id: str,
    fact: InferredTestFact,
    ignored_by_id: Mapping[str, list[Mapping[str, Any]]],
) -> str | None:
    matches = ignored_by_id.get(fact.stable_id, [])
    if fact.ignore_state == "not_ignored":
        if matches:
            raise ValueError(
                f"{mapping_id} not_ignored scanner fact still has an ignored-test "
                f"registry record: {fact.stable_id}"
            )
        return None
    if len(matches) != 1:
        if not matches:
            raise ValueError(
                f"{mapping_id} ignored scanner fact requires one ignored-test "
                f"registry record: {fact.stable_id}"
            )
        raise ValueError(
            f"{mapping_id} discovery_id {fact.stable_id} resolves to "
            f"{len(matches)} ignored-test registry records"
        )
    record = matches[0]
    for field, actual in (
        ("path", fact.path),
        ("name", fact.name),
        ("discovery_source_kind", fact.source_kind),
        ("ignore_state", fact.ignore_state),
    ):
        if record.get(field) != actual:
            raise ValueError(
                f"{mapping_id} ignored-test {field} does not match scanner fact: "
                f"registry {record.get(field)!r}, scanner {actual!r}"
            )
    return _required_string(record, "id", f"{mapping_id} ignored-test registry record")


def _class_rows(
    taxonomy: Mapping[str, Any],
    mappings_by_class: Mapping[str, list[dict[str, Any]]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for anomaly_class in taxonomy["classes"]:
        class_id = anomaly_class["id"]
        mappings = sorted(
            mappings_by_class.get(class_id, []),
            key=lambda row: row["mapping_id"],
        )
        runnable = [row["mapping_id"] for row in mappings if row["effectively_runnable"]]
        non_runnable = [
            row["mapping_id"] for row in mappings if not row["effectively_runnable"]
        ]
        if runnable:
            state = "mapped_runnable"
        elif mappings:
            state = "mapped_non_runnable_or_partial"
        else:
            state = "unmapped"
        rows.append(
            {
                "class_id": class_id,
                "title": anomaly_class["title"],
                "primary_suite": anomaly_class["primary_suite"],
                "conditional_suites": list(anomaly_class.get("conditional_suites", [])),
                "state": state,
                "mapping_ids": [row["mapping_id"] for row in mappings],
                "runnable_mapping_ids": runnable,
                "non_runnable_or_partial_mapping_ids": non_runnable,
            }
        )
    return rows


def _gap_row(class_row: Mapping[str, Any]) -> dict[str, Any]:
    reason = (
        "no_explicit_mapping"
        if class_row["state"] == "unmapped"
        else "no_effectively_runnable_direct_mapping"
    )
    return {
        "class_id": class_row["class_id"],
        "title": class_row["title"],
        "primary_suite": class_row["primary_suite"],
        "state": class_row["state"],
        "mapping_ids": list(class_row["mapping_ids"]),
        "reason": reason,
    }


def _required_string(record: Mapping[str, Any], field: str, label: str) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
