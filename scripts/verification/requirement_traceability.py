"""Explicit forward, reverse, and orphan traceability analysis."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any


def analyze_requirement_traceability(
    *,
    invariants: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    suites: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
    spec_sources: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    """Build traceability using only explicit metadata identifiers."""

    evidence_by_invariant: dict[str, set[str]] = {}
    evidence_by_test: dict[str, set[str]] = {}
    linked_evidence_ids: set[str] = set()
    for evidence_id, record in sorted(evidence.items()):
        _require_identity(evidence_id, record, "evidence")
        invariant_ids = _string_list(record.get("linked_invariants", []), evidence_id, "linked_invariants")
        test_ids = _string_list(record.get("linked_tests", []), evidence_id, "linked_tests")
        gap_ids = _string_list(record.get("linked_spec_gaps", []), evidence_id, "linked_spec_gaps")
        if invariant_ids or test_ids or gap_ids:
            linked_evidence_ids.add(evidence_id)
        for invariant_id in invariant_ids:
            if invariant_id not in invariants:
                raise ValueError(f"evidence {evidence_id} references unknown invariant {invariant_id}")
            evidence_by_invariant.setdefault(invariant_id, set()).add(evidence_id)
        for test_id in test_ids:
            if test_id not in tests:
                raise ValueError(f"evidence {evidence_id} references unknown test {test_id}")
            evidence_by_test.setdefault(test_id, set()).add(evidence_id)

    forward: list[dict[str, Any]] = []
    referenced_source_ids: set[str] = set()
    linked_test_ids: set[str] = set()
    referenced_suite_ids: set[str] = set()
    traced_evidence_ids: set[str] = set()
    for invariant_id, invariant in sorted(invariants.items()):
        _require_identity(invariant_id, invariant, "invariant")
        spec = invariant.get("spec")
        oracle = invariant.get("oracle")
        if not isinstance(spec, Mapping) or not isinstance(oracle, Mapping):
            raise ValueError(f"invariant {invariant_id} requires spec and oracle tables")

        source_ids = _ordered_unique(
            [
                *_string_list(spec.get("source_refs", []), invariant_id, "spec.source_refs"),
                *_oracle_source_ids(oracle.get("ref"), spec_sources),
            ]
        )
        for source_id in source_ids:
            if source_id not in spec_sources:
                raise ValueError(f"invariant {invariant_id} references unknown spec source {source_id}")
        referenced_source_ids.update(source_ids)

        invariant_test_ids = _string_list(invariant.get("tests", []), invariant_id, "tests")
        catalog_backlinks = [
            test_id
            for test_id, test in sorted(tests.items())
            if invariant_id in _string_list(test.get("invariants", []), test_id, "invariants")
        ]
        test_ids = _ordered_unique([*invariant_test_ids, *catalog_backlinks])
        suite_ids = _string_list(invariant.get("gates", []), invariant_id, "gates")
        direct_evidence_ids = _string_list(
            invariant.get("evidence_refs", []), invariant_id, "evidence_refs"
        )
        for test_id in test_ids:
            test = tests.get(test_id)
            if test is None:
                raise ValueError(f"invariant {invariant_id} references unknown test {test_id}")
            if test_id in invariant_test_ids and invariant_id not in _string_list(
                test.get("invariants", []), test_id, "invariants"
            ):
                raise ValueError(f"test {test_id} does not backlink invariant {invariant_id}")
            suite_ids.extend(_string_list(test.get("suite_tiers", []), test_id, "suite_tiers"))
        suite_ids = _ordered_unique(suite_ids)
        for suite_id in suite_ids:
            if suite_id not in suites:
                raise ValueError(f"invariant {invariant_id} references unknown suite {suite_id}")
        for evidence_id in direct_evidence_ids:
            if evidence_id not in evidence:
                raise ValueError(f"invariant {invariant_id} references unknown evidence {evidence_id}")

        evidence_ids = set(direct_evidence_ids)
        evidence_ids.update(evidence_by_invariant.get(invariant_id, set()))
        for test_id in test_ids:
            evidence_ids.update(evidence_by_test.get(test_id, set()))
        claim_ids = [
            source_id
            for source_id in source_ids
            if spec_sources[source_id].get("authority") == "public_claim"
        ]
        linked_test_ids.update(test_ids)
        referenced_suite_ids.update(suite_ids)
        traced_evidence_ids.update(evidence_ids)
        missing_links = sorted(
            label
            for label, values in (
                ("spec_source", source_ids),
                ("test", test_ids),
                ("suite", suite_ids),
                ("evidence", evidence_ids),
            )
            if not values
        )
        forward.append(
            {
                "invariant_id": invariant_id,
                "spec_source_ids": source_ids,
                "test_ids": test_ids,
                "suite_ids": suite_ids,
                "evidence_ids": sorted(evidence_ids),
                "public_claim_ids": claim_ids,
                "missing_links": missing_links,
                "chain_complete_to_evidence": not missing_links,
            }
        )

    reverse_claims: list[dict[str, Any]] = []
    for claim_id, claim in sorted(spec_sources.items()):
        if claim.get("authority") != "public_claim":
            continue
        selected = [row for row in forward if claim_id in row["public_claim_ids"]]
        invariant_ids = [row["invariant_id"] for row in selected]
        reverse_claims.append(
            {
                "public_claim_id": claim_id,
                "invariant_ids": invariant_ids,
                "test_ids": sorted({item for row in selected for item in row["test_ids"]}),
                "suite_ids": sorted({item for row in selected for item in row["suite_ids"]}),
                "evidence_ids": sorted({item for row in selected for item in row["evidence_ids"]}),
                "binding_state": "linked" if invariant_ids else "orphan",
            }
        )

    orphan_source_ids = sorted(set(spec_sources) - referenced_source_ids)
    orphan_test_ids = sorted(
        test_id
        for test_id, record in tests.items()
        if not _string_list(record.get("invariants", []), test_id, "invariants")
    )
    orphan_invariant_ids = sorted(
        row["invariant_id"]
        for row in forward
        if not row["spec_source_ids"] and not row["test_ids"]
    )
    orphan_public_claim_ids = [
        row["public_claim_id"] for row in reverse_claims if row["binding_state"] == "orphan"
    ]
    orphan_evidence_ids = sorted(set(evidence) - linked_evidence_ids)
    incomplete = [row for row in forward if not row["chain_complete_to_evidence"]]
    return {
        "forward_traceability": forward,
        "reverse_public_claim_traceability": reverse_claims,
        "orphans": {
            "spec_source_ids": orphan_source_ids,
            "test_ids": orphan_test_ids,
            "invariant_ids": orphan_invariant_ids,
            "public_claim_ids": orphan_public_claim_ids,
            "evidence_ids": orphan_evidence_ids,
        },
        "incomplete_chains": incomplete,
        "traceability_summary": {
            "forward_invariants": len(forward),
            "complete_to_evidence": len(forward) - len(incomplete),
            "incomplete_to_evidence": len(incomplete),
            "reverse_public_claims": len(reverse_claims),
            "linked_public_claims": len(reverse_claims) - len(orphan_public_claim_ids),
            "orphan_spec_sources": len(orphan_source_ids),
            "orphan_tests": len(orphan_test_ids),
            "orphan_invariants": len(orphan_invariant_ids),
            "orphan_public_claims": len(orphan_public_claim_ids),
            "orphan_evidence": len(orphan_evidence_ids),
            "referenced_suites": len(referenced_suite_ids),
            "trace_linked_tests": len(linked_test_ids),
            "trace_linked_evidence": len(traced_evidence_ids),
        },
    }


def _oracle_source_ids(value: Any, spec_sources: Mapping[str, Mapping[str, Any]]) -> list[str]:
    if not isinstance(value, str) or not value:
        raise ValueError("oracle.ref must be a non-empty string")
    source_id = value.split("#", 1)[0]
    return [source_id] if source_id in spec_sources else []


def _require_identity(key: str, record: Mapping[str, Any], kind: str) -> None:
    if record.get("id") != key:
        raise ValueError(f"{kind} index key {key!r} does not match record id")


def _string_list(value: Any, owner_id: str, field: str) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        raise ValueError(f"{owner_id} {field} must be a string array")
    if len(value) != len(set(value)):
        raise ValueError(f"{owner_id} {field} must not contain duplicates")
    return list(value)


def _ordered_unique(values: list[str]) -> list[str]:
    return list(dict.fromkeys(values))
