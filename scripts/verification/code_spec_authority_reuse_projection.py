"""Pure projection for reviewed reuse of existing code-spec authority."""

from __future__ import annotations

import re
import tomllib
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

from .code_spec_authority_reuse import _object_digest
from .code_spec_refresh_projection import (
    CODE_MAP_PATH,
    GAP_FIELD_ORDER,
    MAPPING_FIELD_ORDER,
    SPEC_GAPS_PATH,
)
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


GROUP_ID_RE = re.compile(r"^CODEAUTH_GROUP_[0-9A-F]{64}$")
INVARIANT_ID_RE = re.compile(r"^[A-Z][A-Z0-9_]+$")
SELECTION_FIELDS = {
    "schema_version",
    "kind",
    "reviewed_on",
    "reviewer_ids",
    "review_ref",
    "source_proposal_sha256",
    "operation_set_digest",
    "decisions",
}
DECISION_FIELDS = {
    "accept": {"candidate_group_id", "decision", "invariant_id"},
    "withhold": {"candidate_group_id", "decision", "reason_code"},
}
WITHHOLD_REASONS = {
    "ambiguous_authority",
    "claim_mismatch",
    "mixed_semantics",
    "needs_new_authority",
}


@dataclass(frozen=True)
class AuthorityReuseProjection:
    """Canonical metadata replacements and bound semantic-review manifest."""

    replacements: dict[str, bytes]
    manifest: dict[str, Any]


def project_authority_reuse_selection(
    *,
    proposal_bytes: bytes,
    selection_bytes: bytes,
    code_map_bytes: bytes,
    spec_gaps_bytes: bytes,
) -> AuthorityReuseProjection:
    """Apply only complete, explicit decisions to the proposal plane."""

    proposal = _load_json_object(proposal_bytes, "authority-reuse proposal")
    selection = _load_json_object(
        selection_bytes, "authority-reuse selection"
    )
    _validate_proposal(proposal)
    _validate_selection(selection, proposal, proposal_bytes)
    if _digest(code_map_bytes) != proposal["source_code_map_sha256"]:
        raise ValueError("authority-reuse proposal code-map digest is stale")
    if _digest(spec_gaps_bytes) != proposal["source_spec_gaps_sha256"]:
        raise ValueError("authority-reuse proposal gap-ledger digest is stale")

    code_blocks = _table_blocks(
        code_map_bytes, "mappings", "code_id", "code-spec map"
    )
    mappings = {block.identifier: block.row for block in code_blocks}
    gap_blocks = _table_blocks(
        spec_gaps_bytes, "spec_gaps", "id", "spec-gap ledger"
    )
    gaps = {block.identifier: block.row for block in gap_blocks}
    groups = _group_index(proposal["candidate_groups"])
    invariants = _invariant_index(proposal["candidate_invariants"])
    decisions = _decision_index(selection["decisions"])
    if set(decisions) != set(groups):
        raise ValueError("authority-reuse review is partial")

    reviewed_on = _required_string(
        selection, "reviewed_on", "authority-reuse selection"
    )
    code_replacements: dict[str, bytes] = {}
    removals_by_gap: dict[str, set[str]] = {}
    accepted_groups = 0
    withheld_groups = 0
    accepted_facts = 0
    for group_id, group in groups.items():
        decision = decisions[group_id]
        if decision["decision"] == "withhold":
            reason = _required_string(decision, "reason_code", group_id)
            if reason not in WITHHOLD_REASONS:
                raise ValueError(f"{group_id}: unsupported withhold reason")
            withheld_groups += 1
            continue
        if group.get("classification") != "singular":
            raise ValueError(f"{group_id}: ambiguous authority cannot be accepted")
        candidate_ids = _string_array(
            group.get("candidate_invariant_ids"),
            f"{group_id} candidate invariant IDs",
        )
        invariant_id = _required_string(
            decision, "invariant_id", group_id
        )
        if (
            INVARIANT_ID_RE.fullmatch(invariant_id) is None
            or candidate_ids != [invariant_id]
        ):
            raise ValueError(
                f"{group_id}: accepted invariant differs from singular candidate"
            )
        invariant = invariants.get(invariant_id)
        if invariant is None:
            raise ValueError(f"{group_id}: candidate invariant is absent")
        spec_ref = _required_string(invariant, "spec_ref", invariant_id)
        test_ids = _string_array(
            invariant.get("test_ids"), f"{invariant_id} test IDs"
        )
        facts = group.get("facts")
        if not isinstance(facts, list) or not facts:
            raise ValueError(f"{group_id}: candidate facts are malformed")
        for fact in facts:
            if not isinstance(fact, Mapping):
                raise ValueError(f"{group_id}: candidate fact is malformed")
            code_id = _required_string(fact, "code_id", group_id)
            mapping = mappings.get(code_id)
            if mapping is None or mapping.get("disposition") != "spec_gap":
                raise ValueError(f"{code_id}: current mapping is not spec_gap")
            gap_id = _required_string(mapping, "spec_gap_ref", code_id)
            if gap_id != fact.get("spec_gap_ref"):
                raise ValueError(f"{code_id}: candidate gap reference drift")
            if (
                mapping.get("area") != group.get("area")
                or mapping.get("path") != group.get("path")
                or mapping.get("qualified_symbol")
                != fact.get("qualified_symbol")
                or mapping.get("item_kind") != fact.get("item_kind")
                or mapping.get("owner") != fact.get("owner")
                or sorted(mapping.get("risk_signals", []))
                != sorted(fact.get("risk_signals", []))
            ):
                raise ValueError(f"{code_id}: candidate fact differs from map")
            updated = dict(mapping)
            updated.pop("spec_gap_ref", None)
            updated.update(
                {
                    "disposition": "specified",
                    "spec_ref": spec_ref,
                    "invariant_ids": [invariant_id],
                    "test_ids": test_ids,
                    "last_reviewed": reviewed_on,
                }
            )
            code_replacements[code_id] = _render_table(
                "mappings", updated, MAPPING_FIELD_ORDER
            )
            removals_by_gap.setdefault(gap_id, set()).add(code_id)
            accepted_facts += 1
        accepted_groups += 1

    gap_replacements: dict[str, bytes] = {}
    for gap_id, removed_ids in sorted(removals_by_gap.items()):
        gap = gaps.get(gap_id)
        if gap is None:
            raise ValueError(f"{gap_id}: owning gap is absent")
        affected = _code_id_array(
            gap.get("affected_code_ids"),
            f"{gap_id} affected code IDs",
            allow_empty=True,
        )
        if not removed_ids.issubset(affected):
            raise ValueError(f"{gap_id}: accepted fact is absent from gap ledger")
        updated = dict(gap)
        updated["affected_code_ids"] = sorted(
            set(affected).difference(removed_ids)
        )
        updated["last_reviewed"] = reviewed_on
        gap_replacements[gap_id] = _render_table(
            "spec_gaps", updated, GAP_FIELD_ORDER
        )

    code_after = _replace_blocks(
        code_map_bytes,
        code_blocks,
        code_replacements,
        append_order=[],
    )
    gaps_after = _replace_blocks(
        spec_gaps_bytes,
        gap_blocks,
        gap_replacements,
        append_order=[],
    )
    _validate_denominators(
        before_code=mappings,
        after_code=code_after,
        before_gaps=gaps,
        after_gaps=gaps_after,
    )
    manifest = {
        "schema_version": 1,
        "kind": "p18_code_spec_authority_reuse_projection",
        "reviewed_on": reviewed_on,
        "reviewer_ids": list(selection["reviewer_ids"]),
        "review_ref": selection["review_ref"],
        "source_proposal_sha256": _digest(proposal_bytes),
        "selection_sha256": _digest(selection_bytes),
        "operation_set_digest": proposal["operation_set_digest"],
        "candidate_groups": len(groups),
        "accepted_groups": accepted_groups,
        "withheld_groups": withheld_groups,
        "accepted_code_mappings": accepted_facts,
        "code_map_before_sha256": _digest(code_map_bytes),
        "code_map_after_sha256": _digest(code_after),
        "spec_gaps_before_sha256": _digest(spec_gaps_bytes),
        "spec_gaps_after_sha256": _digest(gaps_after),
    }
    return AuthorityReuseProjection(
        replacements={
            CODE_MAP_PATH: code_after,
            SPEC_GAPS_PATH: gaps_after,
        },
        manifest=manifest,
    )


def _validate_proposal(proposal: Mapping[str, Any]) -> None:
    expected_fields = {
        "schema_version",
        "kind",
        "generator",
        "generator_version",
        "mode",
        "source_commit",
        "source_report",
        "source_code_map_sha256",
        "source_spec_gaps_sha256",
        "source_bindings",
        "boundaries",
        "summary",
        "candidate_invariants",
        "candidate_groups",
        "no_candidate_code_ids",
        "operation_set_digest",
    }
    if set(proposal) != expected_fields:
        raise ValueError("authority-reuse proposal root fields drift")
    if (
        proposal.get("schema_version") != 1
        or proposal.get("kind")
        != "p18_code_spec_authority_reuse_proposal"
        or proposal.get("generator") != "code-spec-authority-reuse"
        or proposal.get("generator_version") != 1
        or proposal.get("mode") != "proposal_only"
    ):
        raise ValueError("authority-reuse proposal identity differs")
    groups = proposal.get("candidate_groups")
    invariants = proposal.get("candidate_invariants")
    no_candidate = proposal.get("no_candidate_code_ids")
    if (
        not isinstance(groups, list)
        or not groups
        or not isinstance(invariants, list)
        or not isinstance(no_candidate, list)
    ):
        raise ValueError("authority-reuse proposal plane is malformed")
    expected_digest = _object_digest(
        {
            "candidate_groups": groups,
            "candidate_invariants": invariants,
            "no_candidate_code_ids": no_candidate,
        }
    )
    if proposal.get("operation_set_digest") != expected_digest:
        raise ValueError("authority-reuse operation-set digest differs")
    summary = proposal.get("summary")
    if not isinstance(summary, Mapping):
        raise ValueError("authority-reuse proposal summary is malformed")
    singular = sum(
        len(group.get("facts", []))
        for group in groups
        if isinstance(group, Mapping)
        and group.get("classification") == "singular"
    )
    ambiguous = sum(
        len(group.get("facts", []))
        for group in groups
        if isinstance(group, Mapping)
        and group.get("classification") == "ambiguous"
    )
    expected = {
        "candidate_groups": len(groups),
        "candidate_facts": singular + ambiguous,
        "singular_candidate_facts": singular,
        "ambiguous_candidate_facts": ambiguous,
        "no_candidate_facts": len(no_candidate),
    }
    for field, value in expected.items():
        if summary.get(field) != value:
            raise ValueError(f"authority-reuse summary {field} differs")
    _required_string(
        proposal, "source_code_map_sha256", "authority-reuse proposal"
    )
    _required_string(
        proposal, "source_spec_gaps_sha256", "authority-reuse proposal"
    )


def _validate_selection(
    selection: Mapping[str, Any],
    proposal: Mapping[str, Any],
    proposal_bytes: bytes,
) -> None:
    if set(selection) != SELECTION_FIELDS:
        raise ValueError("authority-reuse selection root fields drift")
    if (
        selection.get("schema_version") != 1
        or selection.get("kind")
        != "p18_code_spec_authority_reuse_selection"
    ):
        raise ValueError("authority-reuse selection identity differs")
    reviewers = _string_array(
        selection.get("reviewer_ids"), "authority-reuse reviewer IDs"
    )
    if len(reviewers) != len(set(reviewers)):
        raise ValueError("authority-reuse reviewer IDs contain duplicates")
    review_ref = _required_string(
        selection, "review_ref", "authority-reuse selection"
    )
    if "#" not in review_ref:
        raise ValueError("authority-reuse review_ref requires an anchor")
    _required_string(
        selection, "reviewed_on", "authority-reuse selection"
    )
    if selection.get("source_proposal_sha256") != _digest(proposal_bytes):
        raise ValueError("authority-reuse proposal digest differs")
    if selection.get("operation_set_digest") != proposal.get(
        "operation_set_digest"
    ):
        raise ValueError("authority-reuse operation-set digest differs")


def _group_index(value: Any) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, list):
        raise ValueError("authority-reuse candidate groups are malformed")
    result: dict[str, Mapping[str, Any]] = {}
    for row in value:
        if not isinstance(row, Mapping):
            raise ValueError("authority-reuse candidate group is malformed")
        group_id = row.get("candidate_group_id")
        if (
            not isinstance(group_id, str)
            or GROUP_ID_RE.fullmatch(group_id) is None
        ):
            raise ValueError("authority-reuse candidate group ID is invalid")
        if group_id in result:
            raise ValueError(f"{group_id}: duplicate candidate group")
        result[group_id] = row
    return result


def _invariant_index(value: Any) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, list):
        raise ValueError("authority-reuse candidate invariants are malformed")
    result: dict[str, Mapping[str, Any]] = {}
    for row in value:
        if not isinstance(row, Mapping):
            raise ValueError("authority-reuse candidate invariant is malformed")
        invariant_id = row.get("invariant_id")
        if (
            not isinstance(invariant_id, str)
            or INVARIANT_ID_RE.fullmatch(invariant_id) is None
            or invariant_id in result
        ):
            raise ValueError("authority-reuse candidate invariant ID is invalid")
        result[invariant_id] = row
    return result


def _decision_index(value: Any) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, list):
        raise ValueError("authority-reuse decisions are malformed")
    result: dict[str, Mapping[str, Any]] = {}
    for row in value:
        if not isinstance(row, Mapping):
            raise ValueError("authority-reuse decision is malformed")
        group_id = row.get("candidate_group_id")
        decision = row.get("decision")
        expected = DECISION_FIELDS.get(decision)
        if (
            not isinstance(group_id, str)
            or GROUP_ID_RE.fullmatch(group_id) is None
            or expected is None
            or set(row) != expected
            or group_id in result
        ):
            raise ValueError("authority-reuse decision fields drift")
        result[group_id] = row
    return result


def _validate_denominators(
    *,
    before_code: Mapping[str, Any],
    after_code: bytes,
    before_gaps: Mapping[str, Any],
    after_gaps: bytes,
) -> None:
    code_doc = tomllib.loads(after_code.decode())
    code_rows = code_doc.get("mappings")
    gap_doc = tomllib.loads(after_gaps.decode())
    gap_rows = gap_doc.get("spec_gaps")
    if not isinstance(code_rows, list) or {
        row.get("code_id") for row in code_rows if isinstance(row, Mapping)
    } != set(before_code):
        raise ValueError("authority-reuse code-map denominator drifted")
    if not isinstance(gap_rows, list) or {
        row.get("id") for row in gap_rows if isinstance(row, Mapping)
    } != set(before_gaps):
        raise ValueError("authority-reuse gap-ledger denominator drifted")
