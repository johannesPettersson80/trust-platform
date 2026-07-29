"""Pure metadata projection for reviewed code-spec refresh acceptance."""

from __future__ import annotations

import re
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

from .code_spec_refresh_projection_support import (
    code_id_array as _code_id_array,
    digest as _digest,
    load_json_object as _load_json_object,
    object_digest as _object_digest,
    render_table as _render_table,
    replace_blocks as _replace_blocks,
    required_code_id as _required_code_id,
    required_digest as _required_digest,
    required_string as _required_string,
    string_array as _string_array,
    table_blocks as _table_blocks,
    unique_by as _unique_by,
    validate_projected_documents as _validate_projected_documents,
)


CODE_MAP_PATH = "verification/code-spec-map.toml"
RISK_ACK_PATH = "verification/code-spec-risk-acknowledgements.toml"
SPEC_GAPS_PATH = "verification/spec-gaps.toml"
PROPOSAL_ID_RE = re.compile(r"^CODEMAP_CAND_[0-9A-F]{64}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")

SELECTION_FIELDS = {
    "schema_version",
    "kind",
    "reviewed_on",
    "reviewer_ids",
    "review_ref",
    "risk_ack_expires_on",
    "source_proposal_sha256",
    "operation_set_digest",
    "missing_mapping_decisions",
    "digest_rebind_proposal_ids",
    "stale_identity_decisions",
}
MISSING_DECISION_FIELDS = {
    "spec_gap": {"proposal_id", "decision", "area", "owner", "spec_gap_ref"},
    "stale_successor": {
        "proposal_id",
        "decision",
        "stale_proposal_id",
    },
}
STALE_DECISION_FIELDS = {
    "remove": {"proposal_id", "decision"},
    "transfer": {"proposal_id", "decision", "successor_code_id"},
}
MAPPING_FIELD_ORDER = (
    "schema_version",
    "code_id",
    "source_kind",
    "path",
    "qualified_symbol",
    "item_kind",
    "signature_digest",
    "reviewed_body_digest",
    "owner",
    "area",
    "visibility",
    "risk_signals",
    "owner_ack",
    "owner_ack_expires_on",
    "disposition",
    "spec_ref",
    "spec_gap_ref",
    "invariant_ids",
    "test_ids",
    "parent_code_id",
    "generator_code_id",
    "generator_input_digest",
    "boundary_spec_ref",
    "non_behavioral_reason",
    "provenance",
    "rationale",
    "version_revision",
    "last_reviewed",
)
ACK_FIELD_ORDER = (
    "schema_version",
    "id",
    "code_id",
    "owner",
    "area",
    "body_digest",
    "risk_signals",
    "review_ref",
    "reviewed_by",
    "reviewed_on",
    "expires_on",
    "rationale",
)
GAP_FIELD_ORDER = (
    "schema_version",
    "id",
    "title",
    "area",
    "risk",
    "owner",
    "status",
    "gap_class",
    "blocking_question",
    "affected_invariants",
    "affected_tests",
    "affected_code_ids",
    "candidate_spec_sources",
    "resolution_status",
    "resolution_source_ref",
    "closeout_evidence",
    "last_reviewed",
)


@dataclass(frozen=True)
class RefreshProjection:
    """Projected canonical metadata replacements and exact review manifest."""

    replacements: dict[str, bytes]
    manifest: dict[str, Any]


def project_refresh_selection(
    *,
    proposal_bytes: bytes,
    selection_bytes: bytes,
    code_map_bytes: bytes,
    risk_acknowledgements_bytes: bytes,
    spec_gaps_bytes: bytes,
) -> RefreshProjection:
    """Project one complete reviewed identity-maintenance queue."""

    proposal = _load_json_object(proposal_bytes, "refresh proposal")
    selection = _load_json_object(selection_bytes, "refresh selection")
    _validate_selection_root(selection, proposal_bytes, proposal)

    code_blocks = _table_blocks(
        code_map_bytes, "mappings", "code_id", "code-spec map"
    )
    mappings = {block.identifier: block.row for block in code_blocks}
    if _digest(code_map_bytes) != proposal.get("source_code_map_sha256"):
        raise ValueError("refresh proposal code-map digest is stale")

    ack_blocks = _table_blocks(
        risk_acknowledgements_bytes,
        "acknowledgements",
        "id",
        "risk acknowledgement ledger",
    )
    acknowledgements = {block.identifier: block.row for block in ack_blocks}
    acknowledgements_by_code = _unique_by(
        acknowledgements.values(), "code_id", "risk acknowledgement ledger"
    )

    gap_blocks = _table_blocks(
        spec_gaps_bytes, "spec_gaps", "id", "spec-gap ledger"
    )
    gaps = {block.identifier: block.row for block in gap_blocks}

    missing = _proposal_index(
        proposal, "missing_mapping_proposals", "proposal_id"
    )
    rebinds = _proposal_index(
        proposal, "digest_rebind_proposals", "proposal_id"
    )
    stale = _proposal_index(
        proposal, "stale_identity_proposals", "proposal_id"
    )
    missing_decisions = _decision_index(
        selection.get("missing_mapping_decisions"),
        MISSING_DECISION_FIELDS,
        "missing mapping",
    )
    stale_decisions = _decision_index(
        selection.get("stale_identity_decisions"),
        STALE_DECISION_FIELDS,
        "stale identity",
    )
    rebind_ids = _string_array(
        selection.get("digest_rebind_proposal_ids"),
        "digest rebind proposal IDs",
        allow_empty=True,
    )
    if set(missing_decisions) != set(missing):
        raise ValueError("missing mapping review is partial")
    if set(stale_decisions) != set(stale):
        raise ValueError("stale identity review is partial")
    if set(rebind_ids) != set(rebinds) or len(rebind_ids) != len(set(rebind_ids)):
        raise ValueError("digest rebind review is partial")

    reviewed_on = _required_string(selection, "reviewed_on", "selection")
    review_ref = _required_string(selection, "review_ref", "selection")
    expires_on = _required_string(
        selection, "risk_ack_expires_on", "selection"
    )

    code_replacements: dict[str, bytes | None] = {}
    ack_replacements: dict[str, bytes | None] = {}
    gap_identity_replacements: dict[str, str | None] = {}
    successor_missing_ids: set[str] = set()
    transferred = 0
    removed = 0

    for proposal_id, stale_proposal in stale.items():
        decision = stale_decisions[proposal_id]
        stale_code_id = _required_code_id(
            stale_proposal, "stale_code_id", proposal_id
        )
        old_mapping = mappings.get(stale_code_id)
        if old_mapping is None:
            raise ValueError(f"{stale_code_id}: stale mapping is absent")
        _require_mapping_digest(stale_proposal, old_mapping, proposal_id)
        successors = _code_id_array(
            stale_proposal.get("exact_successor_code_ids"),
            f"{proposal_id} exact successors",
            allow_empty=True,
        )
        if decision["decision"] == "remove":
            if successors:
                raise ValueError(
                    f"{proposal_id}: removal has an exact successor"
                )
            code_replacements[stale_code_id] = None
            gap_identity_replacements[stale_code_id] = None
            _remove_ack_for_code(
                stale_code_id, acknowledgements_by_code, ack_replacements
            )
            removed += 1
            continue

        successor = _required_code_id(
            decision, "successor_code_id", proposal_id
        )
        if successors != [successor]:
            raise ValueError(
                f"{proposal_id}: transfer does not select the exact successor"
            )
        missing_decision = _successor_missing_decision(
            missing_decisions,
            missing,
            stale_proposal_id=proposal_id,
            successor_code_id=successor,
        )
        successor_proposal = missing[missing_decision["proposal_id"]]
        fact = _fact(successor_proposal)
        if old_mapping.get("area") not in fact["area_candidates"]:
            raise ValueError(
                f"{proposal_id}: predecessor area is not a live candidate"
            )
        transferred_row = _transfer_mapping(
            old_mapping,
            fact,
            reviewed_on=reviewed_on,
            expires_on=expires_on,
        )
        code_replacements[stale_code_id] = None
        code_replacements[successor] = _render_table(
            "mappings", transferred_row, MAPPING_FIELD_ORDER
        )
        gap_identity_replacements[stale_code_id] = successor
        successor_missing_ids.add(missing_decision["proposal_id"])
        _remove_ack_for_code(
            stale_code_id, acknowledgements_by_code, ack_replacements
        )
        _remove_ack_for_code(
            successor, acknowledgements_by_code, ack_replacements
        )
        _install_current_ack(
            transferred_row,
            acknowledgements,
            ack_replacements,
            reviewed_on=reviewed_on,
            review_ref=review_ref,
            expires_on=expires_on,
        )
        transferred += 1

    direct = 0
    direct_gap_additions: dict[str, set[str]] = {}
    for proposal_id, decision in missing_decisions.items():
        if proposal_id in successor_missing_ids:
            continue
        if decision["decision"] != "spec_gap":
            raise ValueError(
                f"{proposal_id}: stale successor decision was not consumed"
            )
        proposal_row = missing[proposal_id]
        fact = _fact(proposal_row)
        code_id = _required_code_id(proposal_row, "code_id", proposal_id)
        if code_id in mappings and code_id not in code_replacements:
            raise ValueError(f"{code_id}: direct missing mapping already exists")
        area = _required_string(decision, "area", proposal_id)
        if area not in fact["area_candidates"]:
            raise ValueError(f"{proposal_id}: area is not a live candidate")
        owner = _required_string(decision, "owner", proposal_id)
        gap_id = _required_string(decision, "spec_gap_ref", proposal_id)
        gap = gaps.get(gap_id)
        if gap is None or gap.get("area") != area:
            raise ValueError(
                f"{proposal_id}: spec-gap authority does not match area"
            )
        row = _direct_gap_mapping(
            fact,
            owner=owner,
            area=area,
            gap_id=gap_id,
            reviewed_on=reviewed_on,
            expires_on=expires_on,
        )
        code_replacements[code_id] = _render_table(
            "mappings", row, MAPPING_FIELD_ORDER
        )
        direct_gap_additions.setdefault(gap_id, set()).add(code_id)
        _remove_ack_for_code(
            code_id, acknowledgements_by_code, ack_replacements
        )
        _install_current_ack(
            row,
            acknowledgements,
            ack_replacements,
            reviewed_on=reviewed_on,
            review_ref=review_ref,
            expires_on=expires_on,
        )
        direct += 1

    for proposal_id in rebind_ids:
        proposal_row = rebinds[proposal_id]
        code_id = _required_code_id(proposal_row, "code_id", proposal_id)
        mapping = mappings.get(code_id)
        if mapping is None:
            raise ValueError(f"{code_id}: digest rebind mapping is absent")
        _require_mapping_digest(proposal_row, mapping, proposal_id)
        fact = _fact(proposal_row)
        old_digest = _required_digest(
            proposal_row, "old_body_digest", proposal_id
        )
        new_digest = _required_digest(
            proposal_row, "new_body_digest", proposal_id
        )
        if mapping.get("reviewed_body_digest") != old_digest:
            raise ValueError(f"{proposal_id}: reviewed body digest drift")
        if fact["body_digest"] != new_digest:
            raise ValueError(f"{proposal_id}: live body digest drift")
        row = dict(mapping)
        row["reviewed_body_digest"] = new_digest
        row["risk_signals"] = list(fact["risk_signals"])
        row["last_reviewed"] = reviewed_on
        _set_owner_ack_fields(row, expires_on)
        code_replacements[code_id] = _render_table(
            "mappings", row, MAPPING_FIELD_ORDER
        )
        _remove_ack_for_code(
            code_id, acknowledgements_by_code, ack_replacements
        )
        _install_current_ack(
            row,
            acknowledgements,
            ack_replacements,
            reviewed_on=reviewed_on,
            review_ref=review_ref,
            expires_on=expires_on,
        )

    gap_replacements: dict[str, bytes | None] = {}
    for gap_id, gap in gaps.items():
        affected = gap.get("affected_code_ids")
        if affected is None:
            affected_ids: list[str] = []
        else:
            affected_ids = _code_id_array(
                affected, f"{gap_id} affected code IDs", allow_empty=True
            )
        updated_ids = {
            replacement
            for code_id in affected_ids
            if (
                replacement := gap_identity_replacements.get(
                    code_id, code_id
                )
            )
            is not None
        }
        updated_ids.update(direct_gap_additions.get(gap_id, set()))
        if sorted(set(affected_ids)) != sorted(updated_ids):
            updated_gap = dict(gap)
            updated_gap["affected_code_ids"] = sorted(updated_ids)
            updated_gap["last_reviewed"] = reviewed_on
            gap_replacements[gap_id] = _render_table(
                "spec_gaps", updated_gap, GAP_FIELD_ORDER
            )

    code_after = _replace_blocks(
        code_map_bytes,
        code_blocks,
        code_replacements,
        append_order=sorted(
            code_id
            for code_id in code_replacements
            if code_id not in mappings and code_replacements[code_id] is not None
        ),
    )
    ack_after = _replace_blocks(
        risk_acknowledgements_bytes,
        ack_blocks,
        ack_replacements,
        append_order=sorted(
            ack_id
            for ack_id in ack_replacements
            if ack_id not in acknowledgements
            and ack_replacements[ack_id] is not None
        ),
    )
    gaps_after = _replace_blocks(
        spec_gaps_bytes,
        gap_blocks,
        gap_replacements,
        append_order=[],
    )
    _validate_projected_documents(code_after, ack_after, gaps_after)

    manifest = {
        "schema_version": 1,
        "kind": "p18_code_spec_refresh_acceptance_projection",
        "reviewed_on": reviewed_on,
        "reviewer_ids": list(selection["reviewer_ids"]),
        "review_ref": review_ref,
        "source_proposal_sha256": _digest(proposal_bytes),
        "selection_sha256": _digest(selection_bytes),
        "operation_set_digest": proposal["operation_set_digest"],
        "transferred_successors": transferred,
        "removed_stale_identities": removed,
        "digest_rebinds": len(rebind_ids),
        "direct_missing_mappings": direct,
        "code_map_before_sha256": _digest(code_map_bytes),
        "code_map_after_sha256": _digest(code_after),
        "risk_ledger_before_sha256": _digest(
            risk_acknowledgements_bytes
        ),
        "risk_ledger_after_sha256": _digest(ack_after),
        "spec_gaps_before_sha256": _digest(spec_gaps_bytes),
        "spec_gaps_after_sha256": _digest(gaps_after),
    }
    return RefreshProjection(
        replacements={
            CODE_MAP_PATH: code_after,
            RISK_ACK_PATH: ack_after,
            SPEC_GAPS_PATH: gaps_after,
        },
        manifest=manifest,
    )


def _validate_selection_root(
    selection: Mapping[str, Any],
    proposal_bytes: bytes,
    proposal: Mapping[str, Any],
) -> None:
    if set(selection) != SELECTION_FIELDS:
        raise ValueError("refresh selection root fields drift")
    if selection.get("schema_version") != 1 or selection.get("kind") != (
        "p18_code_spec_refresh_acceptance_selection"
    ):
        raise ValueError("refresh selection kind or version is invalid")
    _string_array(selection.get("reviewer_ids"), "reviewer IDs")
    for field in (
        "reviewed_on",
        "review_ref",
        "risk_ack_expires_on",
    ):
        _required_string(selection, field, "selection")
    if selection.get("source_proposal_sha256") != _digest(proposal_bytes):
        raise ValueError("refresh selection source proposal digest differs")
    operation_digest = selection.get("operation_set_digest")
    if (
        not isinstance(operation_digest, str)
        or DIGEST_RE.fullmatch(operation_digest) is None
        or operation_digest != proposal.get("operation_set_digest")
    ):
        raise ValueError("refresh selection operation-set digest differs")


def _transfer_mapping(
    old: Mapping[str, Any],
    fact: Mapping[str, Any],
    *,
    reviewed_on: str,
    expires_on: str,
) -> dict[str, Any]:
    row = dict(old)
    row.update(
        {
            "code_id": fact["code_id"],
            "source_kind": fact["source_kind"],
            "path": fact["path"],
            "qualified_symbol": fact["qualified_symbol"],
            "item_kind": fact["item_kind"],
            "signature_digest": fact["signature_digest"],
            "reviewed_body_digest": fact["body_digest"],
            "visibility": fact["visibility"],
            "risk_signals": list(fact["risk_signals"]),
            "last_reviewed": reviewed_on,
        }
    )
    _set_owner_ack_fields(row, expires_on)
    return row


def _direct_gap_mapping(
    fact: Mapping[str, Any],
    *,
    owner: str,
    area: str,
    gap_id: str,
    reviewed_on: str,
    expires_on: str,
) -> dict[str, Any]:
    row = {
        "schema_version": 1,
        "code_id": fact["code_id"],
        "source_kind": fact["source_kind"],
        "path": fact["path"],
        "qualified_symbol": fact["qualified_symbol"],
        "item_kind": fact["item_kind"],
        "signature_digest": fact["signature_digest"],
        "reviewed_body_digest": fact["body_digest"],
        "owner": owner,
        "area": area,
        "visibility": fact["visibility"],
        "risk_signals": list(fact["risk_signals"]),
        "disposition": "spec_gap",
        "spec_gap_ref": gap_id,
        "last_reviewed": reviewed_on,
    }
    _set_owner_ack_fields(row, expires_on)
    return row


def _set_owner_ack_fields(row: dict[str, Any], expires_on: str) -> None:
    risk_signals = row.get("risk_signals")
    if isinstance(risk_signals, list) and risk_signals:
        code_id = _required_code_id(row, "code_id", "mapping")
        row["owner_ack"] = "RISK_ACK_" + code_id.removeprefix("CODE_")
        row["owner_ack_expires_on"] = expires_on
    else:
        row.pop("owner_ack", None)
        row.pop("owner_ack_expires_on", None)


def _install_current_ack(
    mapping: Mapping[str, Any],
    acknowledgements: Mapping[str, Mapping[str, Any]],
    replacements: dict[str, bytes | None],
    *,
    reviewed_on: str,
    review_ref: str,
    expires_on: str,
) -> None:
    risks = mapping.get("risk_signals")
    if not isinstance(risks, list) or not risks:
        return
    ack_id = _required_string(mapping, "owner_ack", "mapping")
    code_id = _required_code_id(mapping, "code_id", "mapping")
    owner = _required_string(mapping, "owner", code_id)
    area = _required_string(mapping, "area", code_id)
    row = {
        "schema_version": 1,
        "id": ack_id,
        "code_id": code_id,
        "owner": owner,
        "area": area,
        "body_digest": _required_digest(
            mapping, "reviewed_body_digest", code_id
        ),
        "risk_signals": sorted(set(risks)),
        "review_ref": review_ref,
        "reviewed_by": owner,
        "reviewed_on": reviewed_on,
        "expires_on": expires_on,
        "rationale": (
            "Acknowledges the live scanner risk signals for this exact body "
            "digest and reviewed identity-maintenance transition; it does not "
            "specify behavior, suppress risk, or create product proof."
        ),
    }
    existing = acknowledgements.get(ack_id)
    if existing is not None and existing.get("code_id") not in {
        code_id,
    }:
        raise ValueError(f"{ack_id}: acknowledgement ID collision")
    replacements[ack_id] = _render_table(
        "acknowledgements", row, ACK_FIELD_ORDER
    )


def _remove_ack_for_code(
    code_id: str,
    acknowledgements_by_code: Mapping[str, Mapping[str, Any]],
    replacements: dict[str, bytes | None],
) -> None:
    acknowledgement = acknowledgements_by_code.get(code_id)
    if acknowledgement is None:
        return
    ack_id = _required_string(
        acknowledgement, "id", f"{code_id} acknowledgement"
    )
    replacements[ack_id] = None


def _successor_missing_decision(
    decisions: Mapping[str, Mapping[str, Any]],
    proposals: Mapping[str, Mapping[str, Any]],
    *,
    stale_proposal_id: str,
    successor_code_id: str,
) -> Mapping[str, Any]:
    matches = [
        decision
        for proposal_id, decision in decisions.items()
        if decision["decision"] == "stale_successor"
        and decision.get("stale_proposal_id") == stale_proposal_id
        and proposals[proposal_id].get("code_id") == successor_code_id
    ]
    if len(matches) != 1:
        raise ValueError(
            f"{stale_proposal_id}: exact successor missing decision is absent"
        )
    return matches[0]


def _fact(proposal: Mapping[str, Any]) -> dict[str, Any]:
    fact = proposal.get("fact")
    if not isinstance(fact, Mapping):
        raise ValueError("refresh proposal fact is malformed")
    required = {
        "code_id",
        "source_kind",
        "path",
        "qualified_symbol",
        "item_kind",
        "signature_digest",
        "body_digest",
        "owner",
        "area",
        "area_candidates",
        "visibility",
        "risk_signals",
    }
    if not required.issubset(fact):
        raise ValueError("refresh proposal fact fields are incomplete")
    _required_code_id(fact, "code_id", "fact")
    _required_digest(fact, "signature_digest", "fact")
    _required_digest(fact, "body_digest", "fact")
    area_candidates = _string_array(
        fact.get("area_candidates"), "fact area candidates"
    )
    risks = _string_array(
        fact.get("risk_signals"), "fact risk signals", allow_empty=True
    )
    return {
        **dict(fact),
        "area_candidates": area_candidates,
        "risk_signals": risks,
    }


def _require_mapping_digest(
    proposal: Mapping[str, Any],
    mapping: Mapping[str, Any],
    label: str,
) -> None:
    expected = proposal.get("existing_mapping_sha256")
    if expected != _object_digest(mapping):
        raise ValueError(f"{label}: existing mapping digest differs")


def _proposal_index(
    proposal: Mapping[str, Any], field: str, id_field: str
) -> dict[str, Mapping[str, Any]]:
    rows = proposal.get(field)
    if not isinstance(rows, list) or any(
        not isinstance(row, Mapping) for row in rows
    ):
        raise ValueError(f"refresh proposal {field} is malformed")
    return _unique_by(rows, id_field, field)


def _decision_index(
    rows: Any,
    fields_by_decision: Mapping[str, set[str]],
    label: str,
) -> dict[str, Mapping[str, Any]]:
    if not isinstance(rows, list) or any(
        not isinstance(row, Mapping) for row in rows
    ):
        raise ValueError(f"{label} decisions are malformed")
    result = {}
    for index, row in enumerate(rows):
        decision = row.get("decision")
        expected_fields = fields_by_decision.get(str(decision))
        if expected_fields is None or set(row) != expected_fields:
            raise ValueError(f"{label} decision {index} fields drift")
        proposal_id = _required_string(
            row, "proposal_id", f"{label} decision {index}"
        )
        if PROPOSAL_ID_RE.fullmatch(proposal_id) is None:
            raise ValueError(f"{label} decision {index} proposal ID is invalid")
        if proposal_id in result:
            raise ValueError(f"{label} decisions duplicate {proposal_id}")
        result[proposal_id] = row
    return result
