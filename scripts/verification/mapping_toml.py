"""Deterministic TOML rendering for generated mapping control-plane files."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from typing import Any


PROPOSAL_ROOT_FIELDS = (
    "schema_version",
    "generator",
    "generator_version",
    "source_commit",
    "trace_digest",
    "candidate_report_digest",
    "last_generated",
)
DEMAND_FIELDS = (
    "discovery_id",
    "candidate_tier",
    "covered_code_ids",
    "spec_gap_refs",
    "unmapped_code_ids",
)
DEMAND_RANKING_FIELDS = (
    "spec_gap_ref",
    "affected_discovery_ids",
    "covered_code_ids",
    "tests_touching",
    "code_facts_touched",
)
STRUCTURAL_DEMAND_RANKING_FIELDS = (
    "cell_id",
    "parent_spec_gap_ref",
    "area",
    "owner",
    "source_kind",
    "path",
    "qualified_symbol_parent",
    "item_kind",
    "affected_discovery_ids",
    "covered_code_ids",
    "tests_touching",
    "tests_exclusive",
    "tests_dominant",
    "code_facts_touched",
)
UNMAPPED_CODE_DEMAND_RANKING_FIELDS = (
    "area",
    "affected_discovery_ids",
    "covered_code_ids",
    "tests_touching",
    "code_facts_touched",
)
PROPOSAL_FIELDS = (
    "schema_version",
    "id",
    "proposal_digest",
    "discovery_id",
    "discovery_source_kind",
    "path",
    "name",
    "ignore_state",
    "command",
    "command_authority",
    "duration_ms",
    "duration_class",
    "status",
    "candidate_tier",
    "candidate_invariant_ids",
    "covered_code_ids",
    "specified_code_ids",
    "spec_gap_code_ids",
    "spec_gap_refs",
    "mutation_kill_ids",
    "test_source_digest",
    "assertion_inventory_digest",
    "unresolved_assertion_helpers",
    "trace_record_digest",
    "candidate_signal_digest",
    "oracle_refs",
    "behavior_refs",
)
ASSERTION_FIELDS = (
    "assertion_id",
    "kind",
    "function",
    "line",
    "expression_digest",
    "expression",
)
SUGGESTION_FIELDS = (
    "authoritative",
    "test_class",
    "expected_result",
    "expected_failure_mode",
    "source_behavior_refs",
)
DECISION_ROOT_FIELDS = (
    "schema_version",
    "generator",
    "generator_version",
    "source_commit",
)
DECISION_COMMON_FIELDS = (
    "schema_version",
    "id",
    "proposal_id",
    "proposal_digest",
    "discovery_id",
    "outcome",
    "reviewer_ids",
    "reviewed_source_digest",
    "selected_invariant_ids",
    "selected_behavior_refs",
    "command",
    "command_authority",
)
ASSERTION_REVIEW_FIELDS = (
    "assertion_id",
    "disposition",
    "behavior_refs",
    "rationale",
)
CONFIRMED_CATALOG_FIELDS = (
    "test_class",
    "area",
    "owner",
    "expected_result",
    "expected_failure_mode",
    "suite_tiers",
    "requires_hardware",
    "requires_network",
    "duration_class",
)


def render_decisions_toml(document: Mapping[str, Any]) -> str:
    if set(document) != {
        *DECISION_ROOT_FIELDS,
        "proposal_snapshots",
        "decisions",
    }:
        raise ValueError("decision document root fields drift")
    lines = [
        f"{field} = {_toml_value(document[field])}" for field in DECISION_ROOT_FIELDS
    ]
    snapshots = document.get("proposal_snapshots")
    if not isinstance(snapshots, Sequence) or isinstance(
        snapshots, (str, bytes)
    ):
        raise ValueError("decision document proposal snapshots must be an array")
    decisions = document.get("decisions")
    if not isinstance(decisions, Sequence) or isinstance(decisions, (str, bytes)):
        raise ValueError("decision document decisions must be an array")
    # TOML cannot return to the root after opening a table. Emit every empty
    # root array before the first proposal snapshot or decision table.
    if not snapshots:
        lines.append("proposal_snapshots = []")
    if not decisions:
        lines.append("decisions = []")
    for proposal in sorted(
        snapshots,
        key=lambda record: str(record.get("id", ""))
        if isinstance(record, Mapping)
        else "",
    ):
        _append_proposal_table(lines, proposal, table_name="proposal_snapshots")
    ordered = sorted(
        decisions,
        key=lambda record: str(record.get("id", ""))
        if isinstance(record, Mapping)
        else "",
    )
    for decision in ordered:
        if not isinstance(decision, Mapping):
            raise ValueError("decision must be a table")
        outcome = decision.get("outcome")
        expected = {*DECISION_COMMON_FIELDS, "assertion_reviews"}
        if outcome == "accepted":
            expected.add("confirmed_catalog")
        elif outcome == "rejected":
            expected.add("rejection_reason")
        else:
            raise ValueError("decision outcome must be accepted or rejected")
        if set(decision) != expected:
            raise ValueError("decision fields drift")
        lines.extend(["", "[[decisions]]"])
        lines.extend(
            f"{field} = {_toml_value(decision[field])}"
            for field in DECISION_COMMON_FIELDS
        )
        if outcome == "rejected":
            lines.append(
                f"rejection_reason = {_toml_value(decision['rejection_reason'])}"
            )
        assertion_reviews = decision.get("assertion_reviews")
        if not isinstance(assertion_reviews, Sequence) or isinstance(
            assertion_reviews, (str, bytes)
        ):
            raise ValueError("decision assertion reviews must be an array")
        if not assertion_reviews:
            lines.append("assertion_reviews = []")
        for review in assertion_reviews:
            if not isinstance(review, Mapping) or set(review) != set(
                ASSERTION_REVIEW_FIELDS
            ):
                raise ValueError("decision assertion review fields drift")
            lines.extend(["", "[[decisions.assertion_reviews]]"])
            lines.extend(
                f"{field} = {_toml_value(review[field])}"
                for field in ASSERTION_REVIEW_FIELDS
            )
        if outcome == "rejected":
            continue
        confirmed = decision.get("confirmed_catalog")
        if not isinstance(confirmed, Mapping) or set(confirmed) != set(
            CONFIRMED_CATALOG_FIELDS
        ):
            raise ValueError("confirmed catalog fields drift")
        lines.extend(["", "[decisions.confirmed_catalog]"])
        lines.extend(
            f"{field} = {_toml_value(confirmed[field])}"
            for field in CONFIRMED_CATALOG_FIELDS
        )
    return "\n".join(lines) + "\n"


def render_proposals_toml(document: Mapping[str, Any]) -> str:
    expected_root = {
        *PROPOSAL_ROOT_FIELDS,
        "demands",
        "demand_ranking",
        "structural_demand_ranking",
        "unmapped_code_demand_ranking",
        "proposals",
    }
    if set(document) != expected_root:
        raise ValueError("proposal document root fields drift")
    lines = [
        f"{field} = {_toml_value(document[field])}" for field in PROPOSAL_ROOT_FIELDS
    ]
    object_tables = (
        ("demands", document.get("demands"), DEMAND_FIELDS),
        (
            "demand_ranking",
            document.get("demand_ranking"),
            DEMAND_RANKING_FIELDS,
        ),
        (
            "structural_demand_ranking",
            document.get("structural_demand_ranking"),
            STRUCTURAL_DEMAND_RANKING_FIELDS,
        ),
        (
            "unmapped_code_demand_ranking",
            document.get("unmapped_code_demand_ranking"),
            UNMAPPED_CODE_DEMAND_RANKING_FIELDS,
        ),
    )
    for table_name, value, _fields in object_tables:
        _require_object_sequence(value, table_name)
        if not value:
            lines.append(f"{table_name} = []")
    proposals = document.get("proposals")
    if not isinstance(proposals, Sequence) or isinstance(proposals, (str, bytes)):
        raise ValueError("proposal document proposals must be an array")
    if not proposals:
        lines.append("proposals = []")
    # Empty root arrays are now declared; table arrays can safely follow.
    for table_name, value, fields in object_tables:
        if value:
            _append_object_tables(
                lines,
                value,
                table_name=table_name,
                fields=fields,
            )
    for proposal in proposals:
        _append_proposal_table(lines, proposal, table_name="proposals")
    return "\n".join(lines) + "\n"


def _append_object_tables(
    lines: list[str],
    value: Any,
    *,
    table_name: str,
    fields: tuple[str, ...],
) -> None:
    _require_object_sequence(value, table_name)
    for record in value:
        if not isinstance(record, Mapping) or set(record) != set(fields):
            raise ValueError(f"{table_name} fields drift")
        lines.extend(["", f"[[{table_name}]]"])
        lines.extend(
            f"{field} = {_toml_value(record[field])}" for field in fields
        )


def _require_object_sequence(value: Any, table_name: str) -> None:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{table_name} must be an array")


def _append_proposal_table(
    lines: list[str],
    proposal: Any,
    *,
    table_name: str,
) -> None:
    if not isinstance(proposal, Mapping):
        raise ValueError("proposal must be a table")
    if set(proposal) != {*PROPOSAL_FIELDS, "assertions", "suggestions"}:
        raise ValueError("proposal fields drift")
    lines.extend(["", f"[[{table_name}]]"])
    lines.extend(
        f"{field} = {_toml_value(proposal[field])}" for field in PROPOSAL_FIELDS
    )
    assertions = proposal.get("assertions")
    if not isinstance(assertions, Sequence) or isinstance(
        assertions, (str, bytes)
    ):
        raise ValueError("proposal assertions must be an array")
    if not assertions:
        lines.append("assertions = []")
    for assertion in assertions:
        if not isinstance(assertion, Mapping) or set(assertion) != set(
            ASSERTION_FIELDS
        ):
            raise ValueError("proposal assertion fields drift")
        lines.extend(["", f"[[{table_name}.assertions]]"])
        lines.extend(
            f"{field} = {_toml_value(assertion[field])}"
            for field in ASSERTION_FIELDS
        )
    suggestions = proposal.get("suggestions")
    if not isinstance(suggestions, Mapping) or set(suggestions) != set(
        SUGGESTION_FIELDS
    ):
        raise ValueError("proposal suggestions fields drift")
    lines.extend(["", f"[{table_name}.suggestions]"])
    lines.extend(
        f"{field} = {_toml_value(suggestions[field])}"
        for field in SUGGESTION_FIELDS
    )


def _toml_value(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        if any(not isinstance(item, str) for item in value):
            raise ValueError("generated mapping TOML arrays must contain strings")
        return "[" + ", ".join(json.dumps(item, ensure_ascii=False) for item in value) + "]"
    raise ValueError(f"unsupported generated TOML value {value!r}")
