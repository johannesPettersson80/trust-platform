"""Complete Phase 17 live metadata join."""

from __future__ import annotations

import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Mapping

from .code_spec_common import CodeFact
from .code_spec_contract import validate_map_document
from .code_spec_debt import validate_debt_partition
from .code_spec_rust import RustInventory, invoke_rust_extractor
from .code_spec_risk import index_risk_acknowledgements, validate_risk_ack_document
from .code_spec_tooling import ToolingInventory, invoke_tooling_extractor
from .code_spec_tooling_join import join_tooling_inventories
from .code_spec_typescript import TypeScriptInventory, invoke_typescript_extractor
from .gate_inventory import load_gate_inventory


class CodeSpecLiveError(ValueError):
    """Raised when live metadata cannot form an exact join."""


def merge_fact_sets(*fact_sets: Mapping[str, CodeFact]) -> dict[str, CodeFact]:
    """Merge scanner fact sets without silently rebinding identities."""

    merged: dict[str, CodeFact] = {}
    for fact_set in fact_sets:
        for code_id, fact in fact_set.items():
            if code_id in merged:
                raise CodeSpecLiveError(f"duplicate live code identity {code_id}")
            merged[code_id] = fact
    return dict(sorted(merged.items()))


def validate_global_area_bindings(
    area_bindings: Mapping[str, str],
    live_facts: Mapping[str, CodeFact],
) -> list[str]:
    """Reject invented bindings only after every language denominator is known."""

    return [
        f"{code_id}: area binding names invented code identity"
        for code_id in sorted(set(area_bindings) - set(live_facts))
    ]


@dataclass(frozen=True)
class CodeSpecMetadata:
    code_map: Mapping[str, Any] = field(default_factory=dict)
    debt_baseline: Mapping[str, Any] = field(default_factory=dict)
    mappings: Mapping[str, Mapping[str, Any]] = field(default_factory=dict)
    spec_sources: Mapping[str, Mapping[str, Any]] = field(default_factory=dict)
    spec_gaps: Mapping[str, Mapping[str, Any]] = field(default_factory=dict)
    invariants: Mapping[str, Mapping[str, Any]] = field(default_factory=dict)
    tests: Mapping[str, Mapping[str, Any]] = field(default_factory=dict)
    risk_ack_document: Mapping[str, Any] = field(default_factory=dict)
    risk_acknowledgements: Mapping[str, Mapping[str, Any]] = field(default_factory=dict)

    @property
    def area_bindings(self) -> dict[str, str]:
        return {
            code_id: str(record["area"])
            for code_id, record in self.mappings.items()
            if isinstance(record.get("area"), str)
        }

    @property
    def debt_code_ids(self) -> frozenset[str]:
        entries = self.debt_baseline.get("entries", [])
        return frozenset(
            str(entry["code_id"])
            for entry in entries
            if isinstance(entry, Mapping) and isinstance(entry.get("code_id"), str)
        )


@dataclass(frozen=True)
class CodeSpecLiveState:
    metadata: CodeSpecMetadata
    rust: RustInventory
    typescript: TypeScriptInventory
    tooling: ToolingInventory
    tooling_bindings: Mapping[str, tuple[str, ...]]
    facts: Mapping[str, CodeFact]
    failures: tuple[str, ...]


def load_code_spec_metadata(root: Path) -> CodeSpecMetadata:
    """Load the exact metadata surfaces required by the code-to-spec join."""

    code_map = _load_toml(root / "verification/code-spec-map.toml")
    debt_baseline = _load_toml(root / "verification/code-spec-debt-baseline.toml")
    mappings = _index_records(code_map, "mappings", "code_id", "code-spec map")
    spec_source_document = _load_toml(root / "verification/spec-sources.toml")
    spec_gap_document = _load_toml(root / "verification/spec-gaps.toml")
    catalog_document = _load_toml(root / "verification/test-catalog.toml")
    risk_ack_document = _load_toml(
        root / "verification/code-spec-risk-acknowledgements.toml"
    )
    risk_acknowledgements, risk_ack_failures = index_risk_acknowledgements(
        risk_ack_document
    )
    if risk_ack_failures:
        raise CodeSpecLiveError("; ".join(risk_ack_failures))
    invariants: dict[str, Mapping[str, Any]] = {}
    invariant_root = root / "verification/invariants"
    if not invariant_root.is_dir():
        raise CodeSpecLiveError(f"missing invariant root {invariant_root}")
    for path in sorted(invariant_root.rglob("*.toml")):
        record = _load_toml(path)
        invariant_id = record.get("id")
        if not isinstance(invariant_id, str) or not invariant_id:
            raise CodeSpecLiveError(f"invariant {path} has no nonempty id")
        if invariant_id in invariants:
            raise CodeSpecLiveError(f"duplicate invariant id {invariant_id}")
        invariants[invariant_id] = record
    return CodeSpecMetadata(
        code_map=code_map,
        debt_baseline=debt_baseline,
        mappings=mappings,
        spec_sources=_index_records(
            spec_source_document,
            "spec_sources",
            "id",
            "spec source",
        ),
        spec_gaps=_index_records(spec_gap_document, "spec_gaps", "id", "spec gap"),
        invariants=dict(sorted(invariants.items())),
        tests=_index_records(catalog_document, "tests", "id", "test catalog"),
        risk_ack_document=risk_ack_document,
        risk_acknowledgements=risk_acknowledgements,
    )


def build_code_spec_live_state(root: Path) -> CodeSpecLiveState:
    """Load metadata, scan Rust, and execute the exact semantic join."""

    metadata = load_code_spec_metadata(root)
    matrix = _load_toml(root / "verification/matrix.toml")
    rust = invoke_rust_extractor(
        root,
        matrix,
        area_bindings=metadata.area_bindings,
        reject_invented_area_bindings=False,
    )
    typescript = invoke_typescript_extractor(
        root,
        matrix,
        area_bindings=metadata.area_bindings,
    )
    tooling = invoke_tooling_extractor(
        root,
        matrix,
        area_bindings=metadata.area_bindings,
    )
    fuzz_document = _load_toml(root / "verification/fuzz-program.toml")
    fuzz_targets = fuzz_document.get("targets")
    if not isinstance(fuzz_targets, list) or any(
        not isinstance(target, Mapping) for target in fuzz_targets
    ):
        raise CodeSpecLiveError("fuzz program targets must be an array of tables")
    fuzz_surfaces = fuzz_document.get("surfaces")
    if not isinstance(fuzz_surfaces, list) or any(
        not isinstance(surface, Mapping) for surface in fuzz_surfaces
    ):
        raise CodeSpecLiveError("fuzz program surfaces must be an array of tables")
    area_records = matrix.get("areas")
    if not isinstance(area_records, list) or any(
        not isinstance(area, Mapping) for area in area_records
    ):
        raise CodeSpecLiveError("verification matrix areas must be an array of tables")
    tooling_join = join_tooling_inventories(
        facts=tooling.facts,
        source_paths=tooling.source_paths,
        gate_records=load_gate_inventory(root),
        fuzz_targets=fuzz_targets,
        fuzz_surfaces=fuzz_surfaces,
        area_records=area_records,
    )
    facts = merge_fact_sets(rust.facts, typescript.facts, tooling.facts)
    map_failures = validate_map_document(
        metadata.code_map,
        live_facts=facts,
        spec_sources=metadata.spec_sources,
        spec_gaps=metadata.spec_gaps,
        invariants=metadata.invariants,
        tests=metadata.tests,
        risk_acknowledgements=metadata.risk_acknowledgements,
    )
    baseline_ids = metadata.debt_code_ids
    failures = [
        failure
        for failure in map_failures
        if not (
            failure.endswith(": missing_spec")
            and failure.split(":", 1)[0] in baseline_ids
        )
    ]
    failures.extend(
        validate_debt_partition(
            metadata.debt_baseline,
            live_facts=facts,
            mappings=metadata.mappings,
        )
    )
    failures.extend(validate_global_area_bindings(metadata.area_bindings, facts))
    failures.extend(tooling_join.failures)
    failures.extend(
        validate_risk_ack_document(
            metadata.risk_ack_document,
            live_facts=facts,
            mappings=metadata.mappings,
        )
    )
    failures.extend(f"{code_id}: unmapped" for code_id in rust.unmapped_code_ids)
    failures.extend(f"{code_id}: unmapped" for code_id in typescript.unmapped_code_ids)
    failures.extend(f"{code_id}: unmapped" for code_id in tooling.unmapped_code_ids)
    return CodeSpecLiveState(
        metadata=metadata,
        rust=rust,
        typescript=typescript,
        tooling=tooling,
        tooling_bindings=tooling_join.bindings,
        facts=facts,
        failures=tuple(sorted(set(failures))),
    )


def _load_toml(path: Path) -> dict[str, Any]:
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise CodeSpecLiveError(f"cannot load {path}: {error}") from error


def _index_records(
    document: Mapping[str, Any],
    field: str,
    id_field: str,
    label: str,
) -> dict[str, Mapping[str, Any]]:
    records = document.get(field)
    if not isinstance(records, list):
        raise CodeSpecLiveError(f"{label} root {field} must be an array")
    indexed: dict[str, Mapping[str, Any]] = {}
    for index, record in enumerate(records):
        if not isinstance(record, Mapping):
            raise CodeSpecLiveError(f"{label}[{index}] must be a table")
        record_id = record.get(id_field)
        if not isinstance(record_id, str) or not record_id:
            raise CodeSpecLiveError(f"{label}[{index}] has no nonempty {id_field}")
        if record_id in indexed:
            raise CodeSpecLiveError(f"duplicate {id_field} {record_id}")
        indexed[record_id] = record
    return dict(sorted(indexed.items()))
