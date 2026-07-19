"""Closed code-area metadata contract and default-deny path routing."""

from __future__ import annotations

import fnmatch
import re
from dataclasses import dataclass
from functools import lru_cache
from pathlib import PurePosixPath
from typing import Any, Iterable, Mapping

from .metadata_validator.constants import CASE_FAMILIES, INTENTS, TEST_CLASSES


MILESTONE_SUITE_IDS = {"veryquick", "pr", "nightly", "release", "hardware_lab"}
SUITE_ORDER = {name: index for index, name in enumerate(("veryquick", "pr", "nightly", "release", "hardware_lab"))}
MATRIX_ROOT_FIELDS = {
    "schema_version",
    "id",
    "title",
    "status",
    "owner",
    "last_reviewed",
    "areas",
    "code_areas",
    "intent_requirements",
}
ROUTE_FIELDS = {
    "id",
    "match_kind",
    "area_ids",
    "path_globs",
    "intents",
    "required_test_classes",
    "suite_tiers",
    "conditional_suite_tiers",
    "notes",
}
AREA_FIELDS = {
    "id",
    "status",
    "owner",
    "risk_default",
    "high_risks",
    "path_globs",
    "required_test_classes",
    "required_case_families",
    "suite_tiers",
}
AREA_OPTIONAL_FIELDS = {"decision_ref"}
INTENT_FIELDS = {
    "intent",
    "required_test_classes",
    "red_required",
    "lock_required",
}


class AreaRoutingError(ValueError):
    """Raised when a changed path cannot be safely normalized."""


@dataclass(frozen=True)
class RouteSelection:
    route_ids: tuple[str, ...]
    area_ids: tuple[str, ...]
    required_test_classes: tuple[str, ...]
    suite_tiers: tuple[str, ...]
    conditional_suite_tiers: tuple[str, ...]


@dataclass(frozen=True)
class PathRoute(RouteSelection):
    path: str

    @property
    def unmapped(self) -> bool:
        return not self.route_ids and not self.area_ids


def taxonomy_route_ids(text: str) -> list[str]:
    """Extract the reviewed stable IDs from the code-area table."""

    try:
        section = text.split("## Code Area to Minimum Test Matrix", 1)[1].split(
            "Crates with no", 1
        )[0]
    except IndexError:
        return []
    route_ids: list[str] = []
    for line in section.splitlines():
        match = re.match(r"^\| `([a-z][a-z0-9_]*)` [^|]+\|", line)
        if match:
            route_ids.append(match.group(1))
    return route_ids


def validate_area_routing(
    matrix: Mapping[str, Any],
    taxonomy_text: str,
    *,
    canonical_areas: set[str],
    suite_ids: set[str] = MILESTONE_SUITE_IDS,
) -> list[str]:
    """Return deterministic failures for the full routing contract."""

    failures: list[str] = []
    _check_exact_fields(matrix, MATRIX_ROOT_FIELDS, "planning matrix", failures)
    if matrix.get("schema_version") != 2:
        failures.append("planning matrix must use schema_version = 2")

    areas = matrix.get("areas")
    if not isinstance(areas, list):
        failures.append("planning matrix areas must be an array")
        areas = []
    raw_area_ids = [row.get("id") for row in areas if isinstance(row, dict)]
    area_ids = [value for value in raw_area_ids if isinstance(value, str)]
    if len(area_ids) != len(raw_area_ids):
        failures.append("planning matrix area IDs must be strings")
    if len(area_ids) != len(set(area_ids)):
        failures.append("planning matrix area IDs must be unique")
    if set(area_ids) != canonical_areas:
        failures.append(
            "planning matrix areas must exactly cover canonical AREAS: "
            f"missing={sorted(canonical_areas - set(area_ids))}, "
            f"extra={sorted(set(area_ids) - canonical_areas)}"
        )
    for index, area in enumerate(areas):
        label = f"areas[{index}]"
        if not isinstance(area, dict):
            failures.append(f"{label} must be an object")
            continue
        _check_fields_with_optional(
            area,
            AREA_FIELDS,
            AREA_OPTIONAL_FIELDS,
            label,
            failures,
        )
        _check_string_list(area.get("path_globs"), f"{label}.path_globs", failures, nonempty=True)
        _check_vocab(area.get("required_test_classes"), TEST_CLASSES, f"{label}.required_test_classes", failures)
        _check_vocab(area.get("required_case_families"), CASE_FAMILIES, f"{label}.required_case_families", failures)
        _check_vocab(area.get("suite_tiers"), suite_ids, f"{label}.suite_tiers", failures, nonempty=True)
        for pattern in area.get("path_globs", []) if isinstance(area.get("path_globs"), list) else []:
            _check_glob(pattern, f"{label}.path_globs", failures)

    routes = matrix.get("code_areas")
    if not isinstance(routes, list):
        failures.append("planning matrix code_areas must be an array")
        routes = []
    taxonomy_ids = taxonomy_route_ids(taxonomy_text)
    raw_route_ids = [row.get("id") for row in routes if isinstance(row, dict)]
    route_ids = [value for value in raw_route_ids if isinstance(value, str)]
    if len(route_ids) != len(raw_route_ids):
        failures.append("planning matrix taxonomy route IDs must be strings")
    if len(route_ids) != len(set(route_ids)):
        failures.append("planning matrix taxonomy route IDs must be unique")
    if set(route_ids) != set(taxonomy_ids):
        failures.append(
            "planning matrix taxonomy route IDs drift: "
            f"missing={sorted(set(taxonomy_ids) - set(route_ids))}, "
            f"extra={sorted(set(route_ids) - set(taxonomy_ids))}"
        )
    if route_ids != taxonomy_ids:
        failures.append("planning matrix code_areas must preserve reviewed taxonomy order")

    path_owned_areas: set[str] = set()
    for index, route in enumerate(routes):
        label = f"code_areas[{index}]"
        if not isinstance(route, dict):
            failures.append(f"{label} must be an object")
            continue
        _check_exact_fields(route, ROUTE_FIELDS, label, failures)
        match_kind = route.get("match_kind")
        if match_kind not in {"path", "intent"}:
            failures.append(f"{label}.match_kind must be path or intent")
        route_areas = route.get("area_ids")
        _check_vocab(route_areas, canonical_areas, f"{label}.area_ids", failures)
        paths = route.get("path_globs")
        intents = route.get("intents")
        _check_string_list(paths, f"{label}.path_globs", failures)
        _check_vocab(intents, INTENTS, f"{label}.intents", failures)
        if match_kind == "path":
            if not paths:
                failures.append(f"{label} path route must define path_globs")
            if intents:
                failures.append(f"{label} path route must not define intents")
            if not route_areas:
                failures.append(f"{label} path route must define area_ids")
            if isinstance(route_areas, list):
                path_owned_areas.update(value for value in route_areas if isinstance(value, str))
        elif match_kind == "intent":
            if paths:
                failures.append(f"{label} intent route must not define path_globs")
            if not intents:
                failures.append(f"{label} intent route must define intents")
            if route_areas:
                failures.append(f"{label} intent overlay must not invent area_ids")
        for pattern in paths if isinstance(paths, list) else []:
            _check_glob(pattern, f"{label}.path_globs", failures)
        _check_vocab(
            route.get("required_test_classes"),
            TEST_CLASSES,
            f"{label}.required_test_classes",
            failures,
            nonempty=True,
        )
        _check_vocab(route.get("suite_tiers"), suite_ids, f"{label}.suite_tiers", failures, nonempty=True)
        _check_vocab(
            route.get("conditional_suite_tiers"),
            suite_ids,
            f"{label}.conditional_suite_tiers",
            failures,
        )
        direct = {
            value for value in route.get("suite_tiers", []) if isinstance(value, str)
        } if isinstance(route.get("suite_tiers"), list) else set()
        conditional = {
            value
            for value in route.get("conditional_suite_tiers", [])
            if isinstance(value, str)
        } if isinstance(route.get("conditional_suite_tiers"), list) else set()
        if direct & conditional:
            failures.append(f"{label} suite tiers cannot be both direct and conditional")
        if not isinstance(route.get("notes"), str) or not route["notes"].strip():
            failures.append(f"{label}.notes must be non-empty")

    if path_owned_areas != canonical_areas:
        failures.append(
            "path routes must cover every canonical area: "
            f"missing={sorted(canonical_areas - path_owned_areas)}"
        )
    intents = matrix.get("intent_requirements")
    if not isinstance(intents, list):
        failures.append("planning matrix intent_requirements must be an array")
        intents = []
    seen_intents: set[str] = set()
    for index, intent in enumerate(intents):
        label = f"intent_requirements[{index}]"
        if not isinstance(intent, dict):
            failures.append(f"{label} must be an object")
            continue
        _check_exact_fields(intent, INTENT_FIELDS, label, failures)
        name = intent.get("intent")
        if not isinstance(name, str) or name not in INTENTS:
            failures.append(f"{label}.intent is unsupported")
        elif name in seen_intents:
            failures.append(f"{label}.intent duplicates {name}")
        else:
            seen_intents.add(name)
        _check_vocab(
            intent.get("required_test_classes"),
            TEST_CLASSES,
            f"{label}.required_test_classes",
            failures,
        )
        for field in ("red_required", "lock_required"):
            if not isinstance(intent.get(field), bool):
                failures.append(f"{label}.{field} must be boolean")
    if seen_intents != INTENTS:
        failures.append(
            "planning matrix intent rows must exactly cover INTENTS: "
            f"missing={sorted(INTENTS - seen_intents)}"
        )
    return sorted(set(failures))


def normalize_changed_path(value: str) -> str:
    """Return a canonical workspace-relative POSIX path or fail closed."""

    if not isinstance(value, str) or not value or value != value.strip():
        raise AreaRoutingError("changed path must be non-empty without surrounding whitespace")
    if "\\" in value or any(ord(character) < 32 for character in value):
        raise AreaRoutingError("changed path must use printable POSIX separators")
    path = PurePosixPath(value)
    if path.is_absolute() or "." in path.parts or ".." in path.parts:
        raise AreaRoutingError("changed path must be normalized and workspace-relative")
    normalized = path.as_posix()
    if normalized != value:
        raise AreaRoutingError("changed path must already be in canonical POSIX form")
    return normalized


def classify_changed_path(matrix: Mapping[str, Any], value: str) -> PathRoute:
    """Classify one path; an empty route selection is the default-deny result."""

    path = normalize_changed_path(value)
    matches = [
        route
        for route in matrix.get("code_areas", [])
        if isinstance(route, dict)
        and route.get("match_kind") == "path"
        and any(_path_glob_matches(path, pattern) for pattern in route.get("path_globs", []))
    ]
    selection = _selection(matches) if matches else _fallback_selection(matrix, path)
    return PathRoute(path=path, **selection.__dict__)


def intent_overlay(matrix: Mapping[str, Any], intent: str) -> RouteSelection:
    """Return requirements attached directly to an intent without assigning areas."""

    matches = [
        route
        for route in matrix.get("code_areas", [])
        if isinstance(route, dict)
        and route.get("match_kind") == "intent"
        and intent in route.get("intents", [])
    ]
    return _selection(matches)


def _selection(routes: Iterable[Mapping[str, Any]]) -> RouteSelection:
    rows = list(routes)
    return RouteSelection(
        route_ids=tuple(str(row["id"]) for row in rows),
        area_ids=tuple(sorted({str(value) for row in rows for value in row.get("area_ids", [])})),
        required_test_classes=tuple(
            sorted({str(value) for row in rows for value in row.get("required_test_classes", [])})
        ),
        suite_tiers=tuple(
            sorted(
                {str(value) for row in rows for value in row.get("suite_tiers", [])},
                key=lambda value: (SUITE_ORDER.get(value, len(SUITE_ORDER)), value),
            )
        ),
        conditional_suite_tiers=tuple(
            sorted(
                {
                    str(value)
                    for row in rows
                    for value in row.get("conditional_suite_tiers", [])
                },
                key=lambda value: (SUITE_ORDER.get(value, len(SUITE_ORDER)), value),
            )
        ),
    )


def _fallback_selection(matrix: Mapping[str, Any], path: str) -> RouteSelection:
    """Use canonical area globs only when no more specific taxonomy route matches."""

    areas = [
        area
        for area in matrix.get("areas", [])
        if isinstance(area, dict)
        and any(_path_glob_matches(path, pattern) for pattern in area.get("path_globs", []))
    ]
    return RouteSelection(
        route_ids=(),
        area_ids=tuple(sorted(str(area["id"]) for area in areas)),
        required_test_classes=tuple(
            sorted(
                {
                    str(value)
                    for area in areas
                    for value in area.get("required_test_classes", [])
                }
            )
        ),
        suite_tiers=tuple(
            sorted(
                {str(value) for area in areas for value in area.get("suite_tiers", [])},
                key=lambda value: (SUITE_ORDER.get(value, len(SUITE_ORDER)), value),
            )
        ),
        conditional_suite_tiers=(),
    )


def _path_glob_matches(path: str, pattern: str) -> bool:
    """Match a full POSIX path while reserving recursion for a `**` segment."""

    path_parts = tuple(path.split("/"))
    pattern_parts = tuple(pattern.split("/"))

    @lru_cache(maxsize=None)
    def matches(path_index: int, pattern_index: int) -> bool:
        if pattern_index == len(pattern_parts):
            return path_index == len(path_parts)
        token = pattern_parts[pattern_index]
        if token == "**":
            return matches(path_index, pattern_index + 1) or (
                path_index < len(path_parts)
                and matches(path_index + 1, pattern_index)
            )
        return (
            path_index < len(path_parts)
            and fnmatch.fnmatchcase(path_parts[path_index], token)
            and matches(path_index + 1, pattern_index + 1)
        )

    return matches(0, 0)


def _check_exact_fields(
    row: Mapping[str, Any], expected: set[str], label: str, failures: list[str]
) -> None:
    actual = set(row)
    if actual != expected:
        failures.append(
            f"{label} fields drift: missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
        )


def _check_fields_with_optional(
    row: Mapping[str, Any],
    required: set[str],
    optional: set[str],
    label: str,
    failures: list[str],
) -> None:
    actual = set(row)
    missing = required - actual
    extra = actual - required - optional
    if missing or extra:
        failures.append(
            f"{label} fields drift: missing={sorted(missing)}, extra={sorted(extra)}"
        )


def _check_string_list(
    value: Any, label: str, failures: list[str], *, nonempty: bool = False
) -> None:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        failures.append(f"{label} must be an array of non-empty strings")
        return
    if nonempty and not value:
        failures.append(f"{label} must not be empty")
    if len(value) != len(set(value)):
        failures.append(f"{label} must not contain duplicates")


def _check_vocab(
    value: Any,
    vocabulary: set[str],
    label: str,
    failures: list[str],
    *,
    nonempty: bool = False,
) -> None:
    _check_string_list(value, label, failures, nonempty=nonempty)
    if isinstance(value, list):
        unknown = {item for item in value if isinstance(item, str)} - vocabulary
        if unknown:
            failures.append(f"{label} uses unknown values {sorted(unknown)}")


def _check_glob(value: Any, label: str, failures: list[str]) -> None:
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or "\\" in value
        or any(ord(character) < 32 for character in value)
    ):
        failures.append(f"{label} contains an invalid glob {value!r}")
        return
    path = PurePosixPath(value)
    if path.is_absolute() or "." in path.parts or ".." in path.parts or path.as_posix() != value:
        failures.append(f"{label} contains a noncanonical glob {value!r}")
