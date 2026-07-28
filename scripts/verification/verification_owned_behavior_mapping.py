"""Compile and apply the Phase 18 verification-owned mapping transaction."""

from __future__ import annotations

import argparse
import hashlib
import json
import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .case_backed_behavior_proof import replace_table_record_fields
from .frozen_catalog_gaps import analyze_frozen_catalog_gaps
from .mapping_catalog_updates import (
    append_invariant_tests,
    build_accepted_catalog_record,
    reconcile_invariant_catalog_state,
    remove_invariant_missing_items,
)
from .mapping_proposals import (
    build_mapping_proposals,
    validate_mapping_quarantine,
)
from .mapping_review import build_mapping_decision, validate_mapping_decisions
from .mapping_review_batch import build_review_batch
from .mapping_toml import render_decisions_toml
from .mapping_transaction import apply_mapping_transaction
from .runtime_other_behavior_mapping import _individual_commands
from .runtime_other_behavior_mapping_review import (
    new_review_input,
    refreshed_review_input,
    selected_behavior_refs,
    selected_invariant_ids,
)
from .runtime_other_behavior_mapping_support import (
    candidate,
    inventories,
    json_document,
    load_invariant_documents,
    load_invariants,
    records,
    records_by_id,
    require_unique_catalog_provenance,
    string,
    strings,
    toml_document,
    toml_source,
)
from .verification_owned_behavior_campaign import (
    ARTIFACT_ROOT,
    DEFAULT_GAP_REPORT,
    DEFAULT_SCAN_REPORT,
)
from .verification_owned_behavior_proof import build_assignment_plan


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
PROPOSALS_PATH = Path("verification/test-catalog-proposals.toml")
DECISIONS_PATH = Path("verification/test-catalog-mapping-decisions.toml")
LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "verification-owned-11-semantic-lock.json"
)
CAMPAIGN_PLAN_PATH = ARTIFACT_ROOT / "campaign-plan.json"
CAMPAIGN_PATH = ARTIFACT_ROOT / "campaign.json"
ASSERTIONS_PATH = ARTIFACT_ROOT / "assertions.json"
SOURCE_COMMIT = "1338cd7a48e690c43b65c7239216f651a97178b3"
REVIEWED_DATE = "2026-07-29"
REVIEWED_TIMESTAMP = "2026-07-28T23:30:00Z"
REFRESHED_TEST_IDS = frozenset(
    {
        "TEST_AUTO_1A7BDF97311D84EE779B",
        "TEST_AUTO_247AAB98DD35F057E1EA",
        "TEST_AUTO_C932AA981C465EDBACC6",
    }
)
COMPLETED_MISSING_ITEMS = {
    "VERIFICATION_CASE_ARTIFACT_HELPER_001": (
        "cataloged_tests",
        "focused_execution_evidence",
        "resolved_generated_input_oracles",
    ),
    "VERIFICATION_IEC_TABLE_INVENTORY_001": (
        "cataloged_inventory_test",
        "focused_execution_evidence",
    ),
}


@dataclass(frozen=True)
class VerificationOwnedMappingCompilation:
    summary: dict[str, int]
    replacements: dict[str, bytes]


def compile_verification_owned_mapping(
    *,
    root: Path,
) -> VerificationOwnedMappingCompilation:
    """Return the complete unapplied verification-owned transaction."""

    root = root.resolve()
    if (root / LOCK_PATH).is_file():
        return _reconstruct_applied(root)

    assignment = build_assignment_plan(
        root=root,
        gap_report=root / DEFAULT_GAP_REPORT,
        scan_report=root / DEFAULT_SCAN_REPORT,
    )
    campaign_plan = json_document(root / CAMPAIGN_PLAN_PATH)
    campaign = json_document(root / CAMPAIGN_PATH)
    assertion_inventories = inventories(root / ASSERTIONS_PATH)
    scan = json_document(root / DEFAULT_SCAN_REPORT)
    _validate_frozen_inputs(
        assignment=assignment,
        campaign_plan=campaign_plan,
        campaign=campaign,
        assertion_inventories=assertion_inventories,
    )

    catalog_source = (root / CATALOG_PATH).read_text()
    catalog = toml_source(catalog_source, CATALOG_PATH.as_posix())
    denominator = toml_document(root / DENOMINATOR_PATH)
    proposal_document = toml_document(root / PROPOSALS_PATH)
    decision_document = toml_document(root / DECISIONS_PATH)
    if proposal_document.get("proposals") != []:
        raise ValueError(
            "verification-owned mapping requires an empty proposal queue"
        )
    invariant_sources, invariant_documents = load_invariants(root)
    facts = _live_scan_facts(scan)
    assignments = [
        dict(row)
        for row in records(assignment.get("assignments"), "assignments")
    ]
    denominator = _register_new_denominator_facts(
        denominator=denominator,
        assignments=assignments,
        facts=facts,
    )
    trace_records = _trace_records(
        campaign_plan=campaign_plan,
        campaign=campaign,
        assignments=assignments,
    )

    new_assignments = [
        row for row in assignments if row.get("resolution") == "new_catalog"
    ]
    if len(new_assignments) != 12:
        raise ValueError(
            "verification-owned new-catalog population drifted"
        )
    new_discovery_ids = {
        string(row, "discovery_id", "new assignment")
        for row in new_assignments
    }
    new_proposals = build_mapping_proposals(
        root=root,
        candidates=[
            candidate(
                discovery_id=string(
                    row,
                    "discovery_id",
                    "new assignment",
                ),
                invariant_ids=strings(
                    row.get("invariant_ids"),
                    "new assignment invariant IDs",
                ),
            )
            for row in new_assignments
        ],
        trace_records={
            discovery_id: trace_records[discovery_id]
            for discovery_id in new_discovery_ids
        },
        facts={
            discovery_id: facts[discovery_id]
            for discovery_id in new_discovery_ids
        },
        invariants=invariant_documents,
        assertion_inventories={
            discovery_id: assertion_inventories[discovery_id]
            for discovery_id in new_discovery_ids
        },
    )
    quarantine_failures = validate_mapping_quarantine(
        root=root,
        proposals=new_proposals,
        facts=facts,
        tests=records_by_id(catalog.get("tests"), "catalog tests"),
        denominator_reviews={
            string(row, "discovery_id", "denominator review"): row
            for row in records(
                denominator.get("reviews"),
                "denominator reviews",
            )
        },
        invariants=invariant_documents,
    )
    if quarantine_failures:
        raise ValueError("; ".join(quarantine_failures))

    proposal_document = {
        **proposal_document,
        "source_commit": SOURCE_COMMIT,
        "trace_digest": _document_digest(campaign),
        "candidate_report_digest": _document_digest(assignment),
        "last_generated": REVIEWED_TIMESTAMP,
        "proposals": new_proposals,
    }
    assignment_by_discovery = {
        string(row, "discovery_id", "new assignment"): row
        for row in new_assignments
    }
    new_review_inputs = [
        new_review_input(
            proposal=proposal,
            assignment=assignment_by_discovery[
                string(proposal, "discovery_id", "new proposal")
            ],
            invariants=invariant_documents,
        )
        for proposal in new_proposals
    ]
    touched_new_invariants = {
        invariant_id
        for row in new_assignments
        for invariant_id in strings(
            row.get("invariant_ids"),
            "new assignment invariant IDs",
        )
    }
    new_batch = build_review_batch(
        proposal_document=proposal_document,
        decision_document=decision_document,
        review_inputs=new_review_inputs,
        catalog_source=catalog_source,
        denominator_document=denominator,
        invariant_sources={
            invariant_id: invariant_sources[invariant_id]
            for invariant_id in sorted(touched_new_invariants)
        },
        invariants=invariant_documents,
        reviewed_date=REVIEWED_DATE,
        source_commit=SOURCE_COMMIT,
    )
    if len(new_batch.accepted_test_ids) != 12:
        raise ValueError(
            "verification-owned review did not accept all promotions"
        )

    replacements = dict(new_batch.replacements)
    catalog_source = replacements[CATALOG_PATH.as_posix()].decode()
    catalog = toml_source(
        catalog_source,
        "verification-owned promoted catalog",
    )
    denominator = toml_source(
        replacements[DENOMINATOR_PATH.as_posix()].decode(),
        "verification-owned denominator",
    )
    proposal_document = toml_source(
        replacements[PROPOSALS_PATH.as_posix()].decode(),
        "verification-owned proposal queue",
    )
    decision_document = toml_source(
        replacements[DECISIONS_PATH.as_posix()].decode(),
        "verification-owned decisions",
    )

    existing_assignments = {
        string(row, "catalog_test_id", "existing assignment"): row
        for row in assignments
        if isinstance(row.get("catalog_test_id"), str)
    }
    if set(existing_assignments) != REFRESHED_TEST_IDS:
        raise ValueError(
            "verification-owned provenance refresh population drifted"
        )
    catalog_by_id = records_by_id(catalog.get("tests"), "catalog tests")
    refreshed_proposals = build_mapping_proposals(
        root=root,
        candidates=[
            candidate(
                discovery_id=string(
                    catalog_by_id[test_id],
                    "discovery_id",
                    test_id,
                ),
                invariant_ids=selected_invariant_ids(
                    test=catalog_by_id[test_id],
                    assignment=existing_assignments[test_id],
                ),
            )
            for test_id in sorted(REFRESHED_TEST_IDS)
        ],
        trace_records={
            string(catalog_by_id[test_id], "discovery_id", test_id):
            trace_records[
                string(catalog_by_id[test_id], "discovery_id", test_id)
            ]
            for test_id in sorted(REFRESHED_TEST_IDS)
        },
        facts={
            string(catalog_by_id[test_id], "discovery_id", test_id): facts[
                string(catalog_by_id[test_id], "discovery_id", test_id)
            ]
            for test_id in sorted(REFRESHED_TEST_IDS)
        },
        invariants=invariant_documents,
        assertion_inventories={
            string(catalog_by_id[test_id], "discovery_id", test_id):
            assertion_inventories[
                string(catalog_by_id[test_id], "discovery_id", test_id)
            ]
            for test_id in sorted(REFRESHED_TEST_IDS)
        },
    )
    if len(refreshed_proposals) != 3:
        raise ValueError(
            "verification-owned provenance refresh did not produce 3 proposals"
        )
    refreshed_by_discovery = {
        string(proposal, "discovery_id", "refreshed proposal"): proposal
        for proposal in refreshed_proposals
    }
    snapshots = records_by_id(
        decision_document.get("proposal_snapshots"),
        "proposal snapshots",
    )
    decisions = records_by_id(
        decision_document.get("decisions"),
        "mapping decisions",
    )
    catalog_updates: dict[str, dict[str, Any]] = {}
    refreshed_decisions = []
    for test_id in sorted(REFRESHED_TEST_IDS):
        test = catalog_by_id[test_id]
        assignment_row = existing_assignments[test_id]
        discovery_id = string(test, "discovery_id", test_id)
        proposal = refreshed_by_discovery[discovery_id]
        old_proposal_id = string(
            test,
            "mapping_proposal_id",
            test_id,
        )
        old_decision_id = string(
            test,
            "mapping_decision_id",
            test_id,
        )
        if old_proposal_id not in snapshots or old_decision_id not in decisions:
            raise ValueError(f"{test_id} lacks accepted provenance")
        require_unique_catalog_provenance(
            catalog_tests=catalog_by_id.values(),
            test_id=test_id,
            proposal_id=old_proposal_id,
            decision_id=old_decision_id,
        )
        del snapshots[old_proposal_id]
        del decisions[old_decision_id]
        snapshots[string(proposal, "id", "refreshed proposal")] = proposal
        invariant_ids = selected_invariant_ids(
            test=test,
            assignment=assignment_row,
        )
        behavior_refs = selected_behavior_refs(
            test=test,
            assignment=assignment_row,
        )
        decision = build_mapping_decision(
            proposal=proposal,
            review_input=refreshed_review_input(
                proposal=proposal,
                test=test,
                invariant_ids=invariant_ids,
                behavior_refs=behavior_refs,
            ),
        )
        decisions[string(decision, "id", "refreshed decision")] = decision
        refreshed_decisions.append(decision)
        refreshed_record = build_accepted_catalog_record(
            proposal=proposal,
            decision=decision,
            invariants=invariant_documents,
            reviewed_date=REVIEWED_DATE,
        )
        if refreshed_record.get("id") != test_id:
            raise ValueError(
                f"{test_id} provenance refresh changed catalog identity"
            )
        catalog_updates[test_id] = {
            field: value
            for field, value in refreshed_record.items()
            if field != "id" and test.get(field) != value
        }
    catalog_source = replace_table_record_fields(
        catalog_source,
        table="tests",
        updates=catalog_updates,
    )
    catalog = toml_source(
        catalog_source,
        "verification-owned final catalog",
    )
    decision_document = {
        **decision_document,
        "source_commit": SOURCE_COMMIT,
        "proposal_snapshots": [
            row for _, row in sorted(snapshots.items())
        ],
        "decisions": [row for _, row in sorted(decisions.items())],
    }
    decision_failures = validate_mapping_decisions(
        decisions=list(decisions.values()),
        proposals=snapshots,
        invariants=invariant_documents,
        candidate_generator_id=str(
            proposal_document.get("generator", "test-catalog-proposals")
        ),
    )
    if decision_failures:
        raise ValueError("; ".join(decision_failures))
    replacements[CATALOG_PATH.as_posix()] = catalog_source.encode()
    replacements[DECISIONS_PATH.as_posix()] = render_decisions_toml(
        decision_document
    ).encode()

    final_invariant_sources = {
        invariant_id: source
        for invariant_id, (_path, source) in invariant_sources.items()
    }
    for invariant_id, (path, _source) in invariant_sources.items():
        replacement = replacements.get(path)
        if replacement is not None:
            final_invariant_sources[invariant_id] = replacement.decode()
    test_links = _assignment_test_links(assignments)
    touched_invariants = sorted(test_links)
    if touched_invariants != sorted(COMPLETED_MISSING_ITEMS):
        raise ValueError(
            "verification-owned touched-invariant population drifted"
        )
    for invariant_id in touched_invariants:
        source = final_invariant_sources[invariant_id]
        document = toml_source(source, invariant_id)
        current = set(
            strings(document.get("tests"), f"{invariant_id} tests")
        )
        additions = sorted(test_links[invariant_id] - current)
        if additions:
            source = append_invariant_tests(source, additions)
        source = reconcile_invariant_catalog_state(
            source,
            invariant_id=invariant_id,
            catalog_source=catalog_source,
        )
        source = remove_invariant_missing_items(
            source,
            COMPLETED_MISSING_ITEMS[invariant_id],
        )
        final_invariant_sources[invariant_id] = source
        path = invariant_sources[invariant_id][0]
        if source.encode() != (root / path).read_bytes():
            replacements[path] = source.encode()

    final_invariants = [
        (
            invariant_sources[invariant_id][0],
            toml_source(source, invariant_id),
        )
        for invariant_id, source in sorted(final_invariant_sources.items())
    ]
    gap_analysis = analyze_frozen_catalog_gaps(
        denominator=denominator,
        catalog=catalog,
        invariants=final_invariants,
    )
    gap_summary = gap_analysis["summary"]
    if gap_summary["confirmed_missing_tests"] != 27:
        raise ValueError(
            "verification-owned transaction did not leave 27 behaviors"
        )
    if gap_summary["missing_spec_authority_tests"] != 0:
        raise ValueError(
            "verification-owned transaction reopened authority debt"
        )
    if (
        gap_summary["frozen_live_tests"] != 4401
        or gap_summary["catalog_mapped"] != 3933
        or gap_summary["reviewed_nonmapping"] != 468
    ):
        raise ValueError(
            "verification-owned denominator equations drifted"
        )

    summary = {
        "behavior_demand_count": 11,
        "campaign_test_count": 15,
        "existing_catalog_test_count": 3,
        "new_catalog_test_count": 12,
        "updated_mapping_provenance_count": 3,
        "confirmed_missing_test_count": 27,
        "touched_invariant_count": len(touched_invariants),
        "catalog_mapped_count": 3933,
        "reviewed_nonmapping_count": 468,
    }
    assertion_reviews = [
        _assertion_review(
            assignment=row,
            inventory=assertion_inventories[
                string(row, "discovery_id", "assignment")
            ],
        )
        for row in assignments
    ]
    lock = {
        "schema_version": 1,
        "id": "VERIFICATION_OWNED_11_SEMANTIC_LOCK",
        "source_commit": SOURCE_COMMIT,
        "reviewed_date": REVIEWED_DATE,
        "summary": summary,
        "assignments": assignments,
        "assertion_reviews": sorted(
            assertion_reviews,
            key=lambda row: row["discovery_id"],
        ),
        "refreshed_catalog_test_ids": sorted(REFRESHED_TEST_IDS),
        "touched_invariant_paths": sorted(
            invariant_sources[invariant_id][0]
            for invariant_id in touched_invariants
        ),
        "evidence_digests": {
            path.as_posix(): _digest(root / path)
            for path in (
                CAMPAIGN_PLAN_PATH,
                CAMPAIGN_PATH,
                ASSERTIONS_PATH,
            )
        },
    }
    replacements[LOCK_PATH.as_posix()] = (
        json.dumps(lock, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode()
    return VerificationOwnedMappingCompilation(
        summary=summary,
        replacements=dict(sorted(replacements.items())),
    )


def apply_verification_owned_mapping(
    *,
    root: Path,
) -> VerificationOwnedMappingCompilation:
    """Apply the reviewed transaction atomically and validate equations."""

    root = root.resolve()
    compilation = compile_verification_owned_mapping(root=root)
    if (root / LOCK_PATH).is_file():
        return compilation
    expected_originals = {
        path: (root / path).read_bytes() if (root / path).exists() else None
        for path in compilation.replacements
    }
    apply_mapping_transaction(
        root=root,
        replacements=compilation.replacements,
        expected_originals=expected_originals,
        validate=lambda: _validate_applied(root),
    )
    return _reconstruct_applied(root)


def _validate_frozen_inputs(
    *,
    assignment: Mapping[str, Any],
    campaign_plan: Mapping[str, Any],
    campaign: Mapping[str, Any],
    assertion_inventories: Mapping[str, Mapping[str, Any]],
) -> None:
    expected = {
        "demand_count": 11,
        "route_name_count": 15,
        "assigned_test_count": 15,
        "existing_catalog_test_count": 3,
        "new_catalog_test_count": 12,
        "assignment_link_count": 17,
        "expected_post_transaction_missing_tests": 27,
    }
    for field, value in expected.items():
        if assignment.get(field) != value:
            raise ValueError(
                f"verification-owned assignment {field} drifted"
            )
    if assignment.get("unresolved") != []:
        raise ValueError(
            "verification-owned assignment contains unresolved routes"
        )
    if (
        campaign_plan.get("basis")
        != "p18_verification_owned_exact_behavior_assignment_11"
        or campaign_plan.get("behavior_count") != 11
        or campaign_plan.get("test_count") != 15
        or campaign_plan.get("group_count") != 2
        or campaign.get("source_commit") != SOURCE_COMMIT
        or campaign.get("test_count") != 15
        or campaign.get("passed_count") != 15
        or campaign.get("all_passed") is not True
        or campaign.get("failures") != []
    ):
        raise ValueError(
            "verification-owned campaign is not the exact green set"
        )
    selected_ids = {
        string(row, "discovery_id", "verification-owned assignment")
        for row in records(assignment.get("assignments"), "assignments")
    }
    if set(assertion_inventories) != selected_ids:
        raise ValueError(
            "verification-owned assertion inventory population drifted"
        )
    unresolved = {
        discovery_id: strings(
            inventory.get("unresolved_assertion_helpers", []),
            f"{discovery_id} unresolved helpers",
        )
        for discovery_id, inventory in assertion_inventories.items()
        if inventory.get("unresolved_assertion_helpers")
    }
    if unresolved:
        raise ValueError(
            f"verification-owned assertion oracles remain unresolved: "
            f"{sorted(unresolved)}"
        )


def _trace_records(
    *,
    campaign_plan: Mapping[str, Any],
    campaign: Mapping[str, Any],
    assignments: Sequence[Mapping[str, Any]],
) -> dict[str, dict[str, Any]]:
    durations = {
        string(row, "test_id", "campaign result"): int(row["duration_ms"])
        for row in records(campaign.get("results"), "campaign results")
    }
    commands = _individual_commands(campaign_plan)
    result = {}
    for assignment in assignments:
        discovery_id = string(assignment, "discovery_id", "assignment")
        test_id = assignment.get("catalog_test_id")
        campaign_test_id = (
            test_id if isinstance(test_id, str) else discovery_id
        )
        if campaign_test_id not in durations or campaign_test_id not in commands:
            raise ValueError(
                f"{discovery_id} lacks exact verification-owned execution"
            )
        result[discovery_id] = {
            "discovery_id": discovery_id,
            "trace_status": "observed",
            "command": commands[campaign_test_id],
            "command_authority": "exact",
            "duration_ms": durations[campaign_test_id],
        }
    return result


def _live_scan_facts(
    scan: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    facts = {}
    for row in records(scan.get("inferred_facts"), "scanner facts"):
        discovery_id = string(row, "stable_id", "scanner fact")
        if discovery_id in facts:
            raise ValueError(
                f"verification-owned scanner duplicates {discovery_id}"
            )
        facts[discovery_id] = {
            "discovery_id": discovery_id,
            "discovery_source_kind": string(
                row,
                "source_kind",
                discovery_id,
            ),
            "path": string(row, "path", discovery_id),
            "name": string(row, "name", discovery_id),
            "ignore_state": string(row, "ignore_state", discovery_id),
            "command_hint": row.get("command_hint"),
        }
    if len(facts) != 4401:
        raise ValueError(
            f"verification-owned scanner fact count drifted: {len(facts)}"
        )
    return facts


def _register_new_denominator_facts(
    *,
    denominator: Mapping[str, Any],
    assignments: Sequence[Mapping[str, Any]],
    facts: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    registered = {
        string(row, "discovery_id", "denominator review")
        for row in records(
            denominator.get("reviews"),
            "denominator reviews",
        )
    }
    absent = {
        string(row, "discovery_id", "assignment")
        for row in assignments
        if row.get("denominator_disposition") is None
    }
    if len(absent) != 2 or absent & registered:
        raise ValueError(
            "verification-owned new denominator population drifted"
        )
    reviews = [
        dict(row)
        for row in records(
            denominator.get("reviews"),
            "denominator reviews",
        )
    ]
    for discovery_id in sorted(absent):
        fact = facts[discovery_id]
        reviews.append(
            {
                "discovery_id": discovery_id,
                "discovery_source_kind": string(
                    fact,
                    "discovery_source_kind",
                    discovery_id,
                ),
                "path": string(fact, "path", discovery_id),
                "name": string(fact, "name", discovery_id),
                "ignore_state": string(
                    fact,
                    "ignore_state",
                    discovery_id,
                ),
                "disposition": "reviewed_nonmapping",
                "reason_code": "no_reviewed_spec_or_invariant_binding",
                "last_reviewed": REVIEWED_DATE,
            }
        )
    return {
        **denominator,
        "last_reviewed": REVIEWED_DATE,
        "reviews": sorted(
            reviews,
            key=lambda row: str(row["discovery_id"]),
        ),
    }


def _assignment_test_links(
    assignments: Sequence[Mapping[str, Any]],
) -> dict[str, set[str]]:
    links: dict[str, set[str]] = {}
    for assignment in assignments:
        test_id = assignment.get("catalog_test_id")
        if not isinstance(test_id, str):
            test_id = "TEST_AUTO_" + string(
                assignment,
                "discovery_id",
                "assignment",
            ).removeprefix("DISC_")
        for invariant_id in strings(
            assignment.get("invariant_ids"),
            "assignment invariant IDs",
        ):
            links.setdefault(invariant_id, set()).add(test_id)
    return links


def _assertion_review(
    *,
    assignment: Mapping[str, Any],
    inventory: Mapping[str, Any],
) -> dict[str, Any]:
    discovery_id = string(assignment, "discovery_id", "assignment")
    assertion_rows = records(
        inventory.get("assertions"),
        f"{discovery_id} assertions",
    )
    oracle_ids = [
        string(row, "assertion_id", f"{discovery_id} assertion")
        for row in assertion_rows
        if (
            str(row.get("kind", "")).startswith("macro:assert")
            or row.get("kind") == "method:expect_err"
        )
    ]
    if not oracle_ids:
        raise ValueError(f"{discovery_id} has no direct reviewed oracle")
    return {
        "discovery_id": discovery_id,
        "catalog_test_id": assignment.get("catalog_test_id"),
        "behavior_refs": strings(
            assignment.get("behavior_refs"),
            f"{discovery_id} behavior refs",
        ),
        "inventory_digest": string(
            inventory,
            "inventory_digest",
            discovery_id,
        ),
        "oracle_assertion_ids": sorted(oracle_ids),
        "unresolved_assertion_helpers": strings(
            inventory.get("unresolved_assertion_helpers", []),
            f"{discovery_id} unresolved helpers",
        ),
    }


def _reconstruct_applied(
    root: Path,
) -> VerificationOwnedMappingCompilation:
    failures = _validate_applied(root)
    if failures:
        raise ValueError("; ".join(failures))
    lock = json_document(root / LOCK_PATH)
    raw_summary = lock.get("summary")
    if not isinstance(raw_summary, Mapping):
        raise ValueError("verification-owned semantic lock lacks summary")
    summary = {
        str(key): int(value)
        for key, value in raw_summary.items()
        if isinstance(value, int) and not isinstance(value, bool)
    }
    if len(summary) != len(raw_summary):
        raise ValueError(
            "verification-owned semantic lock summary is invalid"
        )
    paths = {
        CATALOG_PATH.as_posix(),
        DENOMINATOR_PATH.as_posix(),
        PROPOSALS_PATH.as_posix(),
        DECISIONS_PATH.as_posix(),
        LOCK_PATH.as_posix(),
        *strings(
            lock.get("touched_invariant_paths"),
            "touched invariant paths",
        ),
    }
    return VerificationOwnedMappingCompilation(
        summary=summary,
        replacements={
            path: (root / path).read_bytes() for path in sorted(paths)
        },
    )


def _validate_applied(root: Path) -> list[str]:
    failures: list[str] = []
    try:
        lock = json_document(root / LOCK_PATH)
        catalog = toml_document(root / CATALOG_PATH)
        denominator = toml_document(root / DENOMINATOR_PATH)
        proposals = toml_document(root / PROPOSALS_PATH)
        tests = records_by_id(catalog.get("tests"), "catalog tests")
        for assignment in records(
            lock.get("assignments"),
            "locked assignments",
        ):
            test_id = assignment.get("catalog_test_id")
            if not isinstance(test_id, str):
                test_id = "TEST_AUTO_" + string(
                    assignment,
                    "discovery_id",
                    "assignment",
                ).removeprefix("DISC_")
            test = tests.get(test_id)
            if test is None:
                failures.append(f"{test_id} is absent from catalog")
                continue
            required = set(
                strings(
                    assignment.get("behavior_refs"),
                    f"{test_id} behavior refs",
                )
            )
            actual = set(
                strings(
                    test.get("mapping_behavior_refs"),
                    f"{test_id} catalog behavior refs",
                )
            )
            if not required <= actual:
                failures.append(f"{test_id} lacks locked behavior refs")
        if proposals.get("proposals") != []:
            failures.append(
                "verification-owned left pending mapping proposals"
            )
        gaps = analyze_frozen_catalog_gaps(
            denominator=denominator,
            catalog=catalog,
            invariants=load_invariant_documents(root),
        )
        summary = gaps["summary"]
        if summary["confirmed_missing_tests"] > 27:
            failures.append(
                "verification-owned residual behavior count exceeds 27"
            )
        if summary["missing_spec_authority_tests"] != 0:
            failures.append(
                "verification-owned reopened authority debt"
            )
        if summary["catalog_mapped"] < 3933:
            failures.append(
                "verification-owned mapped denominator fell below 3,933"
            )
        if summary["reviewed_nonmapping"] > 468:
            failures.append(
                "verification-owned nonmapping denominator exceeds 468"
            )
    except (
        OSError,
        ValueError,
        KeyError,
        tomllib.TOMLDecodeError,
        json.JSONDecodeError,
    ) as error:
        failures.append(str(error))
    return sorted(set(failures))


def _document_digest(document: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        document,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        compilation = (
            apply_verification_owned_mapping(root=root)
            if args.apply
            else compile_verification_owned_mapping(root=root)
        )
    except (
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
    ) as error:
        print(f"verification-owned mapping transaction failed: {error}")
        return 2
    verb = "applied" if args.apply else "ready"
    print(
        f"verification-owned mapping transaction {verb}: "
        f"{compilation.summary['campaign_test_count']} tests, "
        f"{compilation.summary['new_catalog_test_count']} promoted, "
        f"{compilation.summary['confirmed_missing_test_count']} residual"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
