"""Reviewed reassignment of one existing catalog test authority."""

from __future__ import annotations

import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from .case_backed_behavior_proof import replace_table_record_fields
from .mapping_catalog_updates import (
    append_invariant_tests,
    build_accepted_catalog_record,
    remove_invariant_tests,
)
from .mapping_review import build_mapping_decision, validate_mapping_decisions
from .mapping_toml import render_decisions_toml
from .runtime_other_behavior_mapping_support import (
    records_by_id,
    require_unique_catalog_provenance,
    string,
    strings,
)


@dataclass(frozen=True)
class CatalogAuthorityReassignment:
    replacements: Mapping[str, bytes]
    test_id: str
    old_invariant_ids: tuple[str, ...]
    new_invariant_ids: tuple[str, ...]


def build_catalog_authority_reassignment(
    *,
    proposal: Mapping[str, Any],
    review_input: Mapping[str, Any],
    decision_document: Mapping[str, Any],
    catalog_source: str,
    invariant_sources: Mapping[str, tuple[str, str]],
    invariants: Mapping[str, Mapping[str, Any]],
    expected_test_id: str,
    expected_old_invariant_ids: Sequence[str],
    reviewed_date: str,
    source_commit: str,
) -> CatalogAuthorityReassignment:
    """Build an atomic reassignment plan after semantic review."""

    if not expected_test_id:
        raise ValueError("catalog authority reassignment requires a test ID")
    old_invariant_ids = strings(
        expected_old_invariant_ids,
        "expected old invariant IDs",
    )
    if not old_invariant_ids:
        raise ValueError("catalog authority reassignment requires old authority")
    catalog = tomllib.loads(catalog_source)
    catalog_tests = records_by_id(catalog.get("tests"), "catalog tests")
    existing_test = catalog_tests.get(expected_test_id)
    if existing_test is None:
        raise ValueError(f"catalog lacks reassigned test {expected_test_id}")
    if existing_test.get("subject_kind") != "generated_test":
        raise ValueError("catalog authority reassignment requires generated test")
    if strings(existing_test.get("invariants"), "catalog invariants") != (
        old_invariant_ids
    ):
        raise ValueError("catalog old invariant authority drifted")
    for field in ("discovery_id", "discovery_source_kind", "path", "name"):
        if proposal.get(field) != existing_test.get(field):
            raise ValueError(f"catalog reassignment proposal {field} drifted")

    new_invariant_ids = strings(
        review_input.get("selected_invariant_ids"),
        "selected new invariant IDs",
    )
    if not new_invariant_ids or set(new_invariant_ids).intersection(
        old_invariant_ids
    ):
        raise ValueError("catalog reassignment requires distinct new authority")
    selected_area = None
    for invariant_id in new_invariant_ids:
        invariant = invariants.get(invariant_id)
        if not isinstance(invariant, Mapping):
            raise ValueError(f"unknown reassignment invariant {invariant_id}")
        area = string(invariant, "area", invariant_id)
        if selected_area is None:
            selected_area = area
        elif selected_area != area:
            raise ValueError("reassignment invariants cross area boundaries")
        if invariant_id not in invariant_sources:
            raise ValueError(f"missing source for invariant {invariant_id}")
    confirmed = review_input.get("confirmed_catalog")
    if not isinstance(confirmed, Mapping) or confirmed.get("area") != selected_area:
        raise ValueError("reassignment catalog area differs from new authority")

    refreshed_decision = build_mapping_decision(
        proposal=proposal,
        review_input=review_input,
    )
    refreshed_record = build_accepted_catalog_record(
        proposal=proposal,
        decision=refreshed_decision,
        invariants=invariants,
        reviewed_date=reviewed_date,
    )
    if refreshed_record.get("id") != expected_test_id:
        raise ValueError("catalog reassignment changed test identity")
    for field in (
        "subject_kind",
        "discovery_id",
        "discovery_source_kind",
        "path",
        "name",
        "evidence_destination",
    ):
        if refreshed_record.get(field) != existing_test.get(field):
            raise ValueError(f"catalog reassignment changed immutable {field}")

    snapshots = records_by_id(
        decision_document.get("proposal_snapshots"),
        "proposal snapshots",
    )
    decisions = records_by_id(
        decision_document.get("decisions"),
        "mapping decisions",
    )
    old_proposal_id = string(
        existing_test,
        "mapping_proposal_id",
        expected_test_id,
    )
    old_decision_id = string(
        existing_test,
        "mapping_decision_id",
        expected_test_id,
    )
    if old_proposal_id not in snapshots or old_decision_id not in decisions:
        raise ValueError("catalog reassignment lacks current reviewed provenance")
    require_unique_catalog_provenance(
        catalog_tests=catalog_tests.values(),
        test_id=expected_test_id,
        proposal_id=old_proposal_id,
        decision_id=old_decision_id,
    )
    new_proposal_id = string(proposal, "id", "reassignment proposal")
    new_decision_id = string(
        refreshed_decision,
        "id",
        "reassignment decision",
    )
    if new_proposal_id != old_proposal_id and new_proposal_id in snapshots:
        raise ValueError("catalog reassignment proposal already exists")
    if new_decision_id != old_decision_id and new_decision_id in decisions:
        raise ValueError("catalog reassignment decision already exists")
    del snapshots[old_proposal_id]
    del decisions[old_decision_id]
    snapshots[new_proposal_id] = proposal
    decisions[new_decision_id] = refreshed_decision
    final_decision_document = {
        **decision_document,
        "source_commit": source_commit,
        "proposal_snapshots": [row for _, row in sorted(snapshots.items())],
        "decisions": [row for _, row in sorted(decisions.items())],
    }
    decision_failures = validate_mapping_decisions(
        decisions=list(decisions.values()),
        proposals=snapshots,
        invariants=invariants,
        candidate_generator_id="test-catalog-proposals",
    )
    if decision_failures:
        raise ValueError("; ".join(decision_failures))

    catalog_after = replace_table_record_fields(
        catalog_source,
        table="tests",
        updates={
            expected_test_id: {
                field: value
                for field, value in refreshed_record.items()
                if field != "id" and existing_test.get(field) != value
            }
        },
    )
    replacements: dict[str, bytes] = {
        "verification/test-catalog.toml": catalog_after.encode(),
        "verification/test-catalog-mapping-decisions.toml": render_decisions_toml(
            final_decision_document
        ).encode(),
    }
    for invariant_id in old_invariant_ids:
        source_entry = invariant_sources.get(invariant_id)
        if source_entry is None:
            raise ValueError(f"missing source for old invariant {invariant_id}")
        path, source = source_entry
        replacements[path] = remove_invariant_tests(
            source, [expected_test_id]
        ).encode()
    for invariant_id in new_invariant_ids:
        path, source = invariant_sources[invariant_id]
        document = tomllib.loads(source)
        current_tests = strings(document.get("tests"), f"{invariant_id} tests")
        if expected_test_id in current_tests:
            raise ValueError("new invariant already contains reassigned test")
        replacements[path] = append_invariant_tests(
            source, [expected_test_id]
        ).encode()

    projected_catalog = records_by_id(
        tomllib.loads(catalog_after).get("tests"),
        "projected catalog tests",
    )[expected_test_id]
    if strings(projected_catalog.get("invariants"), "projected invariants") != (
        new_invariant_ids
    ):
        raise ValueError("projected catalog authority drifted")
    return CatalogAuthorityReassignment(
        replacements=replacements,
        test_id=expected_test_id,
        old_invariant_ids=tuple(old_invariant_ids),
        new_invariant_ids=tuple(new_invariant_ids),
    )
