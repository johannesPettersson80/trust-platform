"""Fail-closed contract for the Phase 4 invariant-seed import manifest."""

from __future__ import annotations

import json
import re
import subprocess
import tomllib
from collections import Counter
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Mapping

from .test_catalog_json_schema import validate_json_schema_instance


AREAS_PATH = "docs/internal/testing/checklists/plc-verification-program/verification-areas.md"
MANIFEST_PATH = "verification/invariant-seeds.toml"
MANIFEST_SCHEMA_PATH = "verification/schemas/invariant-seed-manifest.schema.json"
INVARIANT_ROOT = "verification/invariants"
P4_000_SEED_IDS = {
    "IEC_TIMER_001",
    "RT_SAFE_NAN_001",
    "SEC_AUTHZ_001",
    "PROTO_OPCUA_001",
    "RT_RELOAD_001",
}
HIGH_RISKS = {"safety_critical", "wrong_result", "silent_corruption", "false_status"}
NON_CLAIM_AUTHORITIES = {
    "normative_external",
    "normative_product",
    "reviewed_decision",
    "reviewed_deviation",
}
BOARD_ROW_AREAS = {
    "VERIF-P4-001": {"compiler_iec"},
    "VERIF-P4-002": {"bytecode_vm"},
    "VERIF-P4-003": {"runtime_safety"},
    "VERIF-P4-004": {"protocols"},
    "VERIF-P4-005": {"editor_safety", "plcopen_devtools"},
    "VERIF-P4-006": {"hmi_ui"},
    "VERIF-P4-007": {"release"},
    "VERIF-P4-008": {"control_security", "supply_chain_platform"},
}
PREEXISTING_CANONICAL_IDS = {
    "RT_SAFE_STOP_001",
    "VM_SEAM_DECLARED_TYPE_001",
    "VM_SEAM_REF_001",
    "VM_SEAM_OWNER_001",
    "VM_SEAM_VALID_001",
    "VM_SEAM_ENC_001",
    "PROTO_DISCOVERY_TRUTH_001",
}
CANONICAL_ALIASES = {
    "VM_SEAM_TYPE_001": "VM_SEAM_DECLARED_TYPE_001",
    "VM_SEAM_TYPE_002": "VM_SEAM_DECLARED_TYPE_001",
    "PROTO_DISC_001": "PROTO_DISCOVERY_TRUTH_001",
}
PERMITTED_MERGED_SEEDS = {
    frozenset({"VM_SEAM_TYPE_001", "VM_SEAM_TYPE_002"})
}
SEED_RE = re.compile(r"^- \[[ x]\] `([A-Z][A-Z0-9_]+_[0-9]{3})`\s+(.+)$")
CHECKBOX_RE = re.compile(r"^- \[[ x]\] ")


@dataclass(frozen=True)
class WrittenSeed:
    seed_id: str
    title: str
    section: str
    line: int


@dataclass(frozen=True)
class SeedAuditRow:
    seed_id: str
    seed_title: str
    source_section: str
    source_line: int
    canonical_invariant_id: str
    invariant_path: str
    invariant_area: str
    board_row: str
    origin: str
    status: str
    proof_level: str
    risk: str
    oracle_ref: str
    spec_gap_refs: tuple[str, ...]
    source_refs: tuple[str, ...]
    test_ids: tuple[str, ...]
    evidence_ids: tuple[str, ...]
    p4_000_risk_id: str | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "seed_id": self.seed_id,
            "seed_title": self.seed_title,
            "source_section": self.source_section,
            "source_line": self.source_line,
            "canonical_invariant_id": self.canonical_invariant_id,
            "invariant_path": self.invariant_path,
            "invariant_area": self.invariant_area,
            "board_row": self.board_row,
            "origin": self.origin,
            "status": self.status,
            "proof_level": self.proof_level,
            "risk": self.risk,
            "oracle_ref": self.oracle_ref,
            "spec_gap_refs": sorted(self.spec_gap_refs),
            "source_refs": sorted(self.source_refs),
            "test_ids": sorted(self.test_ids),
            "evidence_ids": sorted(self.evidence_ids),
            "p4_000_risk_id": self.p4_000_risk_id,
        }


@dataclass(frozen=True)
class SeedAudit:
    rows: tuple[SeedAuditRow, ...]


def extract_written_seeds(text: str) -> tuple[WrittenSeed, ...]:
    marker = "## Initial High-Risk Invariant Seeds"
    if text.count(marker) != 1:
        raise ValueError("verification areas must contain exactly one invariant-seed section")
    prefix, section_text = text.split(marker, 1)
    del prefix
    lines = section_text.splitlines()
    current_section = ""
    seeds: list[WrittenSeed] = []
    active: dict[str, Any] | None = None
    marker_line = text[: text.index(marker)].count("\n") + 1
    for offset, line in enumerate(lines[1:], start=2):
        absolute_line = marker_line + offset - 1
        if line.startswith("## "):
            break
        if line.startswith("### "):
            if active is not None:
                seeds.append(_finish_seed(active))
                active = None
            current_section = line[4:].strip()
            continue
        if CHECKBOX_RE.match(line):
            if active is not None:
                seeds.append(_finish_seed(active))
            match = SEED_RE.match(line)
            if match is None:
                raise ValueError(f"malformed invariant seed checklist row at line {absolute_line}")
            if not current_section:
                raise ValueError(f"invariant seed lacks a subsection at line {absolute_line}")
            active = {
                "seed_id": match.group(1),
                "parts": [match.group(2).strip()],
                "section": current_section,
                "line": absolute_line,
            }
            continue
        if active is not None and line.startswith("  ") and line.strip():
            active["parts"].append(line.strip())
        elif active is not None and not line.strip():
            seeds.append(_finish_seed(active))
            active = None
    if active is not None:
        seeds.append(_finish_seed(active))
    ids = [row.seed_id for row in seeds]
    duplicates = sorted(seed_id for seed_id, count in Counter(ids).items() if count > 1)
    if duplicates:
        raise ValueError(f"duplicate written seed IDs: {', '.join(duplicates)}")
    if len(seeds) != 44:
        raise ValueError(f"written invariant seed count must be 44, found {len(seeds)}")
    return tuple(seeds)


def load_seed_audit(root: Path) -> SeedAudit:
    root = root.resolve()
    areas = _read_text(root, AREAS_PATH)
    written = extract_written_seeds(areas)
    manifest = _read_toml(root, MANIFEST_PATH)
    schema = json.loads(_read_text(root, MANIFEST_SCHEMA_PATH))
    schema_failures = validate_json_schema_instance(manifest, schema)
    if schema_failures:
        raise ValueError("manifest schema: " + "; ".join(schema_failures))
    records = manifest.get("seeds")
    if manifest.get("schema_version") != 1 or not isinstance(records, list):
        raise ValueError("invariant-seed manifest must use schema_version 1 and a seeds array")
    invariants, invariant_paths = _load_invariants(root)
    spec_sources = _index_records(_read_toml(root, "verification/spec-sources.toml"), "spec_sources")
    spec_gaps = _index_records(_read_toml(root, "verification/spec-gaps.toml"), "spec_gaps")
    risks = _index_records(_read_toml(root, "verification/risk-register.toml"), "risks")
    tests = _index_records(_read_toml(root, "verification/test-catalog.toml"), "tests")
    evidence = _index_records(_read_toml(root, "verification/evidence-index.toml"), "evidence")

    audit = build_seed_audit_from_records(
        written_seed_text=areas,
        seed_records=records,
        invariants=invariants,
        invariant_paths=invariant_paths,
        spec_sources=spec_sources,
        spec_gaps=spec_gaps,
        risks=risks,
        tests=tests,
        evidence=evidence,
    )
    _validate_durable_review_sources(root, audit, spec_sources, risks)
    return audit


def validate_seed_records(
    *,
    written_seed_text: str,
    seed_records: list[Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    invariant_paths: Mapping[str, str],
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    risks: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    """Validate already-loaded records so in-memory corruption cannot bypass the join."""

    try:
        build_seed_audit_from_records(
            written_seed_text=written_seed_text,
            seed_records=seed_records,
            invariants=invariants,
            invariant_paths=invariant_paths,
            spec_sources=spec_sources,
            spec_gaps=spec_gaps,
            risks=risks,
            tests=tests,
            evidence=evidence,
        )
    except ValueError as exc:
        return [str(exc)]
    return []


def build_seed_audit_from_records(
    *,
    written_seed_text: str,
    seed_records: list[Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    invariant_paths: Mapping[str, str],
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    risks: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
) -> SeedAudit:
    written = extract_written_seeds(written_seed_text)
    records = seed_records
    expected_ids = [row.seed_id for row in written]
    actual_ids = [row.get("seed_id") for row in records if isinstance(row, Mapping)]
    if actual_ids != expected_ids:
        raise ValueError("manifest seed IDs do not exactly match written seed order")
    written_by_id = {row.seed_id: row for row in written}
    mapped: dict[str, list[str]] = {}
    rows: list[SeedAuditRow] = []
    p4_links: dict[str, str] = {}
    for record in records:
        if not isinstance(record, Mapping):
            raise ValueError("invariant-seed manifest contains a non-object record")
        required_fields = {
            "seed_id",
            "canonical_invariant_id",
            "board_row",
            "origin",
        }
        if not required_fields.issubset(record) or set(record) - (
            required_fields | {"p4_000_risk_id"}
        ):
            raise ValueError("invariant-seed manifest record fields drift from the closed contract")
        seed_id = str(record["seed_id"])
        canonical_id = str(record["canonical_invariant_id"])
        expected_canonical = CANONICAL_ALIASES.get(seed_id, seed_id)
        if canonical_id != expected_canonical:
            raise ValueError(
                f"{seed_id}: canonical invariant must be {expected_canonical}, found {canonical_id}"
            )
        mapped.setdefault(canonical_id, []).append(seed_id)
        invariant = invariants.get(canonical_id)
        if invariant is None:
            raise ValueError(f"{seed_id}: canonical invariant is missing: {canonical_id}")
        path = invariant_paths.get(canonical_id)
        if not isinstance(path, str):
            raise ValueError(f"{seed_id}: canonical invariant path is missing")
        board_row = str(record["board_row"])
        origin = str(record["origin"])
        raw_risk_id = record.get("p4_000_risk_id")
        risk_id = str(raw_risk_id) if isinstance(raw_risk_id, str) and raw_risk_id else None
        _validate_seed_row(
            seed=written_by_id[seed_id],
            canonical_id=canonical_id,
            invariant=invariant,
            invariant_path=path,
            board_row=board_row,
            origin=origin,
            risk_id=risk_id,
            spec_sources=spec_sources,
            spec_gaps=spec_gaps,
            risks=risks,
            tests=tests,
            evidence=evidence,
        )
        if risk_id is not None:
            p4_links[seed_id] = risk_id
        oracle = invariant.get("oracle")
        spec = invariant.get("spec")
        rows.append(
            SeedAuditRow(
                seed_id=seed_id,
                seed_title=written_by_id[seed_id].title,
                source_section=written_by_id[seed_id].section,
                source_line=written_by_id[seed_id].line,
                canonical_invariant_id=canonical_id,
                invariant_path=path,
                invariant_area=str(invariant["area"]),
                board_row=board_row,
                origin=origin,
                status=str(invariant["status"]),
                proof_level=str(invariant["proof_level"]),
                risk=str(invariant["risk"]),
                oracle_ref=str(oracle["ref"]) if isinstance(oracle, Mapping) else "",
                spec_gap_refs=tuple(invariant.get("spec_gap_refs", [])),
                source_refs=tuple(spec.get("source_refs", [])) if isinstance(spec, Mapping) else (),
                test_ids=tuple(invariant.get("tests", [])),
                evidence_ids=tuple(invariant.get("evidence_refs", [])),
                p4_000_risk_id=risk_id,
            )
        )
    for canonical_id, seed_ids in mapped.items():
        if len(seed_ids) > 1 and frozenset(seed_ids) not in PERMITTED_MERGED_SEEDS:
            raise ValueError(
                f"canonical invariant is mapped by multiple seeds without authorization: "
                f"{canonical_id}: {', '.join(seed_ids)}"
            )
    if set(p4_links) != P4_000_SEED_IDS or len(set(p4_links.values())) != 5:
        raise ValueError("P4-000 risk links do not match the five required reviewed findings")
    if set(row.board_row for row in rows) != set(BOARD_ROW_AREAS):
        raise ValueError("manifest must represent every board row VERIF-P4-001 through VERIF-P4-008")
    return SeedAudit(rows=tuple(rows))


def _validate_seed_row(
    *,
    seed: WrittenSeed,
    canonical_id: str,
    invariant: Mapping[str, Any],
    invariant_path: str,
    board_row: str,
    origin: str,
    risk_id: str | None,
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    risks: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
) -> None:
    prefix = seed.seed_id
    if invariant.get("id") != canonical_id:
        raise ValueError(f"{prefix}: invariant id does not match canonical identity")
    area = invariant.get("area")
    if board_row not in BOARD_ROW_AREAS or area not in BOARD_ROW_AREAS[board_row]:
        raise ValueError(f"{prefix}: invariant area {area!r} is not valid for {board_row}")
    expected_row = _expected_board_row(seed)
    if board_row != expected_row:
        raise ValueError(f"{prefix}: board row must be {expected_row}")
    expected_path = f"{INVARIANT_ROOT}/{area}/{canonical_id}.toml"
    if invariant_path != expected_path:
        raise ValueError(f"{prefix}: canonical invariant path must be {expected_path}")
    expected_origin = "preexisting" if canonical_id in PREEXISTING_CANONICAL_IDS else "phase4"
    if origin != expected_origin:
        raise ValueError(f"{prefix}: origin does not match the reviewed preexisting set")
    if invariant.get("status") not in {"gap_open", "spec_gap"}:
        raise ValueError(f"{prefix}: status must be gap_open or spec_gap")
    if invariant.get("proof_level") != "S0":
        raise ValueError(f"{prefix}: proof_level must remain S0")
    test_ids = _string_list(invariant.get("tests"), f"{prefix}: tests")
    evidence_ids = _string_list(invariant.get("evidence_refs"), f"{prefix}: evidence_refs")
    if origin == "phase4" and test_ids:
        raise ValueError(f"{prefix}: phase4 seed must start with empty tests")
    if origin == "phase4" and evidence_ids:
        raise ValueError(f"{prefix}: phase4 seed must start with empty evidence_refs")
    spec_gap_refs = _string_list(invariant.get("spec_gap_refs", []), f"{prefix}: spec_gap_refs")
    spec = invariant.get("spec")
    oracle = invariant.get("oracle")
    coverage = invariant.get("coverage")
    if not isinstance(spec, Mapping) or not isinstance(oracle, Mapping) or not isinstance(coverage, Mapping):
        raise ValueError(f"{prefix}: invariant requires spec, oracle, and coverage objects")
    source_refs = _string_list(spec.get("source_refs"), f"{prefix}: spec.source_refs")
    unknown_sources = sorted(set(source_refs) - set(spec_sources))
    if unknown_sources:
        raise ValueError(f"{prefix}: spec.source_refs are unknown: {', '.join(unknown_sources)}")
    oracle_ref = oracle.get("ref")
    cells = coverage.get("cells")
    if not isinstance(cells, list) or not cells:
        raise ValueError(f"{prefix}: coverage.cells must be non-empty")
    if invariant.get("status") == "gap_open":
        if not source_refs or oracle_ref not in source_refs:
            raise ValueError(f"{prefix}: every gap_open seed requires its written oracle source")
        if spec_gap_refs:
            raise ValueError(f"{prefix}: gap_open invariant cannot claim spec gaps")
        source = spec_sources.get(str(oracle_ref))
        if (
            source is None
            or source.get("source_status") != "active"
            or source.get("authority") not in NON_CLAIM_AUTHORITIES
        ):
            raise ValueError(
                f"{prefix}: gap_open oracle must use an active normative or reviewed source"
            )
        if source.get("oracle_eligible") is not True:
            raise ValueError(f"{prefix}: gap_open oracle source is provenance-only")
        if spec.get("status") != "specified":
            raise ValueError(f"{prefix}: gap_open invariant requires spec.status specified")
        for cell in cells:
            if not isinstance(cell, Mapping) or cell.get("state") != "gap_open" or "spec_gap_ref" in cell:
                raise ValueError(f"{prefix}: gap_open coverage cells must remain gap_open without a spec gap")
    else:
        if not spec_gap_refs or oracle_ref not in spec_gap_refs:
            raise ValueError(f"{prefix}: spec_gap oracle must reference a listed focused gap")
        for gap_id in spec_gap_refs:
            gap = spec_gaps.get(gap_id)
            if gap is None or gap.get("resolution_status") != "open":
                raise ValueError(f"{prefix}: spec gap must remain open: {gap_id}")
        focused_gap_ids = {str(oracle_ref)}
        for cell in cells:
            gap_id = cell.get("spec_gap_ref") if isinstance(cell, Mapping) else None
            if (
                not isinstance(cell, Mapping)
                or cell.get("state") != "spec_gap"
                or gap_id not in spec_gap_refs
                or gap_id not in spec_gaps
                or spec_gaps[gap_id].get("resolution_status") != "open"
            ):
                raise ValueError(f"{prefix}: coverage spec_gap_ref must use a listed open focused gap")
            focused_gap_ids.add(str(gap_id))
        for gap_id in focused_gap_ids:
            if canonical_id not in spec_gaps[gap_id].get("affected_invariants", []):
                raise ValueError(f"{prefix}: focused spec gap must name the canonical invariant: {gap_id}")
    for evidence_id in evidence_ids:
        item = evidence.get(evidence_id)
        if item is None or item.get("proof_kind") != "none":
            raise ValueError(f"{prefix}: preexisting evidence must use proof_kind none: {evidence_id}")
    for test_id in test_ids:
        item = tests.get(test_id)
        if item is None or item.get("spec_gap_ref") not in spec_gap_refs:
            raise ValueError(
                f"{prefix}: preexisting test must remain bound to an open invariant spec gap: {test_id}"
            )
    if seed.seed_id in P4_000_SEED_IDS:
        if risk_id is None:
            raise ValueError(f"{prefix}: P4-000 seed requires a review risk")
        risk = risks.get(risk_id)
        if risk is None:
            raise ValueError(f"{prefix}: P4-000 risk is missing: {risk_id}")
        if canonical_id not in risk.get("related_invariants", []):
            raise ValueError(f"{prefix}: P4-000 risk must link back to canonical invariant")
        if risk.get("area") != area or risk.get("status") != "planned":
            raise ValueError(f"{prefix}: P4-000 risk must match the invariant area and remain planned")
        risk_source_refs = _string_list(
            risk.get("source_refs"), f"{prefix}: P4-000 risk source_refs"
        )
        if not risk_source_refs:
            raise ValueError(f"{prefix}: P4-000 risk requires a reviewed source")
        for source_id in risk_source_refs:
            source = spec_sources.get(source_id)
            if (
                source is None
                or source.get("source_status") != "active"
                or source.get("authority") not in NON_CLAIM_AUTHORITIES
            ):
                raise ValueError(f"{prefix}: P4-000 risk source must be active and reviewed")
        for evidence_id in risk.get("evidence_refs", []):
            item = evidence.get(evidence_id)
            if item is None or item.get("proof_kind") != "none":
                raise ValueError(f"{prefix}: P4-000 risk evidence cannot close the finding")
    elif risk_id is not None:
        raise ValueError(f"{prefix}: only the five P4-000 seeds may carry P4-000 risk links")


def _finish_seed(active: Mapping[str, Any]) -> WrittenSeed:
    title = " ".join(active["parts"])
    return WrittenSeed(
        seed_id=str(active["seed_id"]),
        title=title,
        section=str(active["section"]),
        line=int(active["line"]),
    )


def _expected_board_row(seed: WrittenSeed) -> str:
    by_section = {
        "Runtime Safety": "VERIF-P4-003",
        "HIR/VM Seam": "VERIF-P4-002",
        "Compiler and IEC": "VERIF-P4-001",
        "PLCopen and Developer Tooling": "VERIF-P4-005",
        "Protocols": "VERIF-P4-004",
        "Security, Supply Chain, and Platform": "VERIF-P4-008",
        "Release and Docs": "VERIF-P4-007",
    }
    if seed.section in by_section:
        return by_section[seed.section]
    if seed.seed_id.startswith("EDIT_") or seed.seed_id == "DEBUG_PAUSE_001":
        return "VERIF-P4-005"
    if seed.seed_id == "UI_STATUS_001":
        return "VERIF-P4-006"
    if seed.seed_id in {"DEBUG_AUTH_001"}:
        return "VERIF-P4-008"
    raise ValueError(f"{seed.seed_id}: no reviewed Phase 4 board mapping")


def _load_invariants(root: Path) -> tuple[dict[str, Mapping[str, Any]], dict[str, str]]:
    records: dict[str, Mapping[str, Any]] = {}
    paths: dict[str, str] = {}
    invariant_root = root / INVARIANT_ROOT
    for path in sorted(invariant_root.rglob("*.toml")):
        relative = path.relative_to(root).as_posix()
        _validate_regular_path(root, relative)
        record = tomllib.loads(path.read_text())
        invariant_id = record.get("id")
        if not isinstance(invariant_id, str) or invariant_id in records:
            raise ValueError(f"duplicate or missing invariant id at {relative}")
        records[invariant_id] = record
        paths[invariant_id] = relative
    return records, paths


def _index_records(data: Mapping[str, Any], key: str) -> dict[str, Mapping[str, Any]]:
    rows = data.get(key)
    if not isinstance(rows, list):
        raise ValueError(f"metadata requires a {key} array")
    result: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if not isinstance(row, Mapping) or not isinstance(row.get("id"), str):
            raise ValueError(f"{key} contains a record without an id")
        record_id = str(row["id"])
        if record_id in result:
            raise ValueError(f"{key} contains duplicate id {record_id}")
        result[record_id] = row
    return result


def _string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        raise ValueError(f"{label} must be a string array")
    if len(value) != len(set(value)):
        raise ValueError(f"{label} must be duplicate-free")
    return value


def _read_text(root: Path, relative: str) -> str:
    _validate_regular_path(root, relative)
    return (root / relative).read_text()


def _read_toml(root: Path, relative: str) -> Mapping[str, Any]:
    return tomllib.loads(_read_text(root, relative))


def _validate_regular_path(root: Path, relative: str) -> None:
    path = PurePosixPath(relative)
    if not relative or path.is_absolute() or ".." in path.parts or "\\" in relative:
        raise ValueError(f"unsafe metadata path: {relative}")
    candidate = root
    for part in path.parts:
        candidate /= part
        if candidate.is_symlink():
            raise ValueError(f"metadata path contains symlink: {relative}")
    try:
        resolved = candidate.resolve(strict=True)
        resolved.relative_to(root)
    except (OSError, ValueError) as exc:
        raise ValueError(f"metadata path is missing or escapes workspace: {relative}") from exc
    if not resolved.is_file():
        raise ValueError(f"metadata path is not a regular file: {relative}")


def required_review_source_paths(
    audit: SeedAudit,
    spec_sources: Mapping[str, Mapping[str, Any]],
    risks: Mapping[str, Mapping[str, Any]],
) -> tuple[str, ...]:
    source_ids = {
        row.oracle_ref for row in audit.rows if row.status == "gap_open"
    }
    for row in audit.rows:
        if row.p4_000_risk_id is None:
            continue
        risk = risks.get(row.p4_000_risk_id, {})
        source_ids.update(risk.get("source_refs", []))
    paths: list[str] = []
    for source_id in sorted(source_ids):
        source = spec_sources.get(source_id)
        path = source.get("path") if isinstance(source, Mapping) else None
        if not isinstance(path, str) or not path:
            raise ValueError(f"review source {source_id} must identify a workspace path")
        paths.append(path)
    return tuple(sorted(set(paths)))


def _validate_durable_review_sources(
    root: Path,
    audit: SeedAudit,
    spec_sources: Mapping[str, Mapping[str, Any]],
    risks: Mapping[str, Mapping[str, Any]],
) -> None:
    for relative in required_review_source_paths(audit, spec_sources, risks):
        _validate_regular_path(root, relative)
        ignored = subprocess.run(
            ["git", "-C", str(root), "check-ignore", "-q", "--", relative],
            check=False,
            capture_output=True,
        )
        if ignored.returncode == 0:
            raise ValueError(f"review source path is gitignored: {relative}")
        tracked = subprocess.run(
            ["git", "-C", str(root), "ls-files", "--error-unmatch", "--", relative],
            check=False,
            capture_output=True,
        )
        if tracked.returncode != 0:
            raise ValueError(f"review source path is not git-tracked: {relative}")
