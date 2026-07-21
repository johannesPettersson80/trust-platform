"""Validated adapter for the Phase 17 Rust production-fact extractor."""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Mapping

from .code_spec_common import RISK_SIGNALS, CodeFact, sha256_digest, stable_code_id
from .code_spec_source_adapter import (
    CodeSpecSourceError,
    candidate_area_owners,
    governed_source_bytes,
    select_area_owner,
)


ROOT_FIELDS = frozenset({"schema_version", "cargo_targets", "sources", "diagnostics"})
TARGET_FIELDS = frozenset({"package", "target", "target_kind", "path", "module"})
SOURCE_FIELDS = frozenset({"path", "module", "cargo_bindings", "facts"})
FACT_FIELDS = frozenset(
    {
        "qualified_symbol",
        "item_kind",
        "visibility",
        "signature",
        "body_start",
        "body_end",
        "line",
        "risk_signals",
    }
)


class CodeSpecRustError(ValueError):
    """Raised when extractor output cannot form an authoritative denominator."""


@dataclass(frozen=True)
class RustInventory:
    facts: Mapping[str, CodeFact] = field(default_factory=dict)
    cargo_bindings: Mapping[str, tuple[str, ...]] = field(default_factory=dict)
    source_paths: tuple[str, ...] = ()
    target_count: int = 0
    source_count: int = 0
    unmapped_code_ids: tuple[str, ...] = ()


def rust_inventory_from_scan(
    root: Path,
    document: Mapping[str, Any],
    matrix: Mapping[str, Any],
    *,
    area_bindings: Mapping[str, str] | None = None,
    reject_invented_area_bindings: bool = True,
) -> RustInventory:
    """Convert closed extractor JSON to canonical live facts."""

    area_bindings = area_bindings or {}
    _require_exact_fields(document, ROOT_FIELDS, "Rust extractor root")
    if document.get("schema_version") != 1:
        raise CodeSpecRustError("Rust extractor schema_version must equal 1")
    diagnostics = _string_list(document.get("diagnostics"), "diagnostics")
    if diagnostics:
        raise CodeSpecRustError(
            "; ".join(f"extractor diagnostic: {diagnostic}" for diagnostic in diagnostics)
        )
    targets = _object_list(document.get("cargo_targets"), "cargo_targets")
    sources = _object_list(document.get("sources"), "sources")
    if not targets:
        raise CodeSpecRustError("Rust extractor returned zero cargo targets")
    if not sources:
        raise CodeSpecRustError("Rust extractor returned zero Rust sources")

    known_bindings: set[str] = set()
    for index, target in enumerate(targets):
        label = f"cargo_targets[{index}]"
        _require_exact_fields(target, TARGET_FIELDS, label)
        package = _string(target.get("package"), f"{label}.package")
        target_name = _string(target.get("target"), f"{label}.target")
        target_kind = _string(target.get("target_kind"), f"{label}.target_kind")
        _string(target.get("path"), f"{label}.path")
        _string(target.get("module"), f"{label}.module")
        binding = f"{package}:{target_name}:{target_kind}"
        if binding in known_bindings:
            raise CodeSpecRustError(f"duplicate cargo target binding {binding}")
        known_bindings.add(binding)

    facts: dict[str, CodeFact] = {}
    fact_bindings: dict[str, tuple[str, ...]] = {}
    source_paths: set[str] = set()
    seen_sources: set[tuple[str, str]] = set()
    for source_index, source_record in enumerate(sources):
        label = f"sources[{source_index}]"
        _require_exact_fields(source_record, SOURCE_FIELDS, label)
        try:
            path, source_bytes = governed_source_bytes(
                root,
                _string(source_record.get("path"), f"{label}.path"),
            )
        except CodeSpecSourceError as error:
            raise CodeSpecRustError(str(error)) from error
        module = _string(source_record.get("module"), f"{label}.module")
        source_key = (path, module)
        if source_key in seen_sources:
            raise CodeSpecRustError(f"duplicate Rust source/module pair {path}::{module}")
        seen_sources.add(source_key)
        source_paths.add(path)
        bindings = tuple(_string_list(source_record.get("cargo_bindings"), f"{label}.cargo_bindings"))
        if not bindings:
            raise CodeSpecRustError(f"{label}.cargo_bindings must not be empty")
        unknown_bindings = sorted(set(bindings) - known_bindings)
        if unknown_bindings:
            raise CodeSpecRustError(f"{label} has unknown cargo bindings {unknown_bindings}")
        source_facts = _object_list(source_record.get("facts"), f"{label}.facts")
        try:
            candidate_owners = candidate_area_owners(matrix, path)
        except CodeSpecSourceError as error:
            raise CodeSpecRustError(str(error)) from error
        for fact_index, record in enumerate(source_facts):
            fact_label = f"{label}.facts[{fact_index}]"
            _require_exact_fields(record, FACT_FIELDS, fact_label)
            symbol = _string(record.get("qualified_symbol"), f"{fact_label}.qualified_symbol")
            item_kind = _string(record.get("item_kind"), f"{fact_label}.item_kind")
            visibility = _string(record.get("visibility"), f"{fact_label}.visibility")
            signature = _string(record.get("signature"), f"{fact_label}.signature")
            body_start = _integer(record.get("body_start"), f"{fact_label}.body_start")
            body_end = _integer(record.get("body_end"), f"{fact_label}.body_end")
            line = _integer(record.get("line"), f"{fact_label}.line")
            risk_signals = tuple(
                sorted(_string_list(record.get("risk_signals"), f"{fact_label}.risk_signals"))
            )
            unknown_risks = sorted(set(risk_signals) - RISK_SIGNALS)
            if unknown_risks:
                raise CodeSpecRustError(f"{fact_label} has unknown risk signals {unknown_risks}")
            if body_start < 0 or body_end < body_start or body_end > len(source_bytes):
                raise CodeSpecRustError(
                    f"{fact_label} has invalid body byte range {body_start}..{body_end} "
                    f"for {len(source_bytes)} bytes"
                )
            if line < 1:
                raise CodeSpecRustError(f"{fact_label}.line must be positive")
            code_id = stable_code_id(
                source_kind="rust",
                path=path,
                qualified_symbol=symbol,
                item_kind=item_kind,
                signature=signature,
            )
            if code_id in facts:
                previous = facts[code_id]
                raise CodeSpecRustError(
                    f"duplicate live code identity {code_id}: "
                    f"{previous.path}:{previous.line} and {path}:{line} "
                    f"for {symbol} ({item_kind})"
                )
            try:
                area, owner = select_area_owner(code_id, candidate_owners, area_bindings)
            except CodeSpecSourceError as error:
                raise CodeSpecRustError(str(error)) from error
            if item_kind == "rust_unsafe_site" and "unsafe_or_ffi" not in risk_signals:
                raise CodeSpecRustError(f"{fact_label} unsafe site lacks unsafe_or_ffi risk signal")
            facts[code_id] = CodeFact(
                code_id=code_id,
                source_kind="rust",
                path=path,
                qualified_symbol=symbol,
                item_kind=item_kind,
                signature=signature,
                signature_digest=sha256_digest(signature.encode("utf-8")),
                body_digest=sha256_digest(source_bytes[body_start:body_end]),
                owner=owner,
                area=area,
                area_candidates=tuple(sorted(candidate_owners)),
                visibility=visibility,
                risk_signals=risk_signals,
                line=line,
            )
            fact_bindings[code_id] = tuple(sorted(set(bindings)))
    if not facts:
        raise CodeSpecRustError("Rust extractor returned zero Rust facts")
    invented_bindings = sorted(set(area_bindings) - set(facts))
    if reject_invented_area_bindings and invented_bindings:
        raise CodeSpecRustError(f"area bindings name invented code identities {invented_bindings}")
    return RustInventory(
        facts=dict(sorted(facts.items())),
        cargo_bindings=dict(sorted(fact_bindings.items())),
        source_paths=tuple(sorted(source_paths)),
        target_count=len(targets),
        source_count=len(sources),
        unmapped_code_ids=tuple(
            code_id for code_id, fact in sorted(facts.items()) if fact.area == "unmapped"
        ),
    )


def invoke_rust_extractor(
    root: Path,
    matrix: Mapping[str, Any],
    *,
    area_bindings: Mapping[str, str] | None = None,
    reject_invented_area_bindings: bool = True,
) -> RustInventory:
    """Run the canonical xtask extractor and validate its JSON output."""

    completed = subprocess.run(
        ["cargo", "run", "-q", "-p", "xtask", "--", "code-spec-rust-all"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    try:
        document = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        detail = completed.stderr.strip() or completed.stdout.strip() or "no extractor output"
        raise CodeSpecRustError(f"Rust extractor did not emit canonical JSON: {detail}") from error
    if not isinstance(document, Mapping):
        raise CodeSpecRustError("Rust extractor JSON root must be an object")
    return rust_inventory_from_scan(
        root,
        document,
        matrix,
        area_bindings=area_bindings,
        reject_invented_area_bindings=reject_invented_area_bindings,
    )


def _require_exact_fields(record: Mapping[str, Any], expected: frozenset[str], label: str) -> None:
    actual = set(record)
    if actual != expected:
        raise CodeSpecRustError(
            f"{label} fields drift: missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
        )


def _object_list(value: Any, label: str) -> list[Mapping[str, Any]]:
    if not isinstance(value, list) or any(not isinstance(item, Mapping) for item in value):
        raise CodeSpecRustError(f"{label} must be an array of objects")
    return value


def _string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        raise CodeSpecRustError(f"{label} must be an array of nonempty strings")
    return value


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise CodeSpecRustError(f"{label} must be a nonempty string")
    return value


def _integer(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise CodeSpecRustError(f"{label} must be an integer")
    return value
