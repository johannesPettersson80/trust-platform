"""Compile and apply the Phase 18 runtime-other mapping transaction."""

from __future__ import annotations

import argparse
import json
import shlex
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .case_backed_behavior_proof import replace_table_record_fields
from .frozen_catalog_gaps import analyze_frozen_catalog_gaps
from .mapping_catalog_updates import (
    append_invariant_tests,
    build_accepted_catalog_record,
    reconcile_invariant_catalog_state,
    render_denominator_document,
)
from .mapping_proposals import (
    build_mapping_proposals,
    duration_class,
    validate_mapping_quarantine,
)
from .mapping_review import build_mapping_decision, validate_mapping_decisions
from .mapping_review_batch import build_review_batch
from .mapping_toml import render_decisions_toml
from .mapping_transaction import apply_mapping_transaction
from .runtime_other_behavior_mapping_review import (
    decision_refinement_input as _decision_refinement_input,
    new_review_input as _new_review_input,
    refreshed_review_input as _refreshed_review_input,
    selected_behavior_refs as _selected_behavior_refs,
    selected_invariant_ids as _selected_invariant_ids,
)
from .runtime_other_behavior_mapping_support import (
    BREAKPOINT_BEHAVIOR_REF,
    BREAKPOINT_DISCOVERY_ID,
    RuntimeOtherMappingCompilation,
    breakpoint_assignment as _breakpoint_assignment,
    candidate as _candidate,
    digest_bytes as _digest_bytes,
    digest_document as _digest_document,
    digest_documents as _digest_documents,
    inventories as _inventories,
    json_document as _json,
    load_invariant_documents as _load_invariant_documents,
    load_invariants as _load_invariants,
    records as _records,
    records_by_id as _records_by_id,
    require_unique_catalog_provenance as _require_unique_catalog_provenance,
    scan_facts as _scan_facts,
    string as _string,
    string_sequence as _string_sequence,
    strings as _strings,
    toml_document as _toml,
    toml_source as _toml_source,
)


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
PROPOSALS_PATH = Path("verification/test-catalog-proposals.toml")
DECISIONS_PATH = Path("verification/test-catalog-mapping-decisions.toml")
LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "runtime-other-106-semantic-lock.json"
)
ARTIFACT_ROOT = Path(
    "target/gate-artifacts/verification/p18-runtime-other-106"
)
ASSIGNMENT_PATH = ARTIFACT_ROOT / "assignment-plan.json"
CAMPAIGN_PLAN_PATH = ARTIFACT_ROOT / "campaign-plan.json"
CAMPAIGN_PATH = ARTIFACT_ROOT / "campaign.json"
BREAKPOINT_CAMPAIGN_PATH = ARTIFACT_ROOT / "campaign-breakpoint.json"
SCAN_PATH = ARTIFACT_ROOT / "live-scan.json"
NEW_ASSERTIONS_PATH = ARTIFACT_ROOT / "assertions.json"
BREAKPOINT_ASSERTIONS_PATH = ARTIFACT_ROOT / "assertions-breakpoint.json"
REFRESH_ASSERTION_PATHS = (
    ARTIFACT_ROOT / "assertions-refresh.json",
    ARTIFACT_ROOT / "assertions-selected-stale.json",
    ARTIFACT_ROOT / "assertions-behavior-refresh.json",
)
REFRESH_RAW_PATHS = (
    ARTIFACT_ROOT / "raw/RUNTIME_OTHER_REFRESH_01.jsonl",
    ARTIFACT_ROOT / "raw/RUNTIME_OTHER_REFRESH_02.jsonl",
)
REVIEWED_DATE = "2026-07-28"
REVIEWED_TIMESTAMP = "2026-07-28T18:30:00Z"
SOURCE_COMMIT = "1dc849081acc06da759b56888930a36b3c6eeee2"
MAPPING_DEBT_REASON = "no_reviewed_spec_or_invariant_binding"


def compile_runtime_other_mapping(
    *,
    root: Path,
) -> RuntimeOtherMappingCompilation:
    """Return the complete, unapplied runtime-other mapping transaction."""

    root = root.resolve()
    if (root / LOCK_PATH).is_file():
        return _reconstruct_applied(root)

    assignment = _json(root / ASSIGNMENT_PATH)
    campaign_plan = _json(root / CAMPAIGN_PLAN_PATH)
    campaign = _json(root / CAMPAIGN_PATH)
    breakpoint_campaign = _json(root / BREAKPOINT_CAMPAIGN_PATH)
    scan = _json(root / SCAN_PATH)
    _validate_frozen_execution(
        assignment=assignment,
        campaign_plan=campaign_plan,
        campaign=campaign,
        breakpoint_campaign=breakpoint_campaign,
    )

    catalog_source = (root / CATALOG_PATH).read_text()
    catalog = _toml_source(catalog_source, CATALOG_PATH.as_posix())
    denominator = _toml(root / DENOMINATOR_PATH)
    proposal_document = _toml(root / PROPOSALS_PATH)
    decision_document = _toml(root / DECISIONS_PATH)
    if proposal_document.get("proposals") != []:
        raise ValueError("runtime-other mapping requires an empty proposal queue")

    invariant_sources, invariants = _load_invariants(root)
    facts = _scan_facts(scan)
    denominator, denominator_additions = _augment_denominator(
        denominator=denominator,
        facts=facts,
        discovery_ids={
            str(row["discovery_id"])
            for row in _records(assignment.get("assignments"), "assignments")
            if row.get("resolution") == "new_catalog"
        }
        | {BREAKPOINT_DISCOVERY_ID},
    )

    new_assignments = [
        dict(row)
        for row in _records(assignment.get("assignments"), "assignments")
        if row.get("resolution") == "new_catalog"
    ]
    new_assignments.append(_breakpoint_assignment(facts))
    if len(new_assignments) != 34:
        raise ValueError(
            f"runtime-other selected {len(new_assignments)} new mappings, expected 34"
        )
    new_inventories = _inventories(root / NEW_ASSERTIONS_PATH)
    new_inventories.update(_inventories(root / BREAKPOINT_ASSERTIONS_PATH))
    if set(new_inventories) != {
        str(row["discovery_id"]) for row in new_assignments
    }:
        raise ValueError("new assertion inventories drifted from selected mappings")

    new_trace_records = _new_trace_records(
        campaign_plan=campaign_plan,
        campaign=campaign,
        breakpoint_campaign=breakpoint_campaign,
        assignments=new_assignments,
    )
    new_proposals = build_mapping_proposals(
        root=root,
        candidates=[
            _candidate(
                discovery_id=str(row["discovery_id"]),
                invariant_ids=_strings(row.get("invariant_ids"), "invariant IDs"),
            )
            for row in new_assignments
        ],
        trace_records=new_trace_records,
        facts={
            str(row["discovery_id"]): facts[str(row["discovery_id"])]
            for row in new_assignments
        },
        invariants=invariants,
        assertion_inventories=new_inventories,
    )
    quarantine_failures = validate_mapping_quarantine(
        root=root,
        proposals=new_proposals,
        facts=facts,
        tests=_records_by_id(catalog.get("tests"), "catalog tests"),
        denominator_reviews={
            str(row["discovery_id"]): row
            for row in _records(denominator.get("reviews"), "denominator reviews")
        },
        invariants=invariants,
    )
    if quarantine_failures:
        raise ValueError("; ".join(quarantine_failures))

    proposal_document = {
        **proposal_document,
        "source_commit": SOURCE_COMMIT,
        "trace_digest": _digest_documents(campaign, breakpoint_campaign),
        "candidate_report_digest": _digest_document(assignment),
        "last_generated": REVIEWED_TIMESTAMP,
        "proposals": new_proposals,
    }
    assignment_by_discovery = {
        str(row["discovery_id"]): row for row in new_assignments
    }
    new_review_inputs = [
        _new_review_input(
            proposal=proposal,
            assignment=assignment_by_discovery[str(proposal["discovery_id"])],
            invariants=invariants,
        )
        for proposal in new_proposals
    ]
    selected_new_invariants = sorted(
        {
            invariant_id
            for row in new_assignments
            for invariant_id in _strings(row.get("invariant_ids"), "invariant IDs")
        }
    )
    new_batch = build_review_batch(
        proposal_document=proposal_document,
        decision_document=decision_document,
        review_inputs=new_review_inputs,
        catalog_source=catalog_source,
        denominator_document=denominator,
        invariant_sources={
            invariant_id: invariant_sources[invariant_id]
            for invariant_id in selected_new_invariants
        },
        invariants=invariants,
        reviewed_date=REVIEWED_DATE,
        source_commit=SOURCE_COMMIT,
    )
    if len(new_batch.accepted_test_ids) != 34:
        raise ValueError("runtime-other review did not accept all new mappings")

    replacements = dict(new_batch.replacements)
    catalog_source = replacements[CATALOG_PATH.as_posix()].decode()
    catalog = _toml_source(catalog_source, "new-batch catalog")
    decision_document = _toml_source(
        replacements[DECISIONS_PATH.as_posix()].decode(),
        "new-batch decisions",
    )
    proposal_document = _toml_source(
        replacements[PROPOSALS_PATH.as_posix()].decode(),
        "new-batch proposals",
    )
    denominator = _toml_source(
        replacements[DENOMINATOR_PATH.as_posix()].decode(),
        "new-batch denominator",
    )

    assignments = [
        dict(row)
        for row in _records(assignment.get("assignments"), "assignments")
    ]
    catalog_by_id = _records_by_id(catalog.get("tests"), "catalog tests")
    assignment_by_test_id = {
        str(row["catalog_test_id"]): row
        for row in assignments
        if row.get("resolution") == "existing_catalog"
    }
    legacy_updates: dict[str, dict[str, Any]] = {}
    provenance_selected: dict[str, Mapping[str, Any]] = {}
    for test_id, selected in sorted(assignment_by_test_id.items()):
        test = catalog_by_id.get(test_id)
        if test is None:
            raise ValueError(f"runtime-other catalog lacks {test_id}")
        selected_refs = set(
            _strings(selected.get("behavior_refs"), f"{test_id} behavior refs")
        )
        current_refs = set(
            _strings(test.get("mapping_behavior_refs", []), f"{test_id} current refs")
        )
        if test.get("mapping_decision_id"):
            provenance_selected[test_id] = selected
        else:
            legacy_updates[test_id] = {
                "mapping_behavior_refs": sorted(current_refs | selected_refs)
            }
    if len(legacy_updates) != 152 or len(provenance_selected) != 11:
        raise ValueError(
            "runtime-other existing mapping provenance split drifted: "
            f"legacy={len(legacy_updates)}, reviewed={len(provenance_selected)}"
        )
    catalog_source = replace_table_record_fields(
        catalog_source,
        table="tests",
        updates=legacy_updates,
    )
    catalog = _toml_source(catalog_source, "legacy-refined catalog")
    catalog_by_id = _records_by_id(catalog.get("tests"), "catalog tests")

    refresh_inventories: dict[str, Mapping[str, Any]] = {}
    for relative in REFRESH_ASSERTION_PATHS:
        overlap = set(refresh_inventories) & set(_inventories(root / relative))
        if overlap:
            raise ValueError(f"refresh assertion inventories overlap: {sorted(overlap)}")
        refresh_inventories.update(_inventories(root / relative))
    if len(refresh_inventories) != 12:
        raise ValueError(
            f"runtime-other selected {len(refresh_inventories)} proposal refreshes"
        )
    catalog_by_discovery = {
        str(row["discovery_id"]): row
        for row in catalog_by_id.values()
        if isinstance(row.get("discovery_id"), str)
    }
    refresh_tests = {
        str(catalog_by_discovery[discovery_id]["id"]): catalog_by_discovery[discovery_id]
        for discovery_id in refresh_inventories
    }
    refresh_trace_records = _refresh_trace_records(
        root=root,
        campaign=campaign,
        refresh_tests=refresh_tests,
    )
    refreshed_proposals = build_mapping_proposals(
        root=root,
        candidates=[
            _candidate(
                discovery_id=str(test["discovery_id"]),
                invariant_ids=_selected_invariant_ids(
                    test=test,
                    assignment=assignment_by_test_id.get(test_id),
                ),
            )
            for test_id, test in sorted(refresh_tests.items())
        ],
        trace_records=refresh_trace_records,
        facts={
            str(test["discovery_id"]): facts[str(test["discovery_id"])]
            for test in refresh_tests.values()
        },
        invariants=invariants,
        assertion_inventories=refresh_inventories,
    )
    refreshed_by_discovery = {
        str(proposal["discovery_id"]): proposal
        for proposal in refreshed_proposals
    }

    snapshots = _records_by_id(
        decision_document.get("proposal_snapshots"),
        "proposal snapshots",
    )
    decisions = _records_by_id(decision_document.get("decisions"), "decisions")
    catalog_updates: dict[str, dict[str, Any]] = {}
    source_closure_refresh_count = 0
    updated_provenance_ids = sorted(
        set(provenance_selected) | set(refresh_tests)
    )
    if len(updated_provenance_ids) != 15:
        raise ValueError(
            "runtime-other mapping provenance population drifted: "
            f"{len(updated_provenance_ids)}"
        )
    for test_id in updated_provenance_ids:
        test = catalog_by_id[test_id]
        old_proposal_id = _string(test, "mapping_proposal_id", test_id)
        old_decision_id = _string(test, "mapping_decision_id", test_id)
        old_proposal = snapshots.get(old_proposal_id)
        old_decision = decisions.get(old_decision_id)
        if old_proposal is None or old_decision is None:
            raise ValueError(f"{test_id} lacks reviewed mapping provenance")
        _require_unique_catalog_provenance(
            catalog_tests=catalog_by_id.values(),
            test_id=test_id,
            proposal_id=old_proposal_id,
            decision_id=old_decision_id,
        )
        del decisions[old_decision_id]
        assignment_row = assignment_by_test_id.get(test_id)
        selected_invariants = _selected_invariant_ids(
            test=test,
            assignment=assignment_row,
        )
        selected_refs = _selected_behavior_refs(
            test=test,
            assignment=assignment_row,
        )
        discovery_id = _string(test, "discovery_id", test_id)
        if discovery_id in refreshed_by_discovery:
            proposal = refreshed_by_discovery[discovery_id]
            if old_proposal.get("assertion_source_digests") != proposal.get(
                "assertion_source_digests"
            ):
                source_closure_refresh_count += 1
            del snapshots[old_proposal_id]
            snapshots[str(proposal["id"])] = proposal
            review_input = _refreshed_review_input(
                proposal=proposal,
                test=test,
                invariant_ids=selected_invariants,
                behavior_refs=selected_refs,
            )
        else:
            proposal = old_proposal
            review_input = _decision_refinement_input(
                proposal=proposal,
                old_decision=old_decision,
                test=test,
                invariant_ids=selected_invariants,
                behavior_refs=selected_refs,
            )
        decision = build_mapping_decision(
            proposal=proposal,
            review_input=review_input,
        )
        if str(decision["id"]) in decisions:
            raise ValueError(f"{test_id} generated a duplicate decision")
        decisions[str(decision["id"])] = decision
        record = build_accepted_catalog_record(
            proposal=proposal,
            decision=decision,
            invariants=invariants,
            reviewed_date=REVIEWED_DATE,
        )
        if record["id"] != test_id:
            raise ValueError(f"{test_id} refresh changed catalog identity")
        catalog_updates[test_id] = {
            field: value
            for field, value in record.items()
            if field != "id" and test.get(field) != value
        }
    if source_closure_refresh_count != 7:
        raise ValueError(
            "runtime-other source closure refresh drifted: "
            f"{source_closure_refresh_count}"
        )
    catalog_source = replace_table_record_fields(
        catalog_source,
        table="tests",
        updates=catalog_updates,
    )
    catalog = _toml_source(catalog_source, "provenance-refreshed catalog")
    catalog_by_id = _records_by_id(catalog.get("tests"), "catalog tests")

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
        invariants=invariants,
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
    test_links = _assignment_test_links(
        assignments=assignments,
        breakpoint_assignment=_breakpoint_assignment(facts),
    )
    touched_invariants = sorted(test_links)
    for invariant_id in touched_invariants:
        source = final_invariant_sources[invariant_id]
        document = _toml_source(source, invariant_id)
        current = set(_strings(document.get("tests"), f"{invariant_id} tests"))
        additions = sorted(test_links[invariant_id] - current)
        if additions:
            source = append_invariant_tests(source, additions)
        source = reconcile_invariant_catalog_state(
            source,
            invariant_id=invariant_id,
            catalog_source=catalog_source,
        )
        final_invariant_sources[invariant_id] = source
        path = invariant_sources[invariant_id][0]
        if source.encode() != (root / path).read_bytes():
            replacements[path] = source.encode()

    final_invariants = [
        (invariant_sources[invariant_id][0], _toml_source(source, invariant_id))
        for invariant_id, source in sorted(final_invariant_sources.items())
    ]
    gap_analysis = analyze_frozen_catalog_gaps(
        denominator=denominator,
        catalog=catalog,
        invariants=final_invariants,
    )
    confirmed_missing = int(
        gap_analysis["summary"]["confirmed_missing_tests"]
    )
    if confirmed_missing != 90:
        raise ValueError(
            f"runtime-other transaction left {confirmed_missing} missing behaviors"
        )
    if int(gap_analysis["summary"]["missing_spec_authority_tests"]) != 0:
        raise ValueError("runtime-other transaction reopened test authority debt")

    summary = {
        "behavior_demand_count": 106,
        "campaign_test_count": 196,
        "new_catalog_test_count": 34,
        "existing_catalog_test_count": 163,
        "refreshed_catalog_test_count": 12,
        "source_closure_refresh_count": source_closure_refresh_count,
        "updated_mapping_provenance_count": len(updated_provenance_ids),
        "denominator_addition_count": denominator_additions,
        "confirmed_missing_test_count": confirmed_missing,
    }
    lock = _build_lock(
        root=root,
        summary=summary,
        assignments=assignments,
        breakpoint_assignment=_breakpoint_assignment(facts),
        refreshed_test_ids=sorted(refresh_tests),
        updated_provenance_ids=updated_provenance_ids,
        touched_invariant_paths=sorted(
            invariant_sources[invariant_id][0]
            for invariant_id in touched_invariants
        ),
    )
    replacements[LOCK_PATH.as_posix()] = (
        json.dumps(lock, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode()
    return RuntimeOtherMappingCompilation(
        summary=summary,
        replacements=dict(sorted(replacements.items())),
    )


def apply_runtime_other_mapping(*, root: Path) -> RuntimeOtherMappingCompilation:
    """Compile and atomically apply the reviewed runtime-other transaction."""

    root = root.resolve()
    compilation = compile_runtime_other_mapping(root=root)
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


def _validate_frozen_execution(
    *,
    assignment: Mapping[str, Any],
    campaign_plan: Mapping[str, Any],
    campaign: Mapping[str, Any],
    breakpoint_campaign: Mapping[str, Any],
) -> None:
    expected_assignment = {
        "demand_count": 106,
        "assigned_test_count": 196,
        "existing_catalog_test_count": 163,
        "new_catalog_test_count": 33,
        "assignment_link_count": 230,
        "expected_post_transaction_missing_tests": 90,
    }
    for field, expected in expected_assignment.items():
        if assignment.get(field) != expected:
            raise ValueError(f"runtime-other assignment {field} drifted")
    if assignment.get("unresolved") != []:
        raise ValueError("runtime-other assignment contains unresolved routes")
    if (
        campaign_plan.get("test_count") != 196
        or campaign_plan.get("group_count") != 27
        or campaign.get("source_commit") != SOURCE_COMMIT
        or campaign.get("test_count") != 196
        or campaign.get("passed_count") != 196
        or campaign.get("all_passed") is not True
        or campaign.get("failures") != []
    ):
        raise ValueError("runtime-other focused campaign is not the frozen green set")
    if (
        breakpoint_campaign.get("source_commit") != SOURCE_COMMIT
        or breakpoint_campaign.get("test_count") != 1
        or breakpoint_campaign.get("passed_count") != 1
        or breakpoint_campaign.get("all_passed") is not True
        or breakpoint_campaign.get("failures") != []
    ):
        raise ValueError("breakpoint denominator-closure campaign is not green")


def _augment_denominator(
    *,
    denominator: Mapping[str, Any],
    facts: Mapping[str, Mapping[str, Any]],
    discovery_ids: set[str],
) -> tuple[dict[str, Any], int]:
    document = json.loads(json.dumps(denominator))
    reviews = _records(document.get("reviews"), "denominator reviews")
    present = {
        _string(row, "discovery_id", "denominator review") for row in reviews
    }
    missing = sorted(discovery_ids - present)
    for discovery_id in missing:
        fact = facts[discovery_id]
        reviews.append(
            {
                "discovery_id": discovery_id,
                "discovery_source_kind": fact["discovery_source_kind"],
                "path": fact["path"],
                "name": fact["name"],
                "ignore_state": fact["ignore_state"],
                "disposition": "reviewed_nonmapping",
                "reason_code": MAPPING_DEBT_REASON,
                "last_reviewed": REVIEWED_DATE,
            }
        )
    document["reviews"] = sorted(
        reviews, key=lambda row: str(row.get("discovery_id", ""))
    )
    document["last_reviewed"] = REVIEWED_DATE
    if len(missing) != 5:
        raise ValueError(
            f"runtime-other denominator additions drifted: {len(missing)}"
        )
    rendered = render_denominator_document(document)
    reparsed = _toml_source(rendered, "augmented denominator")
    if len(_records(reparsed.get("reviews"), "denominator reviews")) != 4399:
        raise ValueError("runtime-other denominator does not equal the live scan")
    return reparsed, len(missing)


def _new_trace_records(
    *,
    campaign_plan: Mapping[str, Any],
    campaign: Mapping[str, Any],
    breakpoint_campaign: Mapping[str, Any],
    assignments: Sequence[Mapping[str, Any]],
) -> dict[str, dict[str, Any]]:
    durations = {
        str(row["test_id"]): int(row["duration_ms"])
        for row in _records(campaign.get("results"), "campaign results")
    }
    durations.update(
        {
            str(row["test_id"]): int(row["duration_ms"])
            for row in _records(
                breakpoint_campaign.get("results"),
                "breakpoint campaign results",
            )
        }
    )
    commands = _individual_commands(campaign_plan)
    breakpoint_groups = _records(
        breakpoint_campaign.get("groups"),
        "breakpoint campaign groups",
    )
    breakpoint_results = _records(
        breakpoint_campaign.get("results"),
        "breakpoint campaign results",
    )
    if len(breakpoint_groups) != 1 or len(breakpoint_results) != 1:
        raise ValueError("breakpoint campaign is not one exact execution")
    commands[str(breakpoint_results[0]["test_id"])] = shlex.join(
        _string_sequence(
            breakpoint_groups[0].get("command"),
            "breakpoint campaign command",
        )
    )
    result = {}
    for assignment in assignments:
        discovery_id = _string(assignment, "discovery_id", "assignment")
        if discovery_id not in durations or discovery_id not in commands:
            raise ValueError(f"{discovery_id} lacks exact focused execution")
        result[discovery_id] = {
            "discovery_id": discovery_id,
            "trace_status": "observed",
            "command": commands[discovery_id],
            "command_authority": "exact",
            "duration_ms": durations[discovery_id],
        }
    return result


def _individual_commands(document: Mapping[str, Any]) -> dict[str, str]:
    commands: dict[str, str] = {}
    for group in _records(document.get("groups"), "campaign groups"):
        command = _string_sequence(group.get("command"), "campaign command")
        full_names = _strings(group.get("full_names"), "campaign full names")
        tests_by_name = group.get("tests_by_full_name")
        if not isinstance(tests_by_name, Mapping):
            continue
        try:
            expression_index = command.index("-E") + 1
        except ValueError as error:
            raise ValueError("campaign group lacks an exact expression") from error
        binary_id = _string(group, "binary_id", "campaign group")
        for full_name in full_names:
            test_id = str(tests_by_name[full_name])
            exact = list(command)
            exact[expression_index] = (
                f"(binary_id(={binary_id}) & test(={full_name}))"
            )
            commands[test_id] = shlex.join(exact)
    return commands


def _refresh_trace_records(
    *,
    root: Path,
    campaign: Mapping[str, Any],
    refresh_tests: Mapping[str, Mapping[str, Any]],
) -> dict[str, dict[str, Any]]:
    durations_by_test = {
        str(row["test_id"]): int(row["duration_ms"])
        for row in _records(campaign.get("results"), "campaign results")
    }
    durations_by_name: dict[str, int] = {}
    for relative in REFRESH_RAW_PATHS:
        for line in (root / relative).read_text().splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if (
                isinstance(event, Mapping)
                and event.get("type") == "test"
                and event.get("event") == "ok"
                and isinstance(event.get("name"), str)
                and isinstance(event.get("exec_time"), (int, float))
            ):
                name = str(event["name"]).partition("$")[2]
                durations_by_name[name] = int(
                    round(float(event["exec_time"]) * 1000)
                )
    result = {}
    for test_id, test in refresh_tests.items():
        discovery_id = _string(test, "discovery_id", test_id)
        duration = durations_by_test.get(test_id)
        if duration is None:
            duration = durations_by_name.get(_string(test, "name", test_id))
        if duration is None:
            raise ValueError(f"{test_id} lacks measured refresh execution")
        result[discovery_id] = {
            "discovery_id": discovery_id,
            "trace_status": "observed",
            "command": _string(test, "command", test_id),
            "command_authority": "exact",
            "duration_ms": duration,
        }
    return result


def _assignment_test_links(
    *,
    assignments: Sequence[Mapping[str, Any]],
    breakpoint_assignment: Mapping[str, Any],
) -> dict[str, set[str]]:
    links: dict[str, set[str]] = {}
    for assignment in [*assignments, breakpoint_assignment]:
        test_id = assignment.get("catalog_test_id")
        if test_id is None:
            test_id = "TEST_AUTO_" + _string(
                assignment, "discovery_id", "assignment"
            ).removeprefix("DISC_")
        for invariant_id in _strings(
            assignment.get("invariant_ids"), "assignment invariants"
        ):
            links.setdefault(invariant_id, set()).add(str(test_id))
    return links


def _build_lock(
    *,
    root: Path,
    summary: Mapping[str, int],
    assignments: Sequence[Mapping[str, Any]],
    breakpoint_assignment: Mapping[str, Any],
    refreshed_test_ids: list[str],
    updated_provenance_ids: list[str],
    touched_invariant_paths: list[str],
) -> dict[str, Any]:
    evidence_paths = [
        ASSIGNMENT_PATH,
        CAMPAIGN_PLAN_PATH,
        CAMPAIGN_PATH,
        BREAKPOINT_CAMPAIGN_PATH,
        SCAN_PATH,
        NEW_ASSERTIONS_PATH,
        BREAKPOINT_ASSERTIONS_PATH,
        *REFRESH_ASSERTION_PATHS,
        *REFRESH_RAW_PATHS,
    ]
    return {
        "schema_version": 1,
        "id": "RUNTIME_OTHER_106_SEMANTIC_LOCK",
        "source_commit": SOURCE_COMMIT,
        "reviewed_date": REVIEWED_DATE,
        "summary": dict(summary),
        "evidence_digests": {
            path.as_posix(): _digest_bytes((root / path).read_bytes())
            for path in evidence_paths
        },
        "assignments": list(assignments),
        "denominator_closure_mappings": [dict(breakpoint_assignment)],
        "refreshed_catalog_test_ids": refreshed_test_ids,
        "updated_mapping_provenance_test_ids": updated_provenance_ids,
        "touched_invariant_paths": touched_invariant_paths,
    }


def _reconstruct_applied(root: Path) -> RuntimeOtherMappingCompilation:
    failures = _validate_applied(root)
    if failures:
        raise ValueError("; ".join(failures))
    lock = _json(root / LOCK_PATH)
    summary = lock.get("summary")
    if not isinstance(summary, Mapping) or any(
        not isinstance(value, int) or isinstance(value, bool)
        for value in summary.values()
    ):
        raise ValueError("runtime-other semantic lock summary is invalid")
    paths = {
        CATALOG_PATH.as_posix(),
        DENOMINATOR_PATH.as_posix(),
        PROPOSALS_PATH.as_posix(),
        DECISIONS_PATH.as_posix(),
        LOCK_PATH.as_posix(),
        *_strings(lock.get("touched_invariant_paths"), "touched invariant paths"),
    }
    return RuntimeOtherMappingCompilation(
        summary={str(key): int(value) for key, value in summary.items()},
        replacements={
            path: (root / path).read_bytes() for path in sorted(paths)
        },
    )


def _validate_applied(root: Path) -> list[str]:
    failures: list[str] = []
    try:
        lock = _json(root / LOCK_PATH)
        catalog = _toml(root / CATALOG_PATH)
        denominator = _toml(root / DENOMINATOR_PATH)
        proposals = _toml(root / PROPOSALS_PATH)
        tests = _records_by_id(catalog.get("tests"), "catalog tests")
        reviews = _records(denominator.get("reviews"), "denominator reviews")
        if proposals.get("proposals") != []:
            failures.append("runtime-other left pending mapping proposals")
        if len(reviews) != 4399 or len(
            {str(row.get("discovery_id")) for row in reviews}
        ) != 4399:
            failures.append("runtime-other denominator is not the live 4,399 set")
        catalog_by_discovery = {
            str(row.get("discovery_id")): row
            for row in tests.values()
            if isinstance(row.get("discovery_id"), str)
        }
        assignments = [
            *_records(lock.get("assignments"), "locked assignments"),
            *_records(
                lock.get("denominator_closure_mappings"),
                "denominator closure mappings",
            ),
        ]
        for assignment in assignments:
            discovery_id = _string(
                assignment, "discovery_id", "locked assignment"
            )
            test = catalog_by_discovery.get(discovery_id)
            if test is None:
                failures.append(f"{discovery_id} lacks applied catalog mapping")
                continue
            required_refs = set(
                _strings(assignment.get("behavior_refs"), "locked behavior refs")
            )
            actual_refs = set(
                _strings(test.get("mapping_behavior_refs"), "catalog behavior refs")
            )
            if not required_refs <= actual_refs:
                failures.append(f"{discovery_id} lacks locked behavior refs")
        breakpoint = catalog_by_discovery.get(BREAKPOINT_DISCOVERY_ID)
        if (
            breakpoint is None
            or breakpoint.get("mapping_behavior_refs")
            != [BREAKPOINT_BEHAVIOR_REF]
        ):
            failures.append("breakpoint identity is not exactly behavior-bound")
        invariants = _load_invariant_documents(root)
        gaps = analyze_frozen_catalog_gaps(
            denominator=denominator,
            catalog=catalog,
            invariants=invariants,
        )
        if gaps["summary"]["confirmed_missing_tests"] > 90:
            failures.append("runtime-other residual behavior count exceeds 90")
        if gaps["summary"]["missing_spec_authority_tests"] != 0:
            failures.append("runtime-other reopened test authority debt")
    except (OSError, ValueError, KeyError, tomllib.TOMLDecodeError) as error:
        failures.append(str(error))
    return sorted(set(failures))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        compilation = (
            apply_runtime_other_mapping(root=root)
            if args.apply
            else compile_runtime_other_mapping(root=root)
        )
        verb = "applied" if args.apply else "ready"
        print(
            f"runtime-other mapping transaction {verb}: "
            f"{compilation.summary['new_catalog_test_count']} new, "
            f"{compilation.summary['existing_catalog_test_count']} existing, "
            f"{compilation.summary['confirmed_missing_test_count']} residual"
        )
        return 0
    except (
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
    ) as error:
        print(f"runtime-other mapping transaction failed: {error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
