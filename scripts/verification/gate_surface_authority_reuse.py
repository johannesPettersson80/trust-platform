"""Exact code-identity routes from cataloged gate surfaces to authority reuse."""

from __future__ import annotations

from collections.abc import Callable, Iterable, Mapping, Sequence
from typing import Any


INVARIANT_ID = "VERIFICATION_GATE_SURFACE_EXECUTION_001"
SPEC_REF = (
    "SPEC_VERIFICATION_INVENTORY_001"
    "#gate-and-workflow-surface-proof-contract"
)


def build_gate_surface_authority_routes(
    *,
    candidate_invariant: Mapping[str, Any],
    tests: Mapping[str, Mapping[str, Any]],
    inventory_records: Mapping[str, Mapping[str, Any]],
    live_gate_facts: Sequence[Any],
    mappings: Mapping[str, Mapping[str, Any]],
) -> dict[str, dict[str, Any]]:
    """Join reviewed gate identities to exact file-entrypoint code facts."""

    if (
        candidate_invariant.get("invariant_id") != INVARIANT_ID
        or candidate_invariant.get("area") != "verification"
        or candidate_invariant.get("spec_ref") != SPEC_REF
    ):
        raise ValueError("gate-surface candidate authority differs")
    test_ids = _string_array(
        candidate_invariant.get("test_ids"), "gate-surface test IDs"
    )
    live_by_discovery = _unique_by(
        live_gate_facts,
        lambda fact: _fact_field(fact, "stable_id"),
        "live gate discovery",
    )
    inventory_by_discovery = _unique_by(
        inventory_records.values(),
        lambda record: record.get("discovery_id"),
        "gate inventory discovery",
        skip_missing=True,
    )
    routes: dict[str, dict[str, Any]] = {}
    used_discoveries: set[str] = set()
    for test_id in test_ids:
        test = tests.get(test_id)
        if not isinstance(test, Mapping):
            raise ValueError(f"{test_id}: gate-surface catalog row is absent")
        source_kind = test.get("discovery_source_kind")
        if source_kind == "github_workflow_job":
            continue
        if source_kind != "gate_script":
            raise ValueError(f"{test_id}: gate-surface source kind differs")
        if (
            test.get("status") != "mapped"
            or test.get("area") != "verification"
            or INVARIANT_ID not in test.get("invariants", [])
        ):
            raise ValueError(f"{test_id}: gate-surface catalog authority differs")
        discovery_id = _required_string(test, "discovery_id", test_id)
        if discovery_id in used_discoveries:
            raise ValueError(
                f"{discovery_id}: gate discovery is duplicated in catalog"
            )
        used_discoveries.add(discovery_id)
        live = live_by_discovery.get(discovery_id)
        inventory = inventory_by_discovery.get(discovery_id)
        if live is None:
            raise ValueError(f"{test_id}: live gate discovery is absent")
        if inventory is None:
            raise ValueError(f"{test_id}: gate inventory discovery is absent")
        if inventory.get("source_kind") != "gate_script":
            raise ValueError(f"{test_id}: inventory source kind differs")
        if _fact_field(live, "source_kind") != "gate_script":
            raise ValueError(f"{test_id}: live source kind differs")
        for field in ("path", "name"):
            live_value = _fact_field(live, field)
            if test.get(field) != live_value:
                raise ValueError(
                    f"{test_id}: catalog {field} differs from live gate"
                )
            if inventory.get(field) != live_value:
                raise ValueError(
                    f"{test_id}: inventory {field} differs from live gate"
                )
        if inventory.get("command") != _fact_field(live, "command_hint"):
            raise ValueError(f"{test_id}: inventory command differs from live gate")
        path = _fact_field(live, "path")
        expected_item_kind = _entrypoint_kind(path)
        code_rows = [
            row
            for row in mappings.values()
            if row.get("path") == path
            and row.get("area") == "verification"
            and row.get("item_kind") == expected_item_kind
        ]
        if len(code_rows) != 1:
            raise ValueError(
                f"{test_id}: expected one exact gate entrypoint, got {len(code_rows)}"
            )
        mapping = code_rows[0]
        if mapping.get("disposition") != "spec_gap":
            continue
        code_id = _required_string(mapping, "code_id", test_id)
        if code_id in routes:
            raise ValueError(f"{code_id}: gate entrypoint route is duplicated")
        routes[code_id] = {
            "invariant_id": INVARIANT_ID,
            "test_id": test_id,
            "discovery_id": discovery_id,
            "inventory_id": _required_string(inventory, "id", test_id),
            "source_kind": "gate_script",
            "path": path,
            "name": _fact_field(live, "name"),
            "catalog_command": _required_string(test, "command", test_id),
            "native_command": _required_string(inventory, "command", test_id),
            "command_role": _required_string(inventory, "command_role", test_id),
            "inventory_owner": _required_string(inventory, "owner", test_id),
            "environment": _required_string(inventory, "environment", test_id),
            "enforcement": _required_string(inventory, "enforcement", test_id),
            "artifact_kind": _required_string(
                inventory, "artifact_kind", test_id
            ),
            "artifact_paths": _string_array(
                inventory.get("artifact_paths"),
                f"{test_id} artifact paths",
                allow_empty=True,
            ),
            "artifact_retention": _required_string(
                inventory, "artifact_retention", test_id
            ),
            "proof_boundary": "registered_surface_only",
        }
    return dict(sorted(routes.items()))


def _entrypoint_kind(path: str) -> str:
    if path.endswith(".py"):
        return "python_entrypoint"
    if path.endswith(".sh"):
        return "shell_entrypoint"
    raise ValueError(f"{path}: gate path has no supported entrypoint kind")


def _unique_by(
    rows: Iterable[Any],
    key_fn: Callable[[Any], Any],
    label: str,
    *,
    skip_missing: bool = False,
) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for row in rows:
        key = key_fn(row)
        if key is None and skip_missing:
            continue
        if not isinstance(key, str) or not key:
            raise ValueError(f"{label} identity is absent")
        if key in result:
            raise ValueError(f"{label} {key} is duplicated")
        result[key] = row
    return result


def _fact_field(fact: Any, field: str) -> str:
    value = (
        fact.get(field)
        if isinstance(fact, Mapping)
        else getattr(fact, field, None)
    )
    if not isinstance(value, str) or not value:
        raise ValueError(f"live gate {field} is absent")
    return value


def _required_string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label}: {field} is absent")
    return value


def _string_array(
    value: Any, label: str, *, allow_empty: bool = False
) -> list[str]:
    if (
        not isinstance(value, list)
        or (not value and not allow_empty)
        or any(not isinstance(item, str) or not item for item in value)
        or value != sorted(set(value))
    ):
        raise ValueError(f"{label} must be a sorted unique string array")
    return list(value)
