"""Live joins and association-state derivation for the Phase 9 fuzz program."""

from __future__ import annotations

from collections import Counter
from collections.abc import Mapping
from typing import Any

from .fuzz_program_discovery import CargoFuzzScan, FuzzLikeScan


SURFACE_STATES = (
    "cargo_fuzz_target",
    "smoke_only",
    "partial_only",
    "unmapped",
)
GAP_REASONS = {
    "smoke_only": "no_cargo_fuzz_target",
    "partial_only": "no_direct_surface_target",
    "unmapped": "no_associated_target",
}


def analyze_fuzz_program(
    program: Mapping[str, Any],
    cargo_scan: CargoFuzzScan,
    smoke_scan: FuzzLikeScan,
) -> tuple[dict[str, Any], list[str]]:
    """Resolve all hand-owned rows to live identities and derive report rows."""

    failures = [
        f"{item.path}:{item.line} {item.kind}: {item.message}"
        for item in [*cargo_scan.diagnostics, *smoke_scan.diagnostics]
    ]
    cargo_by_key = {
        (fact.manifest_path, fact.name): fact for fact in cargo_scan.facts
    }
    smoke_by_id = {fact.stable_id: fact for fact in smoke_scan.facts}
    targets = program.get("targets")
    if not isinstance(targets, list):
        return _empty_analysis(), [*failures, "fuzz program targets must be an array"]

    resolved_rows: list[dict[str, Any]] = []
    registered_cargo: list[tuple[str, str]] = []
    registered_smoke: list[str] = []
    seen_ids: set[str] = set()
    for index, target in enumerate(targets):
        where = f"targets[{index}]"
        if not isinstance(target, Mapping):
            failures.append(f"{where} must be an object")
            continue
        target_id = target.get("id")
        if not isinstance(target_id, str):
            failures.append(f"{where}.id must be a string")
            continue
        if target_id in seen_ids:
            failures.append(f"duplicate fuzz target id {target_id}")
            continue
        seen_ids.add(target_id)
        kind = target.get("target_kind")
        fact: Any | None = None
        if kind == "cargo_fuzz":
            key = (target.get("manifest_path"), target.get("name"))
            if not all(isinstance(item, str) for item in key):
                failures.append(f"{target_id} cargo target identity is incomplete")
                continue
            registered_cargo.append(key)  # type: ignore[arg-type]
            fact = cargo_by_key.get(key)
            if fact is None:
                failures.append(f"{target_id} is absent from live cargo-fuzz targets")
                continue
            expected = {
                "path": fact.path,
                "command": fact.command,
                "corpus_path": fact.corpus_path,
                "artifact_path": fact.artifact_path,
            }
            for field, value in expected.items():
                if target.get(field) != value:
                    failures.append(f"{target_id} {field} does not match live cargo-fuzz target")
        elif kind == "bounded_rust_smoke":
            discovery_id = target.get("discovery_id")
            if not isinstance(discovery_id, str):
                failures.append(f"{target_id} discovery_id is incomplete")
                continue
            registered_smoke.append(discovery_id)
            fact = smoke_by_id.get(discovery_id)
            if fact is None:
                failures.append(f"{target_id} is absent from live fuzz-like Rust facts")
                continue
            expected = {
                "path": fact.path,
                "name": fact.name,
                "discovery_source_kind": fact.source_kind,
                "command": fact.command_hint,
            }
            for field, value in expected.items():
                if target.get(field) != value:
                    failures.append(f"{target_id} {field} does not match live Rust fact")
            if fact.ignore_state != "not_ignored":
                failures.append(
                    f"{target_id} live Rust fact must be not_ignored to retain a runnable tier; "
                    f"found {fact.ignore_state!r}"
                )
        else:
            failures.append(f"{target_id} uses unknown target_kind {kind!r}")
            continue
        resolved = _resolved_target_row(target)
        if kind == "bounded_rust_smoke" and fact is not None:
            resolved["ignore_state"] = fact.ignore_state
        resolved_rows.append(resolved)

    live_cargo = sorted(cargo_by_key)
    if sorted(registered_cargo) != live_cargo:
        failures.append(
            "cargo-fuzz registry does not exactly match live targets: "
            f"registered={sorted(registered_cargo)!r}, live={live_cargo!r}"
        )
    live_smoke = sorted(smoke_by_id)
    if sorted(registered_smoke) != live_smoke:
        failures.append(
            "fuzz-like Rust registry does not exactly match live candidate facts: "
            f"registered={sorted(registered_smoke)!r}, live={live_smoke!r}"
        )

    surface_rows, gap_rows, surface_failures = _surface_rows(program, resolved_rows)
    failures.extend(surface_failures)
    analysis = _analysis(resolved_rows, surface_rows, gap_rows)
    return analysis, sorted(set(failures))


def _resolved_target_row(target: Mapping[str, Any]) -> dict[str, Any]:
    fields = (
        "id",
        "target_kind",
        "name",
        "path",
        "owner",
        "primary_tier",
        "additional_tiers",
        "enforcement_status",
        "execution_basis_ids",
        "surface_associations",
        "last_reviewed",
    )
    row = {field: target.get(field) for field in fields}
    for field in (
        "manifest_path",
        "discovery_id",
        "discovery_source_kind",
        "command",
        "corpus_path",
        "artifact_path",
    ):
        if field in target:
            row[field] = target[field]
    row["corpus_contents_assessed"] = False
    return row


def _surface_rows(
    program: Mapping[str, Any],
    targets: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[str]]:
    failures: list[str] = []
    surfaces = program.get("surfaces")
    if not isinstance(surfaces, list):
        return [], [], ["fuzz program surfaces must be an array"]
    associations: dict[str, list[tuple[dict[str, Any], str]]] = {}
    for target in targets:
        rows = target.get("surface_associations")
        if not isinstance(rows, list):
            failures.append(f"{target.get('id')} surface_associations must be an array")
            continue
        for association in rows:
            if not isinstance(association, Mapping):
                failures.append(f"{target.get('id')} has malformed surface association")
                continue
            surface_id = association.get("surface_id")
            strength = association.get("strength")
            if not isinstance(surface_id, str) or strength not in {"direct", "partial"}:
                failures.append(f"{target.get('id')} has malformed surface association")
                continue
            associations.setdefault(surface_id, []).append((target, strength))

    rows: list[dict[str, Any]] = []
    gaps: list[dict[str, Any]] = []
    known_surface_ids: set[str] = set()
    for surface in surfaces:
        if not isinstance(surface, Mapping) or not isinstance(surface.get("id"), str):
            failures.append("surface record must have a string id")
            continue
        surface_id = surface["id"]
        known_surface_ids.add(surface_id)
        selected = associations.get(surface_id, [])
        direct = [target for target, strength in selected if strength == "direct"]
        partial = [target for target, strength in selected if strength == "partial"]
        if any(target.get("target_kind") == "cargo_fuzz" for target in direct):
            state = "cargo_fuzz_target"
        elif any(target.get("target_kind") == "bounded_rust_smoke" for target in direct):
            state = "smoke_only"
        elif partial:
            state = "partial_only"
        else:
            state = "unmapped"
        row = {
            "surface_id": surface_id,
            "title": surface.get("title"),
            "area": surface.get("area"),
            "state": state,
            "target_ids": sorted(target["id"] for target, _ in selected),
            "direct_target_ids": sorted(target["id"] for target in direct),
            "partial_target_ids": sorted(target["id"] for target in partial),
        }
        rows.append(row)
        if state != "cargo_fuzz_target":
            gaps.append(
                {
                    "surface_id": surface_id,
                    "state": state,
                    "reason": GAP_REASONS[state],
                    "target_ids": row["target_ids"],
                }
            )
    unknown = sorted(set(associations) - known_surface_ids)
    if unknown:
        failures.append("target associations use unknown surfaces: " + ", ".join(unknown))
    return rows, gaps, failures


def _analysis(
    targets: list[dict[str, Any]],
    surfaces: list[dict[str, Any]],
    gaps: list[dict[str, Any]],
) -> dict[str, Any]:
    primary = Counter(row.get("primary_tier") for row in targets)
    additional = Counter(
        tier
        for row in targets
        for tier in row.get("additional_tiers", [])
        if isinstance(tier, str)
    )
    states = Counter(row["state"] for row in surfaces)
    return {
        "targets": targets,
        "surfaces": surfaces,
        "gap_rows": gaps,
        "summary": {
            "inventory_targets": len(targets),
            "cargo_fuzz_targets": sum(row.get("target_kind") == "cargo_fuzz" for row in targets),
            "bounded_rust_smokes": sum(row.get("target_kind") == "bounded_rust_smoke" for row in targets),
            "required_surfaces": len(surfaces),
            "gap_surfaces": len(gaps),
            "by_surface_state": {state: states[state] for state in SURFACE_STATES},
            "by_primary_tier": {
                tier: primary[tier] for tier in ("pr_smoke", "nightly", "manual_extended")
            },
            "by_additional_tier": {
                tier: additional[tier]
                for tier in ("pr_smoke", "nightly", "manual_extended")
            },
        },
    }


def _empty_analysis() -> dict[str, Any]:
    return _analysis([], [], [])
