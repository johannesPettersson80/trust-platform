"""Plan closed focused-execution batches for exact authority routes."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from .frozen_rust_nextest_target import get_target_profile


_PROFILE_BY_PACKAGE = {
    "trust-debug": "debug-remaining",
    "trust-dev": "dev-remaining",
    "trust-hir": "hir-remaining",
    "trust-lsp": "lsp-remaining",
    "trust-plcopen": "plcopen-remaining",
    "trust-runtime": "runtime-remaining",
    "trust-runtime-core": "runtime-core-remaining",
    "trust-syntax": "syntax-remaining",
}


def build_authority_route_execution_plan(
    *,
    routes: Sequence[Mapping[str, Any]],
    facts: Sequence[Mapping[str, Any]],
    denominator_reviews: Sequence[Mapping[str, Any]],
    catalog_tests: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Return deterministic path batches and an exact selection inventory."""

    facts_by_id = _index(facts, "stable_id", "scanner fact")
    reviews_by_id = _index(
        denominator_reviews,
        "discovery_id",
        "denominator review",
    )
    catalog_by_id = _index_scanner_catalog(catalog_tests)
    routes_by_id = _index(routes, "discovery_id", "authority route")

    routes_by_path: dict[str, list[str]] = {}
    source_kinds_by_path: dict[str, str] = {}
    for discovery_id in sorted(routes_by_id):
        route = routes_by_id[discovery_id]
        path = _string(route, "path", f"route {discovery_id}")
        name = _string(route, "name", f"route {discovery_id}")
        fact = _lookup(facts_by_id, discovery_id, "scanner fact")
        review = _lookup(reviews_by_id, discovery_id, "denominator review")
        source_kind = _string(
            fact,
            "source_kind",
            f"scanner fact {discovery_id}",
        )
        _require_identity(fact, discovery_id, path, name, source_kind, "fact")
        _require_identity(
            review,
            discovery_id,
            path,
            name,
            source_kind,
            "review",
        )
        if fact.get("ignore_state") != "not_ignored":
            raise ValueError(f"{discovery_id} is not an active scanner fact")
        if review.get("disposition") != "reviewed_nonmapping":
            raise ValueError(
                f"{discovery_id} is not a reviewed_nonmapping denominator row"
            )
        existing_kind = source_kinds_by_path.setdefault(path, source_kind)
        if existing_kind != source_kind:
            raise ValueError(f"{path} mixes scanner source kinds")
        routes_by_path.setdefault(path, []).append(discovery_id)

    mapped_by_path: dict[str, list[str]] = {}
    for discovery_id, review in reviews_by_id.items():
        if review.get("disposition") != "catalog_mapped":
            continue
        path = _string(review, "path", f"denominator review {discovery_id}")
        if path not in routes_by_path:
            continue
        catalog = _lookup(catalog_by_id, discovery_id, "catalog test")
        name = _string(review, "name", f"denominator review {discovery_id}")
        source_kind = _string(
            review,
            "discovery_source_kind",
            f"denominator review {discovery_id}",
        )
        _require_identity(
            catalog,
            discovery_id,
            path,
            name,
            source_kind,
            "catalog",
        )
        if catalog.get("status") != "mapped":
            raise ValueError(f"{discovery_id} catalog companion is not mapped")
        mapped_by_path.setdefault(path, []).append(discovery_id)

    batches = []
    for path in sorted(routes_by_path):
        profile_key = _profile_key(path)
        profile = get_target_profile(profile_key, path=path)
        if profile.discovery_source_kind != source_kinds_by_path[path]:
            raise ValueError(f"{path} closed profile has the wrong source kind")
        selected = sorted(routes_by_path[path])
        batches.append(
            {
                "expected_catalog_mapped": len(mapped_by_path.get(path, [])),
                "expected_reviewed_nonmapping": len(selected),
                "path": path,
                "profile": profile_key,
                "selected_discovery_ids": selected,
            }
        )

    return {
        "schema_version": 2,
        "batches": batches,
        "inventories": {
            discovery_id: {"path": routes_by_id[discovery_id]["path"]}
            for discovery_id in sorted(routes_by_id)
        },
        "summary": {
            "batches": len(batches),
            "catalog_companions": sum(len(rows) for rows in mapped_by_path.values()),
            "routes": len(routes_by_id),
        },
    }


def _profile_key(path: str) -> str:
    parts = path.split("/")
    package = parts[1] if len(parts) > 2 and parts[0] == "crates" else None
    try:
        return _PROFILE_BY_PACKAGE[package]  # type: ignore[index]
    except KeyError as error:
        raise ValueError(f"{path} has no closed execution profile") from error


def _index(
    rows: Sequence[Mapping[str, Any]],
    field: str,
    label: str,
) -> dict[str, Mapping[str, Any]]:
    result: dict[str, Mapping[str, Any]] = {}
    for index, row in enumerate(rows):
        identifier = _string(row, field, f"{label} row {index}")
        if identifier in result:
            raise ValueError(f"duplicate {label} identity {identifier}")
        result[identifier] = row
    return result


def _index_scanner_catalog(
    rows: Sequence[Mapping[str, Any]],
) -> dict[str, Mapping[str, Any]]:
    scanner_rows = []
    for index, row in enumerate(rows):
        if "discovery_id" not in row:
            continue
        if not isinstance(row.get("discovery_id"), str) or not row["discovery_id"]:
            raise ValueError(
                f"catalog test row {index} has invalid discovery_id"
            )
        scanner_rows.append(row)
    return _index(scanner_rows, "discovery_id", "catalog test")


def _lookup(
    rows: Mapping[str, Mapping[str, Any]],
    discovery_id: str,
    label: str,
) -> Mapping[str, Any]:
    try:
        return rows[discovery_id]
    except KeyError as error:
        raise ValueError(f"{discovery_id} lacks one {label}") from error


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires nonempty {field}")
    return value


def _require_identity(
    row: Mapping[str, Any],
    discovery_id: str,
    path: str,
    name: str,
    source_kind: str,
    label: str,
) -> None:
    actual = (
        row.get("stable_id", row.get("discovery_id")),
        row.get("path"),
        row.get("name"),
        row.get("source_kind", row.get("discovery_source_kind")),
    )
    expected = (discovery_id, path, name, source_kind)
    if actual != expected:
        raise ValueError(f"{discovery_id} {label} identity does not match")
