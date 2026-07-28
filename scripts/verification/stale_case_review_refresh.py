"""Rebind unchanged accepted assertion reviews to refreshed source closure."""

from __future__ import annotations

import copy
import re
import tomllib
from collections.abc import Mapping, Sequence
from typing import Any

from .case_backed_behavior_proof import replace_table_record_fields
from .mapping_proposals import proposal_payload_digest
from .mapping_review import build_mapping_decision
from .mapping_toml import render_decisions_toml


def refresh_source_closure_reviews(
    *,
    catalog_source: str,
    decisions_source: str,
    inventory_document: Mapping[str, Any],
    test_ids: Sequence[str],
    source_commit: str,
    expected_count: int,
) -> tuple[str, str, list[str]]:
    """Rebind unchanged reviewed assertions to their current source closure."""

    if not re.fullmatch(r"[0-9a-f]{40}", source_commit):
        raise ValueError("source-closure review source commit is not a full SHA")
    if inventory_document.get("source_commit") != source_commit:
        raise ValueError("source-closure inventory source commit drifted")
    selected_test_ids = _strings(
        list(test_ids), "source-closure review test IDs"
    )
    if len(selected_test_ids) != expected_count:
        raise ValueError("source-closure review population drifted")

    catalog_document = tomllib.loads(catalog_source)
    catalog_tests = {
        _string(row, "id", "source-closure catalog test"): row
        for row in _records(
            catalog_document.get("tests"), "source-closure catalog tests"
        )
    }
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("source-closure inventory artifact lacks inventories")
    selected_discovery_ids = {
        _string(
            catalog_tests[test_id],
            "discovery_id",
            f"{test_id} catalog test",
        )
        for test_id in selected_test_ids
    }
    if (
        inventory_document.get("inventory_count") != expected_count
        or set(inventories) != selected_discovery_ids
    ):
        raise ValueError("source-closure inventory population drifted")

    decision_document = tomllib.loads(decisions_source)
    snapshots = {
        _string(row, "id", "mapping proposal snapshot"): copy.deepcopy(
            dict(row)
        )
        for row in _records(
            decision_document.get("proposal_snapshots"),
            "mapping proposal snapshots",
        )
    }
    decisions = {
        _string(row, "id", "mapping decision"): copy.deepcopy(dict(row))
        for row in _records(
            decision_document.get("decisions"), "mapping decisions"
        )
    }
    catalog_updates: dict[str, dict[str, Any]] = {}
    refreshed_ids: list[str] = []
    for test_id in sorted(selected_test_ids):
        test = catalog_tests.get(test_id)
        if test is None:
            raise ValueError(f"source-closure catalog lacks {test_id}")
        discovery_id = _string(
            test, "discovery_id", f"{test_id} catalog test"
        )
        inventory = inventories.get(discovery_id)
        if not isinstance(inventory, Mapping):
            raise ValueError(f"{test_id} lacks current assertion inventory")
        proposal_id = _string(
            test, "mapping_proposal_id", f"{test_id} catalog test"
        )
        decision_id = _string(
            test, "mapping_decision_id", f"{test_id} catalog test"
        )
        proposal = snapshots.get(proposal_id)
        decision = decisions.get(decision_id)
        if proposal is None or decision is None:
            raise ValueError(f"{test_id} lacks its accepted review records")
        if (
            proposal.get("discovery_id") != discovery_id
            or decision.get("proposal_id") != proposal_id
            or decision.get("proposal_digest")
            != proposal.get("proposal_digest")
            or decision.get("outcome") != "accepted"
        ):
            raise ValueError(f"{test_id} accepted review binding drifted")
        if (
            proposal.get("path") != inventory.get("path")
            or proposal.get("name") != inventory.get("test_name")
            or proposal.get("assertions") != inventory.get("assertions")
            or proposal.get("unresolved_assertion_helpers")
            != inventory.get("unresolved_assertion_helpers")
        ):
            raise ValueError(
                f"{test_id} assertion semantics changed during closure refresh"
            )
        source_digest = _string(
            inventory, "source_digest", f"{test_id} assertion inventory"
        )
        inventory_digest = _string(
            inventory, "inventory_digest", f"{test_id} assertion inventory"
        )
        source_digests = inventory.get("source_digests")
        if not isinstance(source_digests, Mapping) or any(
            not isinstance(path, str) or not isinstance(digest, str)
            for path, digest in source_digests.items()
        ):
            raise ValueError(f"{test_id} assertion source closure drifted")
        if (
            test.get("mapping_reviewed_source_digest") != source_digest
            or test.get("mapping_assertion_source_digests")
            != source_digests
        ):
            raise ValueError(
                f"{test_id} catalog is not bound to current assertion closure"
            )

        refreshed_proposal = copy.deepcopy(proposal)
        refreshed_proposal.update(
            {
                "test_source_digest": source_digest,
                "assertion_inventory_digest": inventory_digest,
                "assertion_source_digests": dict(source_digests),
                "assertions": copy.deepcopy(inventory["assertions"]),
                "unresolved_assertion_helpers": list(
                    inventory["unresolved_assertion_helpers"]
                ),
            }
        )
        proposal_digest = proposal_payload_digest(refreshed_proposal)
        refreshed_proposal["proposal_digest"] = proposal_digest
        refreshed_proposal["id"] = (
            f"MAP_CAND_{discovery_id.removeprefix('DISC_')}_"
            f"{proposal_digest.removeprefix('sha256:')[:12].upper()}"
        )

        review_input = {
            "proposal_id": refreshed_proposal["id"],
            "outcome": "accepted",
            "reviewer_ids": copy.deepcopy(decision["reviewer_ids"]),
            "assertion_reviews": copy.deepcopy(decision["assertion_reviews"]),
            "selected_invariant_ids": copy.deepcopy(
                decision["selected_invariant_ids"]
            ),
            "selected_behavior_refs": copy.deepcopy(
                decision["selected_behavior_refs"]
            ),
            "command": decision["command"],
            "command_authority": decision["command_authority"],
            "confirmed_catalog": copy.deepcopy(
                decision["confirmed_catalog"]
            ),
        }
        refreshed_decision = build_mapping_decision(
            proposal=refreshed_proposal,
            review_input=review_input,
        )
        if (
            refreshed_decision["assertion_reviews"]
            != decision["assertion_reviews"]
            or refreshed_decision["selected_invariant_ids"]
            != decision["selected_invariant_ids"]
            or refreshed_decision["selected_behavior_refs"]
            != decision["selected_behavior_refs"]
            or refreshed_decision["confirmed_catalog"]
            != decision["confirmed_catalog"]
        ):
            raise ValueError(
                f"{test_id} accepted review semantics changed during refresh"
            )

        del snapshots[proposal_id]
        del decisions[decision_id]
        new_proposal_id = _string(
            refreshed_proposal, "id", f"{test_id} refreshed proposal"
        )
        new_decision_id = _string(
            refreshed_decision, "id", f"{test_id} refreshed decision"
        )
        if new_proposal_id in snapshots or new_decision_id in decisions:
            raise ValueError(f"{test_id} refreshed review identity collides")
        snapshots[new_proposal_id] = refreshed_proposal
        decisions[new_decision_id] = refreshed_decision
        catalog_updates[test_id] = {
            "mapping_proposal_id": new_proposal_id,
            "mapping_proposal_digest": proposal_digest,
            "mapping_decision_id": new_decision_id,
            "mapping_reviewed_source_digest": source_digest,
            "mapping_assertion_source_digests": dict(source_digests),
        }
        refreshed_ids.append(test_id)

    updated_catalog_source = replace_table_record_fields(
        catalog_source,
        table="tests",
        updates=catalog_updates,
    )
    updated_decision_document = {
        **dict(decision_document),
        "source_commit": source_commit,
        "proposal_snapshots": list(snapshots.values()),
        "decisions": list(decisions.values()),
    }
    return (
        updated_catalog_source,
        render_decisions_toml(updated_decision_document),
        refreshed_ids,
    )


def _records(value: Any, label: str) -> list[Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an object array")
    if any(not isinstance(row, Mapping) for row in value):
        raise ValueError(f"{label} must contain only objects")
    return list(value)


def _strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be a string array")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value) or len(result) != len(set(result)):
        raise ValueError(f"{label} must contain distinct non-empty strings")
    return result


def _string(record: Mapping[str, Any], field: str, label: str) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value
