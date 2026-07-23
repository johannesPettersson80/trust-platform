"""Validation boundary for execution-guided mapping control-plane documents."""

from __future__ import annotations

import json
import hashlib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from ..mapping_proposals import validate_mapping_quarantine
from ..mapping_review import validate_mapping_decisions
from ..test_catalog_json_schema import validate_json_schema_instance


PROPOSAL_SCHEMA_PATH = Path("verification/schemas/test-catalog-proposals.schema.json")
DECISION_SCHEMA_PATH = Path(
    "verification/schemas/test-catalog-mapping-decisions.schema.json"
)
PROPOSAL_PATH = Path("verification/test-catalog-proposals.toml")
DECISION_PATH = Path("verification/test-catalog-mapping-decisions.toml")
CATALOG_PROVENANCE_FIELDS = frozenset(
    {
        "mapping_proposal_id",
        "mapping_proposal_digest",
        "mapping_decision_id",
    }
)


def mapping_control_plane_state(
    root: Path,
) -> tuple[bool, list[tuple[Path, str]]]:
    """Allow an uninitialized pair, but reject every one-sided control plane."""

    proposal_exists = (root / PROPOSAL_PATH).is_file()
    decision_exists = (root / DECISION_PATH).is_file()
    if proposal_exists == decision_exists:
        return proposal_exists, []
    missing = DECISION_PATH if proposal_exists else PROPOSAL_PATH
    present = PROPOSAL_PATH if proposal_exists else DECISION_PATH
    return False, [
        (
            missing,
            f"mapping control plane is incomplete: {present} exists without {missing}",
        )
    ]


def validate_mapping_control_plane(
    *,
    root: Path,
    proposal_document: Mapping[str, Any],
    decision_document: Mapping[str, Any],
    facts: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    denominator_reviews: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    failures: list[str] = []
    proposal_schema = _load_schema(root / PROPOSAL_SCHEMA_PATH, failures)
    decision_schema = _load_schema(root / DECISION_SCHEMA_PATH, failures)
    if proposal_schema is not None:
        failures.extend(
            f"proposal document {failure}"
            for failure in validate_json_schema_instance(
                dict(proposal_document), proposal_schema
            )
        )
    if decision_schema is not None:
        failures.extend(
            f"decision document {failure}"
            for failure in validate_json_schema_instance(
                dict(decision_document), decision_schema
            )
        )

    proposals = _mapping_records(
        proposal_document.get("proposals"),
        label="proposal",
        id_field="id",
        failures=failures,
    )
    snapshots = _mapping_records(
        decision_document.get("proposal_snapshots"),
        label="proposal snapshot",
        id_field="id",
        failures=failures,
    )
    decisions = _mapping_records(
        decision_document.get("decisions"),
        label="decision",
        id_field="id",
        failures=failures,
    )
    if proposal_schema is not None:
        snapshot_schema = {
            "$ref": "#/$defs/proposal",
            "$defs": proposal_schema.get("$defs", {}),
        }
        for snapshot_id, snapshot in snapshots.items():
            failures.extend(
                f"{snapshot_id} snapshot {failure}"
                for failure in validate_json_schema_instance(
                    dict(snapshot), snapshot_schema
                )
            )
            _validate_snapshot_staleness(
                root=root,
                snapshot_id=snapshot_id,
                snapshot=snapshot,
                fact=facts.get(str(snapshot.get("discovery_id"))),
                failures=failures,
            )
    failures.extend(
        validate_mapping_quarantine(
            root=root,
            proposals=list(proposals.values()),
            facts=facts,
            tests=tests,
            denominator_reviews=denominator_reviews,
            invariants=invariants,
        )
    )
    failures.extend(
        validate_mapping_decisions(
            decisions=list(decisions.values()),
            proposals=snapshots,
            invariants=invariants,
            candidate_generator_id=str(
                proposal_document.get("generator", "test-catalog-proposals")
            ),
        )
    )
    for proposal_id in sorted(set(proposals) & set(snapshots)):
        failures.append(
            f"{proposal_id} cannot be both pending quarantine and reviewed snapshot"
        )
    decision_proposal_ids = {
        str(decision.get("proposal_id"))
        for decision in decisions.values()
        if isinstance(decision.get("proposal_id"), str)
    }
    for snapshot_id in sorted(set(snapshots) - decision_proposal_ids):
        failures.append(f"{snapshot_id} historical snapshot lacks review decision")
    for proposal_id in sorted(decision_proposal_ids - set(snapshots)):
        failures.append(f"{proposal_id} review decision lacks historical snapshot")
    decisions_by_id = {
        decision_id: decision
        for decision_id, decision in decisions.items()
        if decision.get("outcome") == "accepted"
    }
    accepted_by_discovery: dict[str, Mapping[str, Any]] = {}
    for decision in decisions_by_id.values():
        discovery_id = decision.get("discovery_id")
        if isinstance(discovery_id, str):
            if discovery_id in accepted_by_discovery:
                failures.append(
                    f"multiple accepted review decisions classify {discovery_id}"
                )
            accepted_by_discovery[discovery_id] = decision

    catalog_by_discovery: dict[str, Mapping[str, Any]] = {}
    for test_id, test in tests.items():
        discovery_id = test.get("discovery_id")
        if (
            test.get("subject_kind") == "generated_test"
            and isinstance(discovery_id, str)
        ):
            catalog_by_discovery[discovery_id] = test
        present = CATALOG_PROVENANCE_FIELDS & set(test)
        if not present:
            continue
        if present != CATALOG_PROVENANCE_FIELDS:
            failures.append(
                f"{test_id} mapping review provenance fields are incomplete"
            )
            continue
        decision = decisions_by_id.get(str(test.get("mapping_decision_id")))
        if decision is None:
            failures.append(
                f"{test_id} was catalog accepted without an accepted review decision"
            )
            continue
        for catalog_field, decision_field in (
            ("discovery_id", "discovery_id"),
            ("mapping_proposal_id", "proposal_id"),
            ("mapping_proposal_digest", "proposal_digest"),
        ):
            if test.get(catalog_field) != decision.get(decision_field):
                failures.append(
                    f"{test_id} {catalog_field} does not match its accepted decision"
                )

    for discovery_id, decision in accepted_by_discovery.items():
        test = catalog_by_discovery.get(discovery_id)
        if test is None:
            failures.append(
                f"accepted review decision {decision.get('id')} lacks catalog mapping"
            )
        elif test.get("mapping_decision_id") != decision.get("id"):
            failures.append(
                f"accepted review decision {decision.get('id')} is not bound by its catalog row"
            )
    return sorted(set(failures))


def _load_schema(path: Path, failures: list[str]) -> dict[str, Any] | None:
    try:
        schema = json.loads(path.read_text())
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        failures.append(f"cannot load mapping schema {path}: {error}")
        return None
    if not isinstance(schema, dict):
        failures.append(f"mapping schema {path} root must be an object")
        return None
    return schema


def _mapping_records(
    value: Any,
    *,
    label: str,
    id_field: str,
    failures: list[str],
) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        failures.append(f"mapping {label} document must contain an array")
        return {}
    records: dict[str, Mapping[str, Any]] = {}
    for index, record in enumerate(value):
        if not isinstance(record, Mapping):
            failures.append(f"mapping {label}[{index}] must be an object")
            continue
        record_id = record.get(id_field)
        if not isinstance(record_id, str) or not record_id:
            failures.append(f"mapping {label}[{index}] lacks an ID")
        elif record_id in records:
            failures.append(f"mapping {label} document duplicates {record_id}")
        else:
            records[record_id] = record
    return records


def _validate_snapshot_staleness(
    *,
    root: Path,
    snapshot_id: str,
    snapshot: Mapping[str, Any],
    fact: Mapping[str, Any] | None,
    failures: list[str],
) -> None:
    if fact is None:
        failures.append(f"{snapshot_id} historical snapshot is absent from live facts")
        return
    for field in ("discovery_source_kind", "path", "name", "ignore_state"):
        if snapshot.get(field) != fact.get(field):
            failures.append(
                f"{snapshot_id} historical snapshot {field} is stale"
            )
    path = snapshot.get("path")
    if not isinstance(path, str):
        return
    resolved_root = root.resolve()
    source = resolved_root / path
    try:
        resolved_source = source.resolve()
        resolved_source.relative_to(resolved_root)
    except (OSError, ValueError):
        failures.append(f"{snapshot_id} historical source path escapes repository")
        return
    if source.is_symlink() or not resolved_source.is_file():
        failures.append(f"{snapshot_id} historical source path is not a regular file")
        return
    actual = "sha256:" + hashlib.sha256(resolved_source.read_bytes()).hexdigest()
    if snapshot.get("test_source_digest") != actual:
        failures.append(f"{snapshot_id} historical reviewed source digest is stale")
