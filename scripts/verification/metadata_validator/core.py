#!/usr/bin/env python3
"""Validate the truST verification metadata control plane.

This is intentionally a thin, dependency-free validator. It checks the committed
metadata relationships that Phase 1 depends on without running Rust, Node,
browser, network, or hardware tests.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .constants import (
    AREAS,
    BLOCKS_VALUES,
    CASE_FAMILIES,
    COMMIT_RE,
    CONTRACT_KINDS,
    COVERAGE_STATES,
    DELTA_KEYS,
    DELTA_VALUES,
    EVIDENCE_KINDS,
    GAP_CLASSES,
    HIGH_RISKS,
    INTENTS,
    ORACLE_KINDS,
    OUTCOMES,
    PARTITION_KEYS,
    PROOF_KINDS,
    PROOF_LEVELS,
    PROOF_LEVELS_VALIDATED,
    PROVE_PRODUCER_RE,
    RESOLUTION_STATUSES,
    RISKS,
    ROOT,
    SCHEMA_FILES,
    SCHEMA_REQUIRED_FIELDS,
    SOURCE_AUTHORITIES,
    SOURCE_STATUSES,
    SPEC_STATUSES,
    STATUSES,
    SUITE_AREA,
    TEST_CLASSES,
    VERIFICATION,
)
from .case_files import validate_case_file
from .evidence_proof import validate_green_pairing, validate_lock_pairing
from .integrity import (
    test_counts_as_runnable,
    validate_open_spec_gap_references,
    validate_runnable_test_path,
)
from .oracle_refs import (
    validate_error_code_ref,
    validate_oracle_ref,
    validate_partition_contract,
)
from .taxonomy import validate_taxonomy_drift


@dataclass
class Failure:
    path: Path
    message: str


class Validator:
    def __init__(self) -> None:
        self.failures: list[Failure] = []
        self.spec_sources: dict[str, dict[str, Any]] = {}
        self.spec_gaps: dict[str, dict[str, Any]] = {}
        self.evidence: dict[str, dict[str, Any]] = {}
        self.invariants: dict[str, dict[str, Any]] = {}
        self.suites: dict[str, dict[str, Any]] = {}
        self.tests: dict[str, dict[str, Any]] = {}
        self.ignored_tests: dict[str, dict[str, Any]] = {}
        self.risks: dict[str, dict[str, Any]] = {}
        self.required_specs: dict[str, dict[str, Any]] = {}
        self.matrix: dict[str, Any] = {}

    def fail(self, path: Path, message: str) -> None:
        self.failures.append(Failure(path.relative_to(ROOT), message))

    def load_toml(self, path: Path) -> dict[str, Any]:
        try:
            return tomllib.loads(path.read_text())
        except Exception as exc:  # pragma: no cover - message path matters
            self.fail(path, f"TOML parse failed: {exc}")
            return {}

    def check_no_empty_strings(self, path: Path, value: Any, where: str = "$") -> None:
        if isinstance(value, str):
            if value == "":
                self.fail(path, f"empty-string sentinel at {where}")
        elif isinstance(value, list):
            for index, item in enumerate(value):
                self.check_no_empty_strings(path, item, f"{where}[{index}]")
        elif isinstance(value, dict):
            for key, item in value.items():
                self.check_no_empty_strings(path, item, f"{where}.{key}")

    def require(self, path: Path, record: dict[str, Any], fields: list[str], kind: str) -> None:
        for field in fields:
            if "." in field:
                table, nested = field.split(".", 1)
                if not isinstance(record.get(table), dict) or nested not in record[table]:
                    self.fail(path, f"{kind} {record.get('id', '<unknown>')} missing {field}")
            elif field not in record:
                self.fail(path, f"{kind} {record.get('id', '<unknown>')} missing {field}")

    def check_schema_version(self, path: Path, record: dict[str, Any], kind: str) -> None:
        if record.get("schema_version") != 1:
            self.fail(path, f"{kind} {record.get('id', '<unknown>')} must use schema_version = 1")

    def check_area(self, path: Path, record: dict[str, Any], allow_suite: bool = False) -> None:
        area = record.get("area")
        allowed = set(AREAS)
        if allow_suite:
            allowed.add(SUITE_AREA)
        if area not in allowed:
            self.fail(path, f"{record.get('id', '<unknown>')} uses unknown area {area!r}")

    def check_status(self, path: Path, record: dict[str, Any]) -> None:
        if record.get("status") not in STATUSES:
            self.fail(path, f"{record.get('id', '<unknown>')} uses unknown status {record.get('status')!r}")

    def register(self, target: dict[str, dict[str, Any]], path: Path, record: dict[str, Any], kind: str) -> None:
        record_id = record.get("id")
        if not isinstance(record_id, str) or not record_id:
            self.fail(path, f"{kind} record has missing/invalid id")
            return
        if record_id in target:
            self.fail(path, f"duplicate {kind} id {record_id}")
            return
        record["_path"] = path
        target[record_id] = record

    def load_json_schemas(self) -> None:
        schema_dir = VERIFICATION / "schemas"
        for name in sorted(SCHEMA_FILES):
            path = schema_dir / name
            if not path.exists():
                self.fail(path, "schema file missing")
                continue
            try:
                data = json.loads(path.read_text())
            except Exception as exc:
                self.fail(path, f"JSON schema parse failed: {exc}")
                continue
            for field in ("$schema", "$id", "title", "type"):
                if field not in data:
                    self.fail(path, f"schema missing {field}")
            if data.get("type") != "object":
                self.fail(path, "schema root type must be object")
            if data.get("required") != SCHEMA_REQUIRED_FIELDS[name]:
                self.fail(path, "schema required fields drift from validator contract")
            self.check_schema_enums(path, name, data)

    def load_records(self) -> None:
        self.load_json_schemas()
        self.load_wrapped_records(
            VERIFICATION / "spec-sources.toml",
            "spec_sources",
            self.spec_sources,
            "spec source",
        )
        self.load_wrapped_records(
            VERIFICATION / "spec-gaps.toml",
            "spec_gaps",
            self.spec_gaps,
            "spec gap",
        )
        self.load_wrapped_records(
            VERIFICATION / "evidence-index.toml",
            "evidence",
            self.evidence,
            "evidence",
        )
        self.load_wrapped_records(
            VERIFICATION / "spec-matrix.toml",
            "required_specs",
            self.required_specs,
            "required spec",
        )
        self.matrix = self.load_toml(VERIFICATION / "matrix.toml")
        self.check_no_empty_strings(VERIFICATION / "matrix.toml", self.matrix)
        self.load_optional_wrapped_records(VERIFICATION / "test-catalog.toml", "tests", self.tests, "test")
        self.load_optional_wrapped_records(
            VERIFICATION / "ignored-tests.toml",
            "ignored_tests",
            self.ignored_tests,
            "ignored test",
        )
        self.load_optional_wrapped_records(VERIFICATION / "risk-register.toml", "risks", self.risks, "risk")

        for path in sorted((VERIFICATION / "invariants").rglob("*.toml")):
            relative = path.relative_to(VERIFICATION / "invariants")
            if len(relative.parts) != 2:
                self.fail(path, "invariant file must live at verification/invariants/<area>/<ID>.toml")
                continue
            record = self.load_toml(path)
            self.check_no_empty_strings(path, record)
            self.register(self.invariants, path, record, "invariant")

        for path in sorted((VERIFICATION / "suites").glob("*.toml")):
            record = self.load_toml(path)
            self.check_no_empty_strings(path, record)
            self.register(self.suites, path, record, "suite")

    def check_schema_enums(self, path: Path, name: str, schema: dict[str, Any]) -> None:
        expectations = {
            "catalog.schema.json": {"test_class": TEST_CLASSES},
            "evidence.schema.json": {"kind": EVIDENCE_KINDS, "proof_kind": PROOF_KINDS},
            "invariant.schema.json": {
                "risk": RISKS,
                "contract_kind": CONTRACT_KINDS,
                "proof_level": PROOF_LEVELS,
            },
            "spec-gap.schema.json": {"gap_class": GAP_CLASSES},
        }
        properties = schema.get("properties", {})
        for field, expected_values in expectations.get(name, {}).items():
            actual = properties.get(field, {}).get("enum")
            if set(actual or []) != expected_values:
                self.fail(path, f"schema enum for {field} drifts from validator vocabulary")

    def load_wrapped_records(
        self,
        path: Path,
        key: str,
        target: dict[str, dict[str, Any]],
        kind: str,
    ) -> None:
        data = self.load_toml(path)
        self.check_no_empty_strings(path, data)
        records = data.get(key)
        if not isinstance(records, list):
            self.fail(path, f"expected [[{key}]] wrapper")
            return
        for record in records:
            if not isinstance(record, dict):
                self.fail(path, f"{kind} entry is not a table")
                continue
            self.register(target, path, record, kind)

    def load_optional_wrapped_records(
        self,
        path: Path,
        key: str,
        target: dict[str, dict[str, Any]],
        kind: str,
    ) -> None:
        data = self.load_toml(path)
        self.check_no_empty_strings(path, data)
        if not data:
            return
        records = data.get(key)
        if not isinstance(records, list):
            self.fail(path, f"expected [[{key}]] wrapper or an empty file")
            return
        for record in records:
            if not isinstance(record, dict):
                self.fail(path, f"{kind} entry is not a table")
                continue
            self.register(target, path, record, kind)

    def validate(self) -> None:
        validate_taxonomy_drift(self.fail)
        self.validate_spec_sources()
        self.validate_spec_gaps()
        self.validate_suites()
        self.validate_invariants()
        self.validate_required_specs()
        self.validate_matrix()
        self.validate_evidence()
        self.validate_tests()
        self.validate_ignored_tests()
        self.validate_risks()
        self.validate_public_claim_links()
        validate_open_spec_gap_references(
            fail=self.fail,
            spec_gaps=self.spec_gaps,
            required_specs=self.required_specs,
            invariants=self.invariants,
            tests=self.tests,
            risks=self.risks,
        )

    def validate_spec_sources(self) -> None:
        required = [
            "schema_version",
            "id",
            "title",
            "area",
            "owner",
            "status",
            "authority",
            "source_status",
            "visibility",
            "covers",
            "known_limitations",
        ]
        for record in self.spec_sources.values():
            path = record["_path"]
            self.require(path, record, required, "spec source")
            self.check_common(path, record)
            if record.get("authority") not in SOURCE_AUTHORITIES:
                self.fail(path, f"{record['id']} has unknown authority {record.get('authority')!r}")
            if record.get("source_status") not in SOURCE_STATUSES:
                self.fail(path, f"{record['id']} has unknown source_status {record.get('source_status')!r}")
            if "path" not in record and "external_ref" not in record:
                self.fail(path, f"{record['id']} must have path or external_ref")
            if "path" in record and not (ROOT / record["path"]).exists():
                self.fail(path, f"{record['id']} path does not exist: {record['path']}")
            if not isinstance(record.get("covers"), list) or not record["covers"]:
                self.fail(path, f"{record['id']} must cover at least one tag")
            if record.get("authority") == "public_claim":
                for field in ("claim_text", "surface_ref"):
                    if field not in record:
                        self.fail(path, f"public claim {record['id']} missing {field}")

    def validate_spec_gaps(self) -> None:
        required = [
            "schema_version",
            "id",
            "title",
            "area",
            "risk",
            "owner",
            "status",
            "gap_class",
            "blocking_question",
            "affected_invariants",
            "candidate_spec_sources",
            "resolution_status",
            "last_reviewed",
        ]
        for record in self.spec_gaps.values():
            path = record["_path"]
            self.require(path, record, required, "spec gap")
            self.check_common(path, record)
            if record.get("status") != "spec_gap":
                self.fail(path, f"spec gap {record['id']} must use status = spec_gap")
            if record.get("gap_class") not in GAP_CLASSES:
                self.fail(path, f"{record['id']} has unknown gap_class {record.get('gap_class')!r}")
            if record.get("risk") not in RISKS:
                self.fail(path, f"{record['id']} has unknown risk {record.get('risk')!r}")
            if record.get("resolution_status") not in RESOLUTION_STATUSES:
                self.fail(path, f"{record['id']} has unknown resolution_status {record.get('resolution_status')!r}")
            for source_id in record.get("candidate_spec_sources", []):
                if source_id not in self.spec_sources:
                    self.fail(path, f"{record['id']} references unknown candidate source {source_id}")
            for evidence_id in record.get("closeout_evidence", []):
                if evidence_id not in self.evidence:
                    self.fail(path, f"{record['id']} references unknown closeout evidence {evidence_id}")
            for invariant_id in record.get("affected_invariants", []):
                if invariant_id not in self.invariants:
                    self.fail(path, f"{record['id']} references unknown affected invariant {invariant_id}")

    def validate_suites(self) -> None:
        required = [
            "schema_version",
            "id",
            "title",
            "area",
            "owner",
            "status",
            "last_reviewed",
            "purpose",
            "duration_class",
            "environment",
            "commands",
            "evidence_destination",
        ]
        for record in self.suites.values():
            path = record["_path"]
            self.require(path, record, required, "suite")
            self.check_common(path, record, allow_suite=True)
            if record.get("area") != SUITE_AREA:
                self.fail(path, f"suite {record['id']} must use area = suite")
            for suite_id in record.get("includes", []):
                if suite_id not in self.suites:
                    self.fail(path, f"{record['id']} includes unknown suite {suite_id}")

    def validate_invariants(self) -> None:
        required = [
            "schema_version",
            "id",
            "title",
            "area",
            "risk",
            "status",
            "owner",
            "claim",
            "contract_kind",
            "spec.status",
            "oracle.kind",
            "oracle.ref",
            "proof_level",
            "tests",
            "gates",
            "missing",
            "coverage",
        ]
        for record in self.invariants.values():
            path = record["_path"]
            self.require(path, record, required, "invariant")
            self.check_common(path, record)
            relative = path.relative_to(VERIFICATION / "invariants")
            if relative.parts[0] != record.get("area"):
                self.fail(path, f"{record['id']} area {record.get('area')!r} does not match directory {relative.parts[0]!r}")
            if path.stem != record.get("id"):
                self.fail(path, f"{record['id']} filename must match invariant id")
            if record.get("risk") not in RISKS:
                self.fail(path, f"{record['id']} has unknown risk {record.get('risk')!r}")
            if record.get("proof_level") not in PROOF_LEVELS:
                self.fail(path, f"{record['id']} has unknown proof_level {record.get('proof_level')!r}")
            if record.get("contract_kind") not in CONTRACT_KINDS:
                self.fail(path, f"{record['id']} has unknown contract_kind {record.get('contract_kind')!r}")
            spec = record.get("spec", {})
            if spec.get("status") not in SPEC_STATUSES:
                self.fail(path, f"{record['id']} has unknown spec.status {spec.get('status')!r}")
            if not spec.get("source_refs") and not record.get("spec_gap_refs"):
                self.fail(path, f"{record['id']} must name spec.source_refs or spec_gap_refs")
            oracle = record.get("oracle", {})
            if oracle.get("kind") not in ORACLE_KINDS:
                self.fail(path, f"{record['id']} has unknown oracle.kind {oracle.get('kind')!r}")
            if record.get("status") == "spec_gap":
                if oracle.get("ref") not in record.get("spec_gap_refs", []):
                    self.fail(path, f"{record['id']} spec_gap oracle.ref must name one of spec_gap_refs")
            elif oracle.get("ref") in self.spec_gaps:
                self.fail(path, f"{record['id']} non-spec-gap oracle.ref cannot name a spec gap")
            self.check_refs(path, record.get("tests", []), self.tests, "test", record["id"])
            self.check_refs(path, record.get("gates", []), self.suites, "suite", record["id"])
            self.check_refs(path, record.get("evidence_refs", []), self.evidence, "evidence", record["id"])
            self.check_refs(path, record.get("spec_gap_refs", []), self.spec_gaps, "spec gap", record["id"])
            for source_id in spec.get("source_refs", []):
                if source_id not in self.spec_sources:
                    self.fail(path, f"{record['id']} references unknown spec source {source_id}")
            cells = record.get("coverage", {}).get("cells")
            if not isinstance(cells, list) or not cells:
                self.fail(path, f"{record['id']} must have coverage.cells")
            else:
                for cell in cells:
                    self.validate_coverage_cell(path, record, cell)
            for behavior in record.get("behavior", []):
                self.validate_behavior(path, record, behavior)
            if record.get("status") == "validated":
                self.validate_validated_invariant(path, record)
            if record.get("status") == "test_written" and not record.get("tests"):
                self.fail(path, f"{record['id']} is test_written without tests")
            if record.get("status") == "implemented" and (not record.get("tests") or not record.get("evidence_refs")):
                self.fail(path, f"{record['id']} is implemented without tests and evidence")
            if record.get("status") in {"implemented", "validated"} and record.get("risk") in HIGH_RISKS:
                if not self.has_closing_high_risk_evidence(record):
                    self.fail(path, f"{record['id']} high-risk {record.get('status')} lacks allowlisted green/lock evidence that back-links the invariant")

    def validate_coverage_cell(self, path: Path, record: dict[str, Any], cell: dict[str, Any]) -> None:
        dimension = cell.get("dimension")
        if dimension not in CASE_FAMILIES:
            self.fail(path, f"{record['id']} has unknown coverage dimension {dimension!r}")
        state = cell.get("state")
        if state not in COVERAGE_STATES:
            self.fail(path, f"{record['id']} has unknown coverage state {state!r}")
        if state == "spec_gap":
            gap_id = cell.get("spec_gap_ref")
            if gap_id not in self.spec_gaps:
                self.fail(path, f"{record['id']} coverage cell references unknown spec_gap_ref {gap_id!r}")
        if state in {"covered", "covered_by_fuzz"} and not record.get("tests"):
            self.fail(path, f"{record['id']} coverage cell {dimension} is {state} without tests")
        if state == "not_applicable":
            decision_ref = cell.get("decision_ref")
            source = self.spec_sources.get(decision_ref)
            if not source or source.get("authority") not in {"reviewed_decision", "reviewed_deviation"} or source.get("source_status") != "active":
                self.fail(path, f"{record['id']} not_applicable cell requires active reviewed decision/deviation decision_ref")

    def validate_behavior(self, path: Path, record: dict[str, Any], behavior: dict[str, Any]) -> None:
        partition = behavior.get("partition")
        if not isinstance(partition, dict) or not partition:
            self.fail(path, f"{record['id']} behavior must define a partition table")
        else:
            unknown_keys = set(partition) - PARTITION_KEYS
            if unknown_keys:
                self.fail(path, f"{record['id']} behavior partition has unknown keys {sorted(unknown_keys)}")
            validate_partition_contract(fail=self.fail, path=path, owner_id=record["id"], behavior=behavior)
        if "oracle_ref" not in behavior and "spec_gap_ref" not in behavior:
            self.fail(path, f"{record['id']} behavior must name oracle_ref or spec_gap_ref")
        if "oracle_ref" in behavior and "spec_gap_ref" in behavior:
            self.fail(path, f"{record['id']} behavior cannot use oracle_ref and spec_gap_ref together")
        if "spec_gap_ref" in behavior and behavior["spec_gap_ref"] not in self.spec_gaps:
            self.fail(path, f"{record['id']} behavior references unknown spec_gap_ref {behavior['spec_gap_ref']}")
        if "spec_gap_ref" in behavior:
            forbidden = {"outcome", "delta", "error_code", "no_partial_apply", "fault_surface"} & set(behavior)
            if forbidden:
                self.fail(path, f"{record['id']} spec-gap behavior cannot carry expected outcome fields {sorted(forbidden)}")
            return
        validate_oracle_ref(
            fail=self.fail,
            path=path,
            owner_id=record["id"],
            oracle_ref=behavior.get("oracle_ref"),
            spec_sources=self.spec_sources,
        )
        validate_error_code_ref(
            fail=self.fail,
            path=path,
            owner_id=record["id"],
            behavior=behavior,
            spec_sources=self.spec_sources,
        )
        if behavior.get("outcome") not in OUTCOMES:
            self.fail(path, f"{record['id']} has unknown behavior outcome {behavior.get('outcome')!r}")
        delta = behavior.get("delta")
        if not isinstance(delta, dict):
            self.fail(path, f"{record['id']} behavior must use structured delta")
            return
        extra = set(delta) - DELTA_KEYS
        missing = DELTA_KEYS - set(delta)
        if extra:
            self.fail(path, f"{record['id']} behavior delta has unknown keys {sorted(extra)}")
        if missing:
            self.fail(path, f"{record['id']} behavior delta missing keys {sorted(missing)}")
        for key, value in delta.items():
            if value not in DELTA_VALUES.get(key, set()):
                self.fail(path, f"{record['id']} behavior delta.{key} has invalid value {value!r}")
            if value == "expected_delta" and not (behavior.get("expected_delta_ref") or behavior.get("notes") or behavior.get("rationale")):
                self.fail(path, f"{record['id']} behavior uses expected_delta without oracle-cited expected_delta_ref/notes")

    def validate_validated_invariant(self, path: Path, record: dict[str, Any]) -> None:
        if record.get("proof_level") not in PROOF_LEVELS_VALIDATED:
            self.fail(path, f"{record['id']} validated with insufficient proof_level {record.get('proof_level')!r}")
        if not record.get("tests"):
            self.fail(path, f"{record['id']} validated without tests")
        if not record.get("evidence_refs"):
            self.fail(path, f"{record['id']} validated without evidence_refs")
        if record.get("spec", {}).get("status") != "specified":
            self.fail(path, f"{record['id']} validated without spec.status = specified")
        if record.get("risk") in HIGH_RISKS:
            for cell in record.get("coverage", {}).get("cells", []):
                if cell.get("state") in {"gap_open", "spec_gap"}:
                    self.fail(path, f"{record['id']} high-risk validated with open coverage cell {cell.get('dimension')}")

    def validate_required_specs(self) -> None:
        required = [
            "schema_version",
            "id",
            "area",
            "tag",
            "title",
            "owner",
            "status",
            "expected_authority",
            "blocks",
            "last_reviewed",
        ]
        for record in self.required_specs.values():
            path = record["_path"]
            self.require(path, record, required, "required spec")
            self.check_common(path, record)
            if record.get("blocks") not in BLOCKS_VALUES:
                self.fail(path, f"{record['id']} has invalid blocks value {record.get('blocks')!r}")
            if record.get("waived") is True:
                decision_ref = record.get("decision_ref")
                source = self.spec_sources.get(decision_ref)
                if not source or source.get("authority") not in {"reviewed_decision", "reviewed_deviation"} or source.get("source_status") != "active":
                    self.fail(path, f"{record['id']} waived matrix row requires active reviewed decision/deviation decision_ref")
            source_ref = record.get("source_ref")
            gap_ref = record.get("spec_gap_ref")
            if bool(source_ref) == bool(gap_ref):
                self.fail(path, f"{record['id']} must have exactly one of source_ref or spec_gap_ref")
            if source_ref:
                self.validate_required_spec_source(path, record, source_ref)
            if gap_ref:
                self.validate_required_spec_gap(path, record, gap_ref)

    def validate_required_spec_source(self, path: Path, record: dict[str, Any], source_ref: str) -> None:
        source = self.spec_sources.get(source_ref)
        if not source:
            self.fail(path, f"{record['id']} references unknown source_ref {source_ref}")
            return
        if source.get("source_status") != "active":
            self.fail(path, f"{record['id']} source_ref {source_ref} is not active")
        if source.get("authority") not in record.get("expected_authority", []):
            self.fail(path, f"{record['id']} source_ref {source_ref} authority {source.get('authority')!r} not allowed")
        if source.get("authority") == "public_claim" and record.get("blocks") == "test_mapping":
            self.fail(path, f"{record['id']} cannot satisfy test_mapping with public_claim")
        if record.get("tag") not in source.get("covers", []):
            self.fail(path, f"{record['id']} tag {record.get('tag')!r} missing from {source_ref}.covers")
        if record.get("status") != "mapped":
            self.fail(path, f"{record['id']} with source_ref must use status = mapped")

    def validate_required_spec_gap(self, path: Path, record: dict[str, Any], gap_ref: str) -> None:
        gap = self.spec_gaps.get(gap_ref)
        if not gap:
            self.fail(path, f"{record['id']} references unknown spec_gap_ref {gap_ref}")
            return
        if gap.get("status") != "spec_gap":
            self.fail(path, f"{record['id']} spec_gap_ref {gap_ref} is not a spec_gap")
        if gap.get("resolution_status") not in {"open", "decision_recorded", "spec_updated", "test_mapped"}:
            self.fail(path, f"{record['id']} spec_gap_ref {gap_ref} is not open/actionable")
        if record.get("status") != "spec_gap":
            self.fail(path, f"{record['id']} with spec_gap_ref must use status = spec_gap")

    def validate_matrix(self) -> None:
        path = VERIFICATION / "matrix.toml"
        if not self.matrix:
            self.fail(path, "planning matrix is missing or empty")
            return
        self.require(
            path,
            self.matrix,
            ["schema_version", "id", "title", "status", "owner", "last_reviewed", "areas", "intent_requirements"],
            "planning matrix",
        )
        self.check_schema_version(path, self.matrix, "planning matrix")
        if self.matrix.get("status") not in STATUSES:
            self.fail(path, f"planning matrix uses unknown status {self.matrix.get('status')!r}")

        areas = self.matrix.get("areas")
        if not isinstance(areas, list) or not areas:
            self.fail(path, "planning matrix must define at least one [[areas]] row")
        else:
            seen_areas: set[str] = set()
            for area in areas:
                self.validate_matrix_area(path, area, seen_areas)

        intents = self.matrix.get("intent_requirements")
        if not isinstance(intents, list) or not intents:
            self.fail(path, "planning matrix must define [[intent_requirements]]")
        else:
            seen_intents: set[str] = set()
            for intent in intents:
                self.validate_matrix_intent(path, intent, seen_intents)
            missing = INTENTS - seen_intents
            if missing:
                self.fail(path, f"planning matrix missing intent rows {sorted(missing)}")

    def validate_matrix_area(self, path: Path, area: dict[str, Any], seen_areas: set[str]) -> None:
        area_id = area.get("id")
        if area_id in seen_areas:
            self.fail(path, f"planning matrix duplicates area {area_id}")
        seen_areas.add(area_id)
        if area_id not in AREAS:
            self.fail(path, f"planning matrix has unknown area {area_id!r}")
        if area.get("status") not in STATUSES:
            self.fail(path, f"planning matrix area {area_id} uses unknown status {area.get('status')!r}")
        if area.get("risk_default") not in RISKS:
            self.fail(path, f"planning matrix area {area_id} has unknown risk_default {area.get('risk_default')!r}")
        for risk in area.get("high_risks", []):
            if risk not in HIGH_RISKS:
                self.fail(path, f"planning matrix area {area_id} has non-high risk in high_risks: {risk}")
        if not area.get("path_globs"):
            self.fail(path, f"planning matrix area {area_id} must define path_globs")
        for test_class in area.get("required_test_classes", []):
            if test_class not in TEST_CLASSES:
                self.fail(path, f"planning matrix area {area_id} has unknown test_class {test_class}")
        for family in area.get("required_case_families", []):
            if family not in CASE_FAMILIES:
                self.fail(path, f"planning matrix area {area_id} has unknown case family {family}")

    def validate_matrix_intent(self, path: Path, intent: dict[str, Any], seen_intents: set[str]) -> None:
        name = intent.get("intent")
        if name in seen_intents:
            self.fail(path, f"planning matrix duplicates intent {name}")
        seen_intents.add(name)
        if name not in INTENTS:
            self.fail(path, f"planning matrix has unknown intent {name!r}")
        for test_class in intent.get("required_test_classes", []):
            if test_class not in TEST_CLASSES:
                self.fail(path, f"planning matrix intent {name} has unknown test_class {test_class}")
        for field in ("red_required", "lock_required"):
            if not isinstance(intent.get(field), bool):
                self.fail(path, f"planning matrix intent {name} must set boolean {field}")

    def validate_evidence(self) -> None:
        required = [
            "schema_version",
            "id",
            "title",
            "area",
            "owner",
            "status",
            "kind",
            "commit",
            "platform",
            "date",
            "producer",
            "generated_report_version",
            "linked_invariants",
            "linked_tests",
            "last_reviewed",
        ]
        approved_producers = self.approved_producers()
        for record in self.evidence.values():
            path = record["_path"]
            self.require(path, record, required, "evidence")
            self.check_common(path, record)
            if record.get("kind") not in EVIDENCE_KINDS:
                self.fail(path, f"{record['id']} has unknown evidence kind {record.get('kind')!r}")
            if not COMMIT_RE.match(str(record.get("commit", ""))):
                self.fail(path, f"{record['id']} has invalid commit marker {record.get('commit')!r}")
            if record.get("proof_kind") and record["proof_kind"] not in PROOF_KINDS:
                self.fail(path, f"{record['id']} has unknown proof_kind {record['proof_kind']!r}")
            if not record.get("suite_id") and not record.get("release_object"):
                self.fail(path, f"{record['id']} must name suite_id or release_object")
            self.check_refs(path, record.get("linked_invariants", []), self.invariants, "invariant", record["id"])
            self.check_refs(path, record.get("linked_tests", []), self.tests, "test", record["id"])
            if record.get("suite_id") and record["suite_id"] not in self.suites:
                self.fail(path, f"{record['id']} references unknown suite_id {record['suite_id']}")
            if record.get("kind") == "committed_file":
                evidence_path = record.get("path")
                if not evidence_path:
                    self.fail(path, f"{record['id']} committed_file evidence missing path")
                else:
                    self.validate_durable_path(path, record["id"], evidence_path)
            elif record.get("kind") == "ci_artifact":
                for field in ("workflow", "run_id", "artifact", "retention_days"):
                    if field not in record:
                        self.fail(path, f"{record['id']} ci_artifact evidence missing {field}")
            elif record.get("kind") == "release_object":
                for field in ("release_object", "url"):
                    if field not in record:
                        self.fail(path, f"{record['id']} release_object evidence missing {field}")
            elif record.get("kind") == "lab_report":
                for field in ("path", "device_model", "firmware", "topology", "env_vars", "environment"):
                    if field not in record:
                        self.fail(path, f"{record['id']} lab_report evidence missing {field}")
            if record.get("proof_kind") in {"red", "green", "lock_compare"} and self.links_high_risk(record):
                producer = record.get("producer")
                if not (PROVE_PRODUCER_RE.match(str(producer)) or producer in approved_producers):
                    self.fail(path, f"{record['id']} high-risk red/green proof producer {producer!r} is not allowlisted")
            validate_green_pairing(
                fail=self.fail,
                path=path,
                record=record,
                evidence=self.evidence,
                tests=self.tests,
                approved_producers=approved_producers,
            )
            validate_lock_pairing(
                fail=self.fail,
                path=path,
                record=record,
                evidence=self.evidence,
                tests=self.tests,
                approved_producers=approved_producers,
            )

    def validate_durable_path(self, path: Path, evidence_id: str, value: str) -> None:
        evidence_path = ROOT / value
        if not evidence_path.exists():
            self.fail(path, f"{evidence_id} evidence path does not exist: {value}")
            return
        result = subprocess.run(
            ["git", "check-ignore", "-q", "--", value],
            cwd=ROOT,
            check=False,
        )
        if result.returncode == 0:
            self.fail(path, f"{evidence_id} evidence path is gitignored: {value}")
        elif result.returncode != 1:
            self.fail(path, f"{evidence_id} git check-ignore failed for {value} with exit {result.returncode}")

    def validate_tests(self) -> None:
        required = [
            "schema_version",
            "id",
            "test_class",
            "area",
            "path",
            "command",
            "owner",
            "status",
            "invariants",
            "expected_result",
            "suite_tiers",
            "requires_hardware",
            "requires_network",
            "duration_class",
        ]
        for record in self.tests.values():
            path = record["_path"]
            self.require(path, record, required, "test")
            self.check_common(path, record)
            if record.get("test_class") not in TEST_CLASSES:
                self.fail(path, f"{record['id']} has unknown test_class {record.get('test_class')!r}")
            validate_runnable_test_path(self.fail, path, record)
            if record.get("status") not in {"planned", "gap_open"}:
                if not record.get("invariants"):
                    self.fail(path, f"{record['id']} mapped test must name invariants")
                if "oracle_ref" not in record and "spec_gap_ref" not in record:
                    self.fail(path, f"{record['id']} mapped test must name oracle_ref or spec_gap_ref")
            self.check_refs(path, record.get("invariants", []), self.invariants, "invariant", record["id"])
            self.check_refs(path, record.get("suite_tiers", []), self.suites, "suite", record["id"])
            if "spec_gap_ref" in record and record["spec_gap_ref"] not in self.spec_gaps:
                self.fail(path, f"{record['id']} references unknown spec_gap_ref {record['spec_gap_ref']}")
            if "oracle_ref" in record:
                validate_oracle_ref(
                    fail=self.fail,
                    path=path,
                    owner_id=record["id"],
                    oracle_ref=record["oracle_ref"],
                    spec_sources=self.spec_sources,
                )
            self.validate_case_digest(path, record)

    def validate_case_digest(self, path: Path, record: dict[str, Any]) -> None:
        has_file = "case_file" in record
        has_digest = "case_file_digest" in record
        if has_file != has_digest:
            self.fail(path, f"{record['id']} must set case_file and case_file_digest together")
            return
        if not has_file:
            return
        case_path = ROOT / record["case_file"]
        if not case_path.exists():
            self.fail(path, f"{record['id']} case_file does not exist: {record['case_file']}")
            return
        actual = "sha256:" + hashlib.sha256(case_path.read_bytes()).hexdigest()
        if actual != record["case_file_digest"]:
            self.fail(path, f"{record['id']} case_file_digest mismatch: expected {record['case_file_digest']}, actual {actual}")
        validate_case_file(
            fail=self.fail,
            path=path,
            test_record=record,
            invariants=self.invariants,
            spec_sources=self.spec_sources,
            spec_gaps=self.spec_gaps,
        )

    def validate_ignored_tests(self) -> None:
        for record in self.ignored_tests.values():
            path = record["_path"]
            self.require(
                path,
                record,
                ["schema_version", "id", "test_id", "owner", "area", "status", "ignore_class", "reason", "unblock_condition", "last_reviewed"],
                "ignored test",
            )
            self.check_common(path, record)

    def validate_risks(self) -> None:
        for record in self.risks.values():
            path = record["_path"]
            self.require(
                path,
                record,
                ["schema_version", "id", "title", "area", "risk", "owner", "status", "last_reviewed", "description", "mitigation", "related_invariants"],
                "risk",
            )
            self.check_common(path, record)
            if record.get("risk") not in RISKS:
                self.fail(path, f"{record['id']} has unknown risk {record.get('risk')!r}")
            self.check_refs(path, record.get("related_invariants", []), self.invariants, "invariant", record["id"])
            self.check_refs(path, record.get("related_spec_gaps", []), self.spec_gaps, "spec gap", record["id"])
            self.check_refs(path, record.get("evidence_refs", []), self.evidence, "evidence", record["id"])

    def check_common(self, path: Path, record: dict[str, Any], allow_suite: bool = False) -> None:
        self.check_schema_version(path, record, "record")
        self.check_area(path, record, allow_suite=allow_suite)
        self.check_status(path, record)
        if "last_reviewed" not in record and "updated_at" not in record:
            self.fail(path, f"{record.get('id', '<unknown>')} missing last_reviewed or updated_at")

    def check_refs(
        self,
        path: Path,
        refs: list[str],
        target: dict[str, dict[str, Any]],
        kind: str,
        owner_id: str,
    ) -> None:
        if not isinstance(refs, list):
            self.fail(path, f"{owner_id} {kind} refs must be a list")
            return
        for ref in refs:
            if ref not in target:
                self.fail(path, f"{owner_id} references unknown {kind} {ref}")

    def approved_producers(self) -> set[str]:
        result: set[str] = set()
        for suite in self.suites.values():
            result.update(suite.get("approved_proof_producers", []))
        return result

    def links_high_risk(self, evidence: dict[str, Any]) -> bool:
        for invariant_id in evidence.get("linked_invariants", []):
            invariant = self.invariants.get(invariant_id)
            if invariant and invariant.get("risk") in HIGH_RISKS:
                return True
        return False

    def has_closing_high_risk_evidence(self, invariant: dict[str, Any]) -> bool:
        approved_producers = self.approved_producers()
        for evidence_id in invariant.get("evidence_refs", []):
            evidence = self.evidence.get(evidence_id)
            if not evidence:
                continue
            if invariant["id"] not in evidence.get("linked_invariants", []):
                continue
            if evidence.get("proof_kind") not in {"green", "lock_compare"}:
                continue
            producer = str(evidence.get("producer", ""))
            if PROVE_PRODUCER_RE.match(producer) or producer in approved_producers:
                return True
        return False

    def validate_public_claim_links(self) -> None:
        referenced_sources: set[str] = set()
        for gap in self.spec_gaps.values():
            referenced_sources.update(gap.get("candidate_spec_sources", []))
        for invariant in self.invariants.values():
            referenced_sources.update(invariant.get("spec", {}).get("source_refs", []))
        for required in self.required_specs.values():
            source_ref = required.get("source_ref")
            if source_ref:
                referenced_sources.add(source_ref)
        for source in self.spec_sources.values():
            if source.get("authority") == "public_claim" and source["id"] not in referenced_sources:
                self.fail(source["_path"], f"public claim {source['id']} has no invariant, required-spec, or spec-gap reference")

    def finish(self) -> int:
        if self.failures:
            print("verification metadata validation failed:", file=sys.stderr)
            for failure in self.failures:
                print(f"- {failure.path}: {failure.message}", file=sys.stderr)
            return 1
        total = (
            len(self.spec_sources)
            + len(self.spec_gaps)
            + len(self.evidence)
            + len(self.invariants)
            + len(self.suites)
            + len(self.tests)
            + len(self.ignored_tests)
            + len(self.risks)
            + len(self.required_specs)
            + (1 if self.matrix else 0)
        )
        print(f"verification metadata validated: {total} records")
        return 0


def main() -> int:
    validator = Validator()
    validator.load_records()
    validator.validate()
    return validator.finish()


if __name__ == "__main__":
    raise SystemExit(main())
