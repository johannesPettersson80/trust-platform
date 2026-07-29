"""Pure projection for reviewed code-spec semantic-group acceptance."""

from __future__ import annotations

import re
import tomllib
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

from .code_spec_refresh_projection import CODE_MAP_PATH, MAPPING_FIELD_ORDER
from .code_spec_refresh_projection_support import (
    code_id_array as _code_id_array,
    digest as _digest,
    load_json_object as _load_json_object,
    render_table as _render_table,
    replace_blocks as _replace_blocks,
    required_string as _required_string,
    string_array as _string_array,
    table_blocks as _table_blocks,
)


GROUP_ID_RE = re.compile(r"^CODEMAP_GROUP_[0-9A-F]{64}$")
INVARIANT_ID_RE = re.compile(r"^[A-Z][A-Z0-9_]+$")
TEST_ID_RE = re.compile(r"^[A-Z][A-Z0-9_]+$")
CROSS_FINDING_RE = re.compile(
    r"^invariant ([A-Z][A-Z0-9_]+) crosses area without decision$"
)
TEST_FINDING_RE = re.compile(
    r"^test ([A-Z][A-Z0-9_]+) does not link a mapped invariant$"
)
SELECTION_FIELDS = {
    "schema_version",
    "kind",
    "reviewed_on",
    "reviewer_ids",
    "review_ref",
    "source_proposal_sha256",
    "operation_set_digest",
    "semantic_decisions",
}
DECISION_FIELDS = {
    "accept_cross_area": {"review_group_id", "decision"},
    "remove_mapped_test": {"review_group_id", "decision", "test_id"},
}


@dataclass(frozen=True)
class SemanticProjection:
    """Canonical code-map replacement and bound review manifest."""

    replacements: dict[str, bytes]
    manifest: dict[str, Any]


def project_semantic_selection(
    *,
    proposal_bytes: bytes,
    selection_bytes: bytes,
    code_map_bytes: bytes,
) -> SemanticProjection:
    """Project a complete reviewed semantic queue without touching source."""

    proposal = _load_json_object(proposal_bytes, "refresh proposal")
    selection = _load_json_object(selection_bytes, "semantic selection")
    _validate_proposal(proposal)
    _validate_selection(selection, proposal, proposal_bytes)
    if _digest(code_map_bytes) != proposal.get("source_code_map_sha256"):
        raise ValueError("semantic proposal code-map digest is stale")

    blocks = _table_blocks(
        code_map_bytes, "mappings", "code_id", "code-spec map"
    )
    mappings = {block.identifier: block.row for block in blocks}
    groups = _group_index(proposal.get("semantic_review_groups"))
    decisions = _decision_index(selection.get("semantic_decisions"))
    if set(decisions) != set(groups):
        raise ValueError("semantic review is partial")

    reviewed_on = _required_string(
        selection, "reviewed_on", "semantic selection"
    )
    review_ref = _required_string(
        selection, "review_ref", "semantic selection"
    )
    if "#" not in review_ref:
        raise ValueError("semantic selection review_ref must contain an anchor")

    updated: dict[str, dict[str, Any]] = {}
    accepted_cross = 0
    removed_tests = 0
    for group_id, group in groups.items():
        decision = decisions[group_id]
        kind = _required_string(group, "kind", group_id)
        finding = _required_string(group, "finding", group_id)
        affected = _code_id_array(
            group.get("affected_code_ids"),
            f"{group_id} affected code IDs",
        )
        if group.get("affected_refs") != affected:
            raise ValueError(f"{group_id}: affected finding references drift")
        if kind == "cross_area_decision":
            _apply_cross_area_decision(
                group_id=group_id,
                finding=finding,
                affected=affected,
                decision=decision,
                mappings=mappings,
                updated=updated,
                review_ref=review_ref,
            )
            accepted_cross += 1
        elif kind == "test_invariant_relation":
            _apply_test_removal(
                group_id=group_id,
                finding=finding,
                affected=affected,
                decision=decision,
                mappings=mappings,
                updated=updated,
            )
            removed_tests += 1
        else:
            raise ValueError(
                f"{group_id}: unsupported semantic group kind {kind!r}"
            )

    replacements = {
        code_id: _render_table(
            "mappings",
            {**row, "last_reviewed": reviewed_on},
            MAPPING_FIELD_ORDER,
        )
        for code_id, row in updated.items()
    }
    code_after = _replace_blocks(
        code_map_bytes,
        blocks,
        replacements,
        append_order=[],
    )
    parsed = tomllib.loads(code_after.decode())
    rows = parsed.get("mappings")
    if not isinstance(rows, list) or len(rows) != len(mappings):
        raise ValueError("projected code map changed the mapping denominator")
    ids = [row.get("code_id") for row in rows if isinstance(row, Mapping)]
    if len(ids) != len(set(ids)) or set(ids) != set(mappings):
        raise ValueError("projected code map identity set drifted")

    manifest = {
        "schema_version": 1,
        "kind": "p18_code_spec_semantic_acceptance_projection",
        "reviewed_on": reviewed_on,
        "reviewer_ids": list(selection["reviewer_ids"]),
        "review_ref": review_ref,
        "source_proposal_sha256": _digest(proposal_bytes),
        "selection_sha256": _digest(selection_bytes),
        "operation_set_digest": proposal["operation_set_digest"],
        "semantic_review_groups": len(groups),
        "accepted_cross_area_groups": accepted_cross,
        "removed_mapped_test_groups": removed_tests,
        "affected_code_mappings": len(updated),
        "code_map_before_sha256": _digest(code_map_bytes),
        "code_map_after_sha256": _digest(code_after),
    }
    return SemanticProjection(
        replacements={CODE_MAP_PATH: code_after},
        manifest=manifest,
    )


def _validate_proposal(proposal: Mapping[str, Any]) -> None:
    if proposal.get("schema_version") != 1:
        raise ValueError("semantic proposal schema_version is invalid")
    if (
        proposal.get("generator") != "code-spec-refresh-proposals"
        or proposal.get("mode") != "proposal_only"
    ):
        raise ValueError("semantic proposal generator contract differs")
    for field in (
        "missing_mapping_proposals",
        "digest_rebind_proposals",
        "stale_identity_proposals",
    ):
        rows = proposal.get(field)
        if not isinstance(rows, list) or rows:
            raise ValueError("semantic proposal identity queue is not empty")
    groups = proposal.get("semantic_review_groups")
    if not isinstance(groups, list) or not groups:
        raise ValueError("semantic proposal has no review groups")
    unclassified = proposal.get("unclassified_findings")
    if not isinstance(unclassified, list) or unclassified:
        raise ValueError("semantic proposal has unclassified findings")
    summary = proposal.get("summary")
    if not isinstance(summary, Mapping):
        raise ValueError("semantic proposal summary is malformed")
    expected = {
        "missing_mapping_proposals": 0,
        "digest_rebind_proposals": 0,
        "stale_identity_proposals": 0,
        "semantic_review_groups": len(groups),
        "unclassified_findings": 0,
    }
    for field, value in expected.items():
        if summary.get(field) != value:
            raise ValueError(f"semantic proposal summary {field} differs")
    _required_string(proposal, "operation_set_digest", "semantic proposal")
    _required_string(
        proposal, "source_code_map_sha256", "semantic proposal"
    )


def _validate_selection(
    selection: Mapping[str, Any],
    proposal: Mapping[str, Any],
    proposal_bytes: bytes,
) -> None:
    if set(selection) != SELECTION_FIELDS:
        raise ValueError("semantic selection root fields drift")
    if (
        selection.get("schema_version") != 1
        or selection.get("kind")
        != "p18_code_spec_semantic_acceptance_selection"
    ):
        raise ValueError("semantic selection kind or version is invalid")
    reviewers = _string_array(
        selection.get("reviewer_ids"), "semantic reviewer IDs"
    )
    if len(reviewers) != len(set(reviewers)):
        raise ValueError("semantic reviewer IDs contain duplicates")
    if selection.get("source_proposal_sha256") != _digest(proposal_bytes):
        raise ValueError("semantic selection source proposal digest differs")
    if selection.get("operation_set_digest") != proposal.get(
        "operation_set_digest"
    ):
        raise ValueError("semantic selection operation-set digest differs")
    _required_string(selection, "reviewed_on", "semantic selection")
    _required_string(selection, "review_ref", "semantic selection")


def _group_index(value: Any) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, list):
        raise ValueError("semantic review groups are malformed")
    result: dict[str, Mapping[str, Any]] = {}
    for row in value:
        if not isinstance(row, Mapping):
            raise ValueError("semantic review group is malformed")
        group_id = row.get("review_group_id")
        if not isinstance(group_id, str) or GROUP_ID_RE.fullmatch(group_id) is None:
            raise ValueError("semantic review group ID is invalid")
        if group_id in result:
            raise ValueError(f"{group_id}: duplicate semantic review group")
        result[group_id] = row
    return result


def _decision_index(value: Any) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, list):
        raise ValueError("semantic decisions are malformed")
    result: dict[str, Mapping[str, Any]] = {}
    for row in value:
        if not isinstance(row, Mapping):
            raise ValueError("semantic decision is malformed")
        group_id = row.get("review_group_id")
        decision = row.get("decision")
        if not isinstance(group_id, str) or GROUP_ID_RE.fullmatch(group_id) is None:
            raise ValueError("semantic decision group ID is invalid")
        expected = DECISION_FIELDS.get(decision)
        if expected is None or set(row) != expected:
            raise ValueError(f"{group_id}: semantic decision fields drift")
        if group_id in result:
            raise ValueError(f"{group_id}: duplicate semantic decision")
        result[group_id] = row
    return result


def _mapping(
    code_id: str,
    mappings: Mapping[str, Mapping[str, Any]],
    updated: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    row = mappings.get(code_id)
    if row is None:
        raise ValueError(f"{code_id}: semantic mapping is absent")
    if row.get("disposition") != "specified":
        raise ValueError(f"{code_id}: semantic decision requires specified mapping")
    return updated.setdefault(code_id, dict(row))


def _apply_cross_area_decision(
    *,
    group_id: str,
    finding: str,
    affected: list[str],
    decision: Mapping[str, Any],
    mappings: Mapping[str, Mapping[str, Any]],
    updated: dict[str, dict[str, Any]],
    review_ref: str,
) -> None:
    match = CROSS_FINDING_RE.fullmatch(finding)
    if match is None or INVARIANT_ID_RE.fullmatch(match.group(1)) is None:
        raise ValueError(f"{group_id}: cross-area finding is malformed")
    if decision.get("decision") != "accept_cross_area":
        raise ValueError(f"{group_id}: cross-area decision differs")
    invariant_id = match.group(1)
    for code_id in affected:
        row = _mapping(code_id, mappings, updated)
        invariant_ids = _string_array(
            row.get("invariant_ids"), f"{code_id} invariant IDs"
        )
        if invariant_id not in invariant_ids:
            raise ValueError(
                f"{group_id}: affected mapping does not name {invariant_id}"
            )
        existing = row.get("cross_area_decision_ref")
        if existing not in (None, review_ref):
            raise ValueError(
                f"{code_id}: cross-area decision has a different review"
            )
        row["cross_area_decision_ref"] = review_ref


def _apply_test_removal(
    *,
    group_id: str,
    finding: str,
    affected: list[str],
    decision: Mapping[str, Any],
    mappings: Mapping[str, Mapping[str, Any]],
    updated: dict[str, dict[str, Any]],
) -> None:
    match = TEST_FINDING_RE.fullmatch(finding)
    if match is None or TEST_ID_RE.fullmatch(match.group(1)) is None:
        raise ValueError(f"{group_id}: test relation finding is malformed")
    if decision.get("decision") != "remove_mapped_test":
        raise ValueError(f"{group_id}: test relation decision differs")
    test_id = _required_string(decision, "test_id", group_id)
    if test_id != match.group(1):
        raise ValueError(f"{group_id}: finding test differs from decision")
    for code_id in affected:
        row = _mapping(code_id, mappings, updated)
        tests = _string_array(row.get("test_ids"), f"{code_id} test IDs")
        if test_id not in tests:
            raise ValueError(
                f"{group_id}: affected mapping does not name {test_id}"
            )
        retained = [item for item in tests if item != test_id]
        if not retained:
            raise ValueError(f"{code_id}: semantic decision removes every test")
        row["test_ids"] = retained
