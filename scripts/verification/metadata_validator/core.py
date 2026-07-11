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
    EVIDENCE_KINDS,
    GAP_CLASSES,
    HIGH_RISKS,
    INTENTS,
    PROOF_KINDS,
    PROVE_PRODUCER_RE,
    RESOLUTION_STATUSES,
    RISKS,
    ROOT,
    SCHEMA_FILES,
    SCHEMA_REQUIRED_FIELDS,
    SOURCE_AUTHORITIES,
    SOURCE_STATUSES,
    STATUSES,
    TEST_CLASSES,
    VERIFICATION,
)
from ..area_routing import MILESTONE_SUITE_IDS, validate_area_routing
from ..gate_inventory import (
    INVENTORY_PATH,
    GateInventoryError,
    load_gate_inventory,
    validate_gate_inventory,
)
from .case_files import validate_case_file
from ..invariant_seed_contract import load_seed_audit, validate_seed_records
from ..test_catalog_intent import validate_catalog_intent
from .evidence_proof import validate_green_pairing, validate_lock_pairing
from .integrity import (
    test_counts_as_runnable,
    validate_open_spec_gap_references,
    validate_runnable_test_path,
)
from .ignored_tests import load_checklist_row_ids, validate_ignored_test_records
from .invariants import validate_invariants as validate_invariant_records
from .mutation_shards import validate_committed_mutation_metadata
from .oracle_refs import validate_oracle_ref
from .public_claims import validate_public_claim_records
from .schema_contracts import validate_schema_enums
from .risks import validate_risks as validate_risk_records
from .taxonomy import validate_taxonomy_drift
from .spec_gap_closure import validate_spec_gap_closure
from .suites import validate_suite_records


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
        self.gate_inventory: dict[str, dict[str, Any]] = {}
        self.tests: dict[str, dict[str, Any]] = {}
        self.ignored_tests: dict[str, dict[str, Any]] = {}
        self.risks: dict[str, dict[str, Any]] = {}
        self.required_specs: dict[str, dict[str, Any]] = {}
        self.matrix: dict[str, Any] = {}
        self.seed_manifest: dict[str, Any] = {}

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

    def check_schema_version(
        self,
        path: Path,
        record: dict[str, Any],
        kind: str,
        *,
        expected: int = 1,
    ) -> None:
        if record.get("schema_version") != expected:
            self.fail(
                path,
                f"{kind} {record.get('id', '<unknown>')} must use schema_version = {expected}",
            )

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
            for failure in validate_schema_enums(name, data):
                self.fail(path, failure)

    def load_records(self) -> None:
        self.load_json_schemas()
        try:
            self.gate_inventory = load_gate_inventory(ROOT)
        except GateInventoryError as exc:
            self.fail(ROOT / INVENTORY_PATH, str(exc))
        else:
            validate_gate_inventory(
                ROOT,
                self.gate_inventory,
                on_failure=self.fail,
            )
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
        self.seed_manifest = self.load_toml(VERIFICATION / "invariant-seeds.toml")
        self.check_no_empty_strings(
            VERIFICATION / "invariant-seeds.toml", self.seed_manifest
        )
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
        self.validate_invariant_seeds()
        for failure in validate_spec_gap_closure(
            root=ROOT,
            spec_gaps=self.spec_gaps,
            spec_sources=self.spec_sources,
            tests=self.tests,
            evidence=self.evidence,
            invariants=self.invariants,
            required_specs=self.required_specs,
            risks=self.risks,
        ):
            self.fail(VERIFICATION / "spec-gaps.toml", failure)
        validate_committed_mutation_metadata(
            fail=self.fail,
            tests=self.tests,
            evidence=self.evidence,
        )
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
            "oracle_eligible",
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
            if not isinstance(record.get("oracle_eligible"), bool):
                self.fail(path, f"{record['id']} oracle_eligible must be boolean")
            if record.get("authority") == "public_claim" and record.get("oracle_eligible") is not False:
                self.fail(path, f"public claim {record['id']} must set oracle_eligible = false")
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
            "affected_tests",
            "candidate_spec_sources",
            "resolution_status",
            "closeout_evidence",
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
            self.check_refs(
                path,
                record.get("affected_tests", []),
                self.tests,
                "test",
                record["id"],
            )

    def validate_suites(self) -> None:
        validate_suite_records(
            fail=self.fail,
            suites=self.suites,
            inventory=self.gate_inventory,
        )

    def validate_invariants(self) -> None:
        validate_invariant_records(
            fail=self.fail,
            require=self.require,
            check_common=self.check_common,
            check_refs=self.check_refs,
            invariants=self.invariants,
            spec_sources=self.spec_sources,
            spec_gaps=self.spec_gaps,
            tests=self.tests,
            suites=self.suites,
            evidence=self.evidence,
            approved_producers=self.approved_producers(),
        )

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
                if (
                    not source
                    or source.get("authority") not in {"reviewed_decision", "reviewed_deviation"}
                    or source.get("source_status") != "active"
                    or source.get("oracle_eligible") is not True
                ):
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
        if source.get("oracle_eligible") is not True:
            self.fail(
                path,
                f"{record['id']} source_ref {source_ref} is provenance-only and cannot satisfy a required spec",
            )
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
            [
                "schema_version",
                "id",
                "title",
                "status",
                "owner",
                "last_reviewed",
                "areas",
                "code_areas",
                "intent_requirements",
            ],
            "planning matrix",
        )
        self.check_schema_version(path, self.matrix, "planning matrix", expected=2)
        taxonomy = (
            ROOT
            / "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md"
        )
        try:
            taxonomy_text = taxonomy.read_text()
        except (OSError, UnicodeError) as exc:
            self.fail(taxonomy, f"test taxonomy cannot be read: {exc}")
        else:
            for failure in validate_area_routing(
                self.matrix,
                taxonomy_text,
                canonical_areas=AREAS,
                suite_ids=set(self.suites) & MILESTONE_SUITE_IDS,
            ):
                self.fail(path, failure)
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
        decision_ref = area.get("decision_ref")
        if decision_ref is not None:
            source = self.spec_sources.get(decision_ref)
            if (
                not source
                or source.get("authority")
                not in {"reviewed_decision", "reviewed_deviation"}
                or source.get("source_status") != "active"
                or source.get("oracle_eligible") is not True
            ):
                self.fail(
                    path,
                    f"planning matrix area {area_id} decision_ref must name an "
                    "active oracle-eligible reviewed decision/deviation",
                )

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
            self.check_refs(
                path,
                record.get("linked_spec_gaps", []),
                self.spec_gaps,
                "spec gap",
                record["id"],
            )
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
            "subject_kind",
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
            "expected_failure_mode",
            "evidence_destination",
            "last_reviewed",
        ]
        for record in self.tests.values():
            path = record["_path"]
            self.require(path, record, required, "test")
            self.check_common(path, record, schema_version=2)
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
        for failure in validate_catalog_intent(
            tests=self.tests,
            matrix=self.matrix,
            invariants=self.invariants,
            spec_sources=self.spec_sources,
            spec_gaps=self.spec_gaps,
        ):
            self.fail(VERIFICATION / "test-catalog.toml", failure)

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
        path = VERIFICATION / "ignored-tests.toml"
        for failure in validate_ignored_test_records(
            root=ROOT,
            ignored_tests=self.ignored_tests,
            tests=self.tests,
            checklist_row_ids=load_checklist_row_ids(ROOT),
        ):
            self.fail(path, failure)

    def validate_risks(self) -> None:
        validate_risk_records(
            fail=self.fail,
            require=self.require,
            check_common=self.check_common,
            check_refs=self.check_refs,
            risks=self.risks,
            invariants=self.invariants,
            spec_gaps=self.spec_gaps,
            spec_sources=self.spec_sources,
            evidence=self.evidence,
        )

    def validate_invariant_seeds(self) -> None:
        path = VERIFICATION / "invariant-seeds.toml"
        try:
            load_seed_audit(ROOT)
        except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as exc:
            self.fail(path, f"invariant seed audit failed: {exc}")
        records = self.seed_manifest.get("seeds")
        if self.seed_manifest.get("schema_version") != 1 or not isinstance(records, list):
            self.fail(path, "invariant seed manifest must use schema_version 1 and [[seeds]]")
            return
        invariant_paths = {
            invariant_id: record["_path"].relative_to(ROOT).as_posix()
            for invariant_id, record in self.invariants.items()
        }
        for failure in validate_seed_records(
            written_seed_text=(
                ROOT
                / "docs/internal/testing/checklists/plc-verification-program/verification-areas.md"
            ).read_text(),
            seed_records=records,
            invariants=self.invariants,
            invariant_paths=invariant_paths,
            spec_sources=self.spec_sources,
            spec_gaps=self.spec_gaps,
            risks=self.risks,
            tests=self.tests,
            evidence=self.evidence,
        ):
            self.fail(path, failure)

    def check_common(
        self,
        path: Path,
        record: dict[str, Any],
        allow_suite: bool = False,
        *,
        schema_version: int = 1,
    ) -> None:
        self.check_schema_version(path, record, "record", expected=schema_version)
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

    def validate_public_claim_links(self) -> None:
        validate_public_claim_records(
            fail=self.fail,
            spec_sources=self.spec_sources,
            spec_gaps=self.spec_gaps,
            invariants=self.invariants,
            required_specs=self.required_specs,
            evidence=self.evidence,
        )

    def finish(self) -> int:
        if self.failures:
            print("verification metadata validation failed:", file=sys.stderr)
            for failure in self.failures:
                print(f"- {failure.path}: {failure.message}", file=sys.stderr)
            return 1
        seed_records = self.seed_manifest.get("seeds", [])
        seed_count = len(seed_records) if isinstance(seed_records, list) else 0
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
            + seed_count
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
