"""Resolve frozen static-test assertions to reviewed invariant behaviors."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any


BehaviorRef = tuple[str, int]
BehaviorAssignments = dict[str, tuple[BehaviorRef, ...]]


def resolve_behavior_assignments(
    *,
    inventories: Mapping[str, Any],
    plan: Mapping[str, Any],
    default_invariant_id: str | None,
    allowed_invariant_ids: set[str],
) -> BehaviorAssignments:
    """Resolve exactly one supported semantic-assignment plan form."""

    if not isinstance(plan, Mapping):
        raise ValueError("assignment plan must be an object")
    if "test_assignments" in plan:
        if "ranges" in plan:
            raise ValueError(
                "plan cannot combine test_assignments with ranges"
            )
        return _assign_by_test_name(
            inventories=inventories,
            rows=plan["test_assignments"],
            allowed_invariant_ids=allowed_invariant_ids,
        )
    return _assign_by_ranges(
        inventories=inventories,
        ranges=plan.get("ranges"),
        default_invariant_id=default_invariant_id,
        allowed_invariant_ids=allowed_invariant_ids,
    )


def assign_behaviors(
    *,
    inventories: Mapping[str, Any],
    ranges: Any,
) -> dict[str, tuple[int, ...]]:
    """Compatibility wrapper for single-invariant line-range plans."""

    assignments = assign_behavior_refs(
        inventories=inventories,
        ranges=ranges,
        default_invariant_id="_single",
        allowed_invariant_ids={"_single"},
    )
    return {
        discovery_id: tuple(index for _invariant_id, index in values)
        for discovery_id, values in assignments.items()
    }


def assign_behavior_refs(
    *,
    inventories: Mapping[str, Any],
    ranges: Any,
    default_invariant_id: str | None,
    allowed_invariant_ids: set[str],
) -> BehaviorAssignments:
    """Compatibility wrapper for reviewed line-range behavior bindings."""

    return resolve_behavior_assignments(
        inventories=inventories,
        plan={"ranges": ranges},
        default_invariant_id=default_invariant_id,
        allowed_invariant_ids=allowed_invariant_ids,
    )


def _assign_by_test_name(
    *,
    inventories: Mapping[str, Any],
    rows: Any,
    allowed_invariant_ids: set[str],
) -> BehaviorAssignments:
    if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
        raise ValueError("plan test_assignments must be an array")

    by_name: dict[str, BehaviorRef] = {}
    for index, row in enumerate(rows):
        if not isinstance(row, Mapping) or set(row) != {
            "name",
            "invariant_id",
            "behavior_index",
        }:
            raise ValueError(f"plan test_assignment {index} fields drift")
        name = row["name"]
        invariant_id = row["invariant_id"]
        behavior_index = row["behavior_index"]
        if not isinstance(name, str) or not name:
            raise ValueError(
                f"plan test_assignment {index} name must be non-empty"
            )
        if name in by_name:
            raise ValueError(
                f"plan test_assignments duplicate name {name}"
            )
        if not isinstance(invariant_id, str) or not invariant_id:
            raise ValueError(
                f"plan test_assignment {index} invariant_id "
                "must be non-empty"
            )
        if invariant_id not in allowed_invariant_ids:
            raise ValueError(
                f"plan test_assignment {index} selects unapproved invariant "
                f"{invariant_id}"
            )
        if (
            not isinstance(behavior_index, int)
            or isinstance(behavior_index, bool)
            or behavior_index < 0
        ):
            raise ValueError(
                f"plan test_assignment {index} behavior_index is invalid"
            )
        by_name[name] = (invariant_id, behavior_index)

    inventory_by_name: dict[str, tuple[str, int]] = {}
    for discovery_id, inventory in inventories.items():
        assertion_count = _reviewed_assertion_count(
            discovery_id=str(discovery_id),
            inventory=inventory,
        )
        test_name = inventory.get("test_name")
        if not isinstance(test_name, str) or not test_name:
            raise ValueError(
                f"{discovery_id} assertion inventory lacks test_name"
            )
        if test_name in inventory_by_name:
            raise ValueError(
                f"assertion inventories duplicate test name {test_name}"
            )
        inventory_by_name[test_name] = (
            str(discovery_id),
            assertion_count,
        )

    expected_names = set(inventory_by_name)
    assigned_names = set(by_name)
    missing = sorted(expected_names - assigned_names)
    extra = sorted(assigned_names - expected_names)
    if missing or extra:
        details = []
        if missing:
            details.append("missing names: " + ", ".join(missing))
        if extra:
            details.append("extra names: " + ", ".join(extra))
        raise ValueError(
            "plan test_assignments do not equal selected inventory names; "
            + "; ".join(details)
        )

    return {
        discovery_id: tuple(by_name[name] for _ in range(assertion_count))
        for name, (discovery_id, assertion_count) in inventory_by_name.items()
    }


def _assign_by_ranges(
    *,
    inventories: Mapping[str, Any],
    ranges: Any,
    default_invariant_id: str | None,
    allowed_invariant_ids: set[str],
) -> BehaviorAssignments:
    if not isinstance(ranges, Sequence) or isinstance(ranges, (str, bytes)):
        raise ValueError("plan ranges must be an array")
    normalized: list[tuple[str, int, int, int]] = []
    for index, row in enumerate(ranges):
        if not isinstance(row, Mapping):
            raise ValueError(f"plan range {index} fields drift")
        fields = set(row)
        if fields == {"behavior_index", "min_line", "max_line"}:
            if default_invariant_id is None:
                raise ValueError(f"plan range {index} requires invariant_id")
            invariant_id = default_invariant_id
        elif fields == {
            "invariant_id",
            "behavior_index",
            "min_line",
            "max_line",
        }:
            invariant_id = row["invariant_id"]
            if not isinstance(invariant_id, str) or not invariant_id:
                raise ValueError(
                    f"plan range {index} invariant_id must be non-empty"
                )
        else:
            raise ValueError(f"plan range {index} fields drift")
        if invariant_id not in allowed_invariant_ids:
            raise ValueError(
                f"plan range {index} selects unapproved invariant "
                f"{invariant_id}"
            )
        values = (
            row["behavior_index"],
            row["min_line"],
            row["max_line"],
        )
        if any(
            not isinstance(value, int) or isinstance(value, bool)
            for value in values
        ):
            raise ValueError(f"plan range {index} values must be integers")
        behavior_index, minimum, maximum = values
        if behavior_index < 0 or minimum < 1 or maximum < minimum:
            raise ValueError(f"plan range {index} is invalid")
        normalized.append((invariant_id, behavior_index, minimum, maximum))

    assignments: BehaviorAssignments = {}
    for discovery_id, inventory in inventories.items():
        lines = _reviewed_assertion_lines(
            discovery_id=str(discovery_id),
            inventory=inventory,
        )
        matched_by_line = [
            {
                (invariant_id, behavior_index)
                for invariant_id, behavior_index, minimum, maximum in normalized
                if minimum <= line <= maximum
            }
            for line in lines
        ]
        if any(len(matches) != 1 for matches in matched_by_line):
            bad_line, matches = next(
                (line, matches)
                for line, matches in zip(lines, matched_by_line, strict=True)
                if len(matches) != 1
            )
            raise ValueError(
                f"{discovery_id} line {bad_line} has {len(matches)} "
                "behavior assignments"
            )
        assignments[str(discovery_id)] = tuple(
            next(iter(matches)) for matches in matched_by_line
        )
    return assignments


def _reviewed_assertion_count(
    *,
    discovery_id: str,
    inventory: Any,
) -> int:
    if not isinstance(inventory, Mapping):
        raise ValueError(f"{discovery_id} assertion inventory is not an object")
    assertions = inventory.get("assertions")
    if (
        not isinstance(assertions, Sequence)
        or isinstance(assertions, (str, bytes))
        or not assertions
        or any(not isinstance(row, Mapping) for row in assertions)
    ):
        raise ValueError(f"{discovery_id} lacks reviewed assertions")
    return len(assertions)


def _reviewed_assertion_lines(
    *,
    discovery_id: str,
    inventory: Any,
) -> tuple[int, ...]:
    _reviewed_assertion_count(
        discovery_id=discovery_id,
        inventory=inventory,
    )
    lines = tuple(assertion.get("line") for assertion in inventory["assertions"])
    if any(
        not isinstance(line, int) or isinstance(line, bool) for line in lines
    ):
        raise ValueError(f"{discovery_id} assertion lines are invalid")
    return lines
