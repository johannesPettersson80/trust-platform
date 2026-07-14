"""Tests for Phase 6 requirement/oracle mapping and missing-oracle reporting."""

from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.core import Validator
from scripts.verification.requirement_oracle_contract import (
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)
from scripts.verification.requirement_oracle_live import (
    BOARD_PATH,
    REQUIRED_OPEN_ROWS,
    build_live_requirement_oracle_state,
    validate_open_board_rows,
    validate_source_revision,
)
from scripts.verification.requirement_oracle_mapping import analyze_requirement_oracles
from scripts.verification.requirement_oracle_report import (
    RequirementOracleProvenance,
    RequirementOracleReport,
    render_markdown,
)
from scripts.verification.requirement_oracle_validation import validate_report_files
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


SCHEMA_PATH = ROOT / "verification/schemas/requirement-oracle-audit-report.schema.json"
GROUP_EXPECTATIONS = {
    "VERIF-P6-001": {
        "area_ids": ["compiler_iec"],
        "invariant_count": 5,
        "eligible_oracle_count": 3,
        "spec_gap_blocked_count": 2,
    },
    "VERIF-P6-002": {
        "area_ids": ["runtime_safety"],
        "invariant_count": 11,
        "eligible_oracle_count": 7,
        "spec_gap_blocked_count": 4,
    },
    "VERIF-P6-003": {
        "area_ids": ["protocols"],
        "invariant_count": 7,
        "eligible_oracle_count": 1,
        "spec_gap_blocked_count": 6,
    },
    "VERIF-P6-004": {
        "area_ids": ["editor_safety"],
        "invariant_count": 6,
        "eligible_oracle_count": 0,
        "spec_gap_blocked_count": 6,
    },
    "VERIF-P6-005": {
        "area_ids": ["control_security", "supply_chain_platform"],
        "invariant_count": 6,
        "eligible_oracle_count": 0,
        "spec_gap_blocked_count": 6,
    },
}


def loaded_validator() -> Validator:
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise AssertionError([failure.message for failure in validator.failures])
    return validator


def analysis_for(validator: Validator) -> dict:
    return analyze_requirement_oracles(
        invariants=validator.invariants,
        spec_sources=validator.spec_sources,
        spec_gaps=validator.spec_gaps,
    )


def test_report(analysis: dict) -> RequirementOracleReport:
    return RequirementOracleReport(
        provenance=RequirementOracleProvenance(
            command=(
                "python3",
                "scripts/report_requirement_oracle_audit.py",
                "--json-out",
                "target/gate-artifacts/verification/requirement-oracle-audit.json",
                "--markdown-out",
                "docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6-requirement-oracle-audit.md",
                "--timestamp",
                "2026-07-11T08:00:00Z",
            ),
            commit="a" * 40,
            timestamp="2026-07-11T08:00:00Z",
            platform="test-platform",
            input_paths=("verification/spec-sources.toml",),
            output_json="target/gate-artifacts/verification/requirement-oracle-audit.json",
            output_markdown=(
                "docs/internal/testing/evidence/plc-verification-program/"
                "2026-07-11/p6-requirement-oracle-audit.md"
            ),
        ),
        input_digest="sha256:" + "b" * 64,
        analysis=analysis,
    )


class RequirementOracleAnalysisTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.validator = loaded_validator()
        cls.analysis = analysis_for(cls.validator)

    def test_live_denominator_and_oracle_posture_are_exhaustive(self) -> None:
        self.assertEqual(
            self.analysis["summary"],
            {
                "invariants_total": 53,
                "mapped_phase6_invariants": 35,
                "other_area_invariants": 18,
                "eligible_oracles": 12,
                "missing_oracles": 41,
                "future_enforcement_candidates": 32,
            },
        )
        self.assertEqual(53, len(self.analysis["invariants"]))
        self.assertEqual(41, len(self.analysis["missing_oracles"]))
        self.assertEqual(
            {record["id"] for record in self.validator.invariants.values()},
            {row["invariant_id"] for row in self.analysis["invariants"]},
        )

    def test_phase6_mapping_groups_match_the_reviewed_area_denominators(self) -> None:
        groups = {row["board_row"]: row for row in self.analysis["mapping_groups"]}
        self.assertEqual(set(GROUP_EXPECTATIONS), set(groups))
        for board_row, expected in GROUP_EXPECTATIONS.items():
            with self.subTest(board_row=board_row):
                for field, value in expected.items():
                    self.assertEqual(value, groups[board_row][field])
                self.assertEqual(groups[board_row]["invariant_count"], len(groups[board_row]["invariant_ids"]))

    def test_gap_placeholder_is_not_upgraded_by_an_eligible_candidate_source(self) -> None:
        rows = {row["invariant_id"]: row for row in self.analysis["invariants"]}
        string_row = rows["IEC_STRING_001"]

        self.assertEqual(["SPEC_IEC_DATA_TYPES_CANDIDATE_001"], string_row["spec_source_refs"])
        self.assertTrue(
            self.validator.spec_sources["SPEC_IEC_DATA_TYPES_CANDIDATE_001"]["oracle_eligible"]
        )
        self.assertEqual("spec_gap_blocked", string_row["oracle_state"])
        self.assertEqual("SPEC_GAP_IEC_STRING_BINDING_BOUNDS_001", string_row["oracle_ref"])

    def test_public_claim_cannot_be_relabelled_as_an_oracle(self) -> None:
        validator = loaded_validator()
        invariant = copy.deepcopy(validator.invariants["PROTO_OPCUA_001"])
        invariant["oracle"] = {
            "kind": "public_doc",
            "ref": "PUBLIC_CLAIM_RUNTIME_WIRE_001",
        }
        validator.invariants[invariant["id"]] = invariant

        with self.assertRaisesRegex(ValueError, "public_claim.*cannot be an oracle"):
            analysis_for(validator)

    def test_unknown_inactive_and_provenance_only_oracles_fail_closed(self) -> None:
        for label in ("unknown", "inactive", "provenance_only"):
            with self.subTest(label=label):
                validator = loaded_validator()
                invariant = copy.deepcopy(validator.invariants["IEC_PREC_001"])
                if label == "unknown":
                    invariant["oracle"]["ref"] = "SPEC_UNKNOWN"
                elif label == "inactive":
                    source = copy.deepcopy(validator.spec_sources["SPEC_IEC_EXPRESSIONS_001"])
                    source["source_status"] = "stale"
                    validator.spec_sources[source["id"]] = source
                else:
                    source = copy.deepcopy(validator.spec_sources["SPEC_IEC_EXPRESSIONS_001"])
                    source["oracle_eligible"] = False
                    validator.spec_sources[source["id"]] = source
                validator.invariants[invariant["id"]] = invariant

                with self.assertRaises(ValueError):
                    analysis_for(validator)

    def test_gap_oracle_must_be_open_and_attached_to_the_invariant(self) -> None:
        for label in ("closed", "unattached"):
            with self.subTest(label=label):
                validator = loaded_validator()
                invariant = copy.deepcopy(validator.invariants["IEC_STRING_001"])
                gap_id = invariant["oracle"]["ref"]
                if label == "closed":
                    gap = copy.deepcopy(validator.spec_gaps[gap_id])
                    gap["resolution_status"] = "closed"
                    validator.spec_gaps[gap_id] = gap
                else:
                    invariant["spec_gap_refs"] = []
                    validator.invariants[invariant["id"]] = invariant

                with self.assertRaises(ValueError):
                    analysis_for(validator)

    def test_names_paths_and_unreferenced_sources_cannot_create_mappings(self) -> None:
        validator = loaded_validator()
        before = analysis_for(validator)
        source = copy.deepcopy(validator.spec_sources["SPEC_RUNTIME_ENGINE_001"])
        source["id"] = "SPEC_TITLE_AND_PATH_DECOY"
        source["title"] = "IEC string binding boundary contract"
        source["path"] = "docs/specs/02-data-types.md"
        validator.spec_sources[source["id"]] = source
        after = analysis_for(validator)

        before_rows = {row["invariant_id"]: row for row in before["invariants"]}
        after_rows = {row["invariant_id"]: row for row in after["invariants"]}
        self.assertEqual(before_rows["IEC_STRING_001"], after_rows["IEC_STRING_001"])

    def test_report_rows_preserve_status_proof_and_explicit_links(self) -> None:
        rows = {row["invariant_id"]: row for row in self.analysis["invariants"]}
        for invariant_id, invariant in self.validator.invariants.items():
            with self.subTest(invariant_id=invariant_id):
                row = rows[invariant_id]
                self.assertEqual(invariant["status"], row["invariant_status"])
                self.assertEqual(invariant["proof_level"], row["proof_level"])
                self.assertEqual(invariant["tests"], row["tests"])
                self.assertEqual(invariant["gates"], row["gates"])
                self.assertEqual(invariant["evidence_refs"], row["evidence_refs"])

    def test_blocked_enforcement_and_traceability_rows_must_remain_open(self) -> None:
        board = (ROOT / BOARD_PATH).read_text()
        self.assertEqual([], validate_open_board_rows(board))
        self.assertTrue(
            {
                "VERIF-P1A-003",
                "VERIF-P1A-006",
                "VERIF-P1B-012",
                "VERIF-P1B-014",
                "VERIF-P5-000B",
            }.issubset(REQUIRED_OPEN_ROWS)
        )

        for row_id in REQUIRED_OPEN_ROWS:
            with self.subTest(row_id=row_id):
                tampered = board.replace(f"- [ ] `{row_id}`", f"- [x] `{row_id}`", 1)
                self.assertIn(
                    f"{row_id} must remain open",
                    "\n".join(validate_open_board_rows(tampered)),
                )


class RequirementOracleReportContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.analysis = analysis_for(loaded_validator())
        cls.report = test_report(cls.analysis)

    def test_schema_and_payload_are_closed_and_in_sync(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text())
        payload = self.report.to_dict()

        self.assertEqual(
            ["python3", "scripts/report_requirement_oracle_audit.py"],
            payload["command"][:2],
        )
        self.assertEqual([], validate_schema_contract(schema))
        self.assertEqual([], validate_report_payload(payload, expected_analysis=self.analysis))
        self.assertEqual([], validate_json_schema_instance(payload, schema))

    def test_schema_honesty_enums_and_consts_are_drift_pinned(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text())
        enum_paths = (
            ("oracle state", ("invariant", "oracle_state"), "unknown"),
            ("group board row", ("mapping_group", "board_row"), "VERIF-P6-999"),
        )

        for label, (definition, field), extra_value in enum_paths:
            with self.subTest(label=label):
                tampered = copy.deepcopy(schema)
                tampered["$defs"][definition]["properties"][field]["enum"].append(
                    extra_value
                )
                self.assertTrue(validate_schema_contract(tampered))

        tampered = copy.deepcopy(schema)
        tampered["$defs"]["scope"]["properties"]["mapping_rows"]["items"][
            "enum"
        ].append("VERIF-P6-999")
        self.assertTrue(validate_schema_contract(tampered))

        tampered = copy.deepcopy(schema)
        tampered["$defs"]["boundaries"]["properties"]["audit_creates_proof"] = {
            "const": True
        }
        self.assertTrue(validate_schema_contract(tampered))

        tampered = copy.deepcopy(schema)
        tampered["$defs"] = []
        self.assertTrue(validate_schema_contract(tampered))

    def test_json_is_canonical_and_markdown_is_exactly_bound(self) -> None:
        json_bytes = self.report.to_json().encode()
        digest = hashlib.sha256(json_bytes).hexdigest()
        markdown = self.report.to_markdown(json_digest=digest)

        self.assertEqual(
            [],
            validate_markdown_binding(self.report.to_dict(), json_bytes, markdown),
        )
        self.assertTrue(
            validate_markdown_binding(
                self.report.to_dict(),
                json_bytes,
                markdown.replace("Missing Oracles", "Missing Oracle Debt", 1),
            )
        )

    def test_semantic_tamper_fails_even_when_summary_is_recomputed(self) -> None:
        payload = copy.deepcopy(self.report.to_dict())
        payload["invariants"][0]["oracle_state"] = "eligible_oracle"
        payload["summary"]["eligible_oracles"] += 1
        payload["summary"]["missing_oracles"] -= 1

        failures = validate_report_payload(payload, expected_analysis=self.analysis)

        self.assertTrue(any("current requirement/oracle analysis" in item for item in failures), failures)

    def test_at_rest_validator_rejects_canonical_semantic_corruption(self) -> None:
        state = build_live_requirement_oracle_state(
            ROOT,
            timestamp="2026-07-11T08:00:00Z",
        )
        report = RequirementOracleReport(
            provenance=RequirementOracleProvenance(
                command=(
                    "python3",
                    "scripts/report_requirement_oracle_audit.py",
                    "--json-out",
                    "target/gate-artifacts/verification/requirement-oracle-audit.json",
                    "--markdown-out",
                    "docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6-requirement-oracle-audit.md",
                    "--timestamp",
                    state.timestamp,
                ),
                commit=state.commit,
                timestamp=state.timestamp,
                platform=state.platform,
                input_paths=state.input_paths,
                output_json="target/gate-artifacts/verification/requirement-oracle-audit.json",
                output_markdown="docs/internal/testing/evidence/plc-verification-program/2026-07-11/p6-requirement-oracle-audit.md",
            ),
            input_digest=state.input_digest,
            analysis=state.analysis,
        )
        with tempfile.TemporaryDirectory() as temp:
            json_path = Path(temp) / "report.json"
            markdown_path = Path(temp) / "report.md"
            json_path.write_text(report.to_json())
            digest = hashlib.sha256(json_path.read_bytes()).hexdigest()
            markdown_path.write_text(report.to_markdown(json_digest=digest))
            with patch(
                "scripts.verification.requirement_oracle_validation.validate_source_revision",
                return_value=[],
            ):
                self.assertEqual(
                    [],
                    validate_report_files(
                        ROOT,
                        json_path,
                        markdown_path,
                        SCHEMA_PATH,
                        allow_external_test_outputs=True,
                    ),
                )

                payload = report.to_dict()
                payload["invariants"][0]["eligible_context_source_refs"] = [
                    "SPEC_RUNTIME_ENGINE_001"
                ]
                json_text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
                json_path.write_text(json_text)
                markdown_path.write_text(
                    render_markdown(
                        payload,
                        json_digest=hashlib.sha256(json_text.encode()).hexdigest(),
                    )
                )
                failures = validate_report_files(
                    ROOT,
                    json_path,
                    markdown_path,
                    SCHEMA_PATH,
                    allow_external_test_outputs=True,
                )
            self.assertTrue(
                any("current requirement/oracle analysis" in item for item in failures),
                failures,
            )

    def test_duplicate_identity_and_wrong_mapping_group_fail_closed(self) -> None:
        duplicate = copy.deepcopy(self.report.to_dict())
        duplicate["invariants"].insert(1, copy.deepcopy(duplicate["invariants"][0]))
        wrong_group = copy.deepcopy(self.report.to_dict())
        wrong_group["invariants"][0]["mapping_board_row"] = "VERIF-P6-001"

        duplicate_failures = validate_report_payload(duplicate)
        wrong_group_failures = validate_report_payload(wrong_group)

        self.assertTrue(
            any("unique canonical ID order" in item for item in duplicate_failures),
            duplicate_failures,
        )
        self.assertTrue(
            any("mapping_board_row must be" in item for item in wrong_group_failures),
            wrong_group_failures,
        )

    def test_source_revision_requires_a_clean_full_sha(self) -> None:
        self.assertIn(
            "commit must identify a clean full Git SHA",
            validate_source_revision(ROOT, "dirty:" + "a" * 40, ()),
        )
        self.assertIn(
            "commit must identify a clean full Git SHA",
            validate_source_revision(ROOT, "a" * 12, ()),
        )

    def test_live_state_binds_all_invariants_and_mapping_sources(self) -> None:
        state = build_live_requirement_oracle_state(ROOT, require_clean_commit=False)

        self.assertEqual(self.analysis, state.analysis)
        self.assertIn("verification/spec-sources.toml", state.input_paths)
        self.assertIn("verification/spec-gaps.toml", state.input_paths)
        self.assertTrue(
            all(
                str(record["_path"].relative_to(ROOT)).replace("\\", "/") in state.input_paths
                for record in loaded_validator().invariants.values()
            )
        )


if __name__ == "__main__":
    unittest.main()
