"""Report and at-rest validation tests for the Phase 2A refactor assessment."""

from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.test_catalog_common import input_digest
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance
from scripts.verification.test_refactor_cli import default_command, main
from scripts.verification.test_refactor_live import (
    REPORT_ENTRYPOINT_PATHS,
    LiveTestRefactorState,
)
from scripts.verification.test_refactor_report import (
    RefactorAssessmentProvenance,
    TestRefactorAssessmentReport,
    write_reports,
)
from scripts.verification.test_refactor_validation import (
    validate_markdown_binding,
    validate_report_files,
    validate_report_payload,
    validate_schema_contract,
)


class TestRefactorReportTests(unittest.TestCase):
    def test_replay_entrypoints_are_digest_bound(self) -> None:
        expected = {
            "scripts/check_test_catalog_staleness.py",
            "scripts/report_test_refactor_assessment.py",
            "scripts/validate_test_refactor_assessment_report.py",
            "scripts/validate_test_refactor_proposals.py",
        }
        self.assertEqual(REPORT_ENTRYPOINT_PATHS, expected)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for relative in sorted(expected):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(relative + "\n")
            before = input_digest(root, sorted(expected))
            (root / "scripts/report_test_refactor_assessment.py").write_text("tampered\n")
            self.assertNotEqual(input_digest(root, sorted(expected)), before)

    def test_report_json_and_markdown_are_canonical_and_exact(self) -> None:
        report = fixture_report()
        json_bytes = report.to_json().encode()
        markdown = report.to_markdown(json_digest=hashlib.sha256(json_bytes).hexdigest())

        self.assertEqual(json_bytes, (json.dumps(report.to_dict(), indent=2, sort_keys=True) + "\n").encode())
        self.assertEqual(
            validate_report_payload(report.to_dict(), expected_assessment=report.assessment),
            [],
        )
        self.assertEqual(validate_markdown_binding(report.to_dict(), json_bytes, markdown), [])
        self.assertIn("Size is a review signal, not a refactor decision.", markdown)
        self.assertIn("`CASE_A`", markdown)
        self.assertIn("`TEST_ARTIFACT`", markdown)
        self.assertLess(
            markdown.index("Structural peers"),
            markdown.index("## VS Code Registration"),
        )

        self.assertIn(
            "test-refactor assessment Markdown does not exactly match JSON",
            validate_markdown_binding(
                report.to_dict(),
                json_bytes,
                markdown + "\nContradictory appendix.\n",
            ),
        )

    def test_assessment_or_summary_tampering_fails(self) -> None:
        report = fixture_report()
        tampered = copy.deepcopy(report.to_dict())
        tampered["summary"]["scanner_facts"] += 1

        failures = validate_report_payload(tampered, expected_assessment=report.assessment)

        self.assertTrue(any("does not match current refactor assessment" in item for item in failures))

    def test_closed_schema_accepts_fixture_and_rejects_unknown_fields(self) -> None:
        payload = fixture_report().to_dict()
        schema = json.loads(
            (ROOT / "verification/schemas/test-refactor-assessment-report.schema.json").read_text()
        )

        self.assertEqual(validate_schema_contract(schema), [])
        self.assertEqual(validate_json_schema_instance(payload, schema), [])
        payload["unexpected"] = True
        self.assertIn(
            "$: additional property unexpected is forbidden",
            validate_json_schema_instance(payload, schema),
        )

    def test_schema_honesty_constants_patterns_and_enums_are_drift_pinned(self) -> None:
        schema = json.loads(
            (ROOT / "verification/schemas/test-refactor-assessment-report.schema.json").read_text()
        )
        corruptions = (
            (
                "const for generator",
                lambda item: item["properties"]["generator"].__setitem__("const", "other"),
            ),
            (
                "commit pattern",
                lambda item: item["properties"]["commit"].__setitem__("pattern", ".*"),
            ),
            (
                "proposal disposition enum",
                lambda item: item["$defs"]["proposal_evaluation"]["properties"][
                    "disposition"
                ].__setitem__("enum", ["no_refactor_needed"]),
            ),
        )
        for expected, mutate in corruptions:
            with self.subTest(expected=expected):
                tampered = copy.deepcopy(schema)
                mutate(tampered)
                self.assertTrue(
                    any(expected in failure for failure in validate_schema_contract(tampered))
                )

    def test_default_command_requires_timestamp(self) -> None:
        with self.assertRaisesRegex(ValueError, "timestamp is required"):
            default_command(Path("report.json"), Path("report.md"), "")

    def test_report_only_cli_returns_zero_when_candidates_exist(self) -> None:
        report = fixture_report()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with (
                patch(
                    "scripts.verification.test_refactor_cli.generate_report",
                    return_value=report,
                ),
                patch(
                    "scripts.verification.test_refactor_cli.validate_report_files",
                    return_value=[],
                ),
            ):
                result = main(
                    [
                        "--root",
                        str(root),
                        "--json-out",
                        str(root / "report.json"),
                        "--markdown-out",
                        str(root / "report.md"),
                        "--timestamp",
                        "2026-07-10T12:00:00Z",
                    ]
                )

        self.assertEqual(result, 0)

    def test_at_rest_validator_recomputes_assessment_and_binds_actual_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            catalog = root / "verification/test-catalog.toml"
            catalog.parent.mkdir(parents=True)
            catalog.write_text("tests = []\n")
            schema = root / "verification/schemas/test-refactor-assessment-report.schema.json"
            schema.parent.mkdir(parents=True)
            schema.write_text(
                (ROOT / "verification/schemas/test-refactor-assessment-report.schema.json").read_text()
            )
            report = fixture_report()
            report = TestRefactorAssessmentReport(
                provenance=report.provenance,
                input_digest=input_digest(root, report.provenance.input_paths),
                scope=report.scope,
                assessment=report.assessment,
                limitations=report.limitations,
            )
            json_path = root / report.provenance.output_json
            markdown_path = root / report.provenance.output_markdown
            write_reports(report, json_path=json_path, markdown_path=markdown_path)
            state = LiveTestRefactorState(
                assessment=report.assessment,
                scope=report.scope,
                limitations=report.limitations,
                input_paths=report.provenance.input_paths,
                commit=report.provenance.commit,
                timestamp=report.provenance.timestamp,
                platform=report.provenance.platform,
                catalog_count=2,
                fact_count=3,
                proposal_count=1,
                redirect_count=0,
            )
            with (
                patch(
                    "scripts.verification.test_refactor_validation.build_live_test_refactor_state",
                    return_value=state,
                ),
                patch(
                    "scripts.verification.test_refactor_validation._validate_source_commit",
                    return_value=[],
                ),
            ):
                self.assertEqual(
                    validate_report_files(root, json_path, markdown_path, schema),
                    [],
                )

                tampered_assessment = copy.deepcopy(report.assessment)
                tampered_assessment["summary"]["scanner_facts"] += 1
                tampered = TestRefactorAssessmentReport(
                    provenance=report.provenance,
                    input_digest=report.input_digest,
                    scope=report.scope,
                    assessment=tampered_assessment,
                    limitations=report.limitations,
                )
                write_reports(tampered, json_path=json_path, markdown_path=markdown_path)
                self.assertTrue(
                    any(
                        "does not match current refactor assessment" in failure
                        for failure in validate_report_files(root, json_path, markdown_path, schema)
                    )
                )

                write_reports(report, json_path=json_path, markdown_path=markdown_path)
                other_json = json_path.with_name("copied.json")
                other_json.write_bytes(json_path.read_bytes())
                self.assertTrue(
                    any(
                        "does not identify the validated JSON file" in failure
                        for failure in validate_report_files(
                            root,
                            other_json,
                            markdown_path,
                            schema,
                        )
                    )
                )


def fixture_report() -> TestRefactorAssessmentReport:
    assessment = {
        "summary": {
            "scanner_facts": 3,
            "fact_files": 2,
            "large_file_candidates": 1,
            "reviewed_mapping_diversity_candidates": 0,
            "broad_claim_candidates": 0,
            "exact_fact_file_duplicate_groups": 0,
            "whitespace_normalized_fact_file_duplicate_groups": 0,
            "exact_case_input_duplicate_groups": 0,
            "structural_case_input_peer_groups": 1,
            "shared_case_reference_groups": 0,
            "malformed_class_overlap_groups": 0,
            "vscode_facts": 1,
            "vscode_files": 1,
            "vscode_registrations": 1,
            "vscode_large_candidates": 0,
            "catalog_records": 2,
            "scanner_duration_classified": 1,
            "scanner_duration_unclassified": 2,
            "catalog_slow_records": 1,
            "proposals": 1,
            "supported_proposals": 1,
        },
        "file_assessment": [
            {
                "path": "crates/example/tests/large.rs",
                "source_kinds": ["rust_integration_test"],
                "packages": ["example"],
                "physical_lines": 1000,
                "scanner_fact_count": 2,
                "ignored_count": 0,
                "conditional_count": 0,
                "mapped_test_ids": [],
                "reviewed_areas": [],
                "reviewed_test_classes": [],
                "reviewed_invariant_ids": [],
                "unmapped_fact_count": 2,
                "candidate_reasons": ["large_file"],
            }
        ],
        "broad_claim_assessment": [],
        "duplicate_assessment": {
            "case_file_paths": ["verification/cases/example.toml"],
            "exact_fact_file_groups": [],
            "whitespace_normalized_fact_file_groups": [],
            "exact_case_input_groups": [],
            "structural_case_input_peer_groups": [
                {
                    "case_file": "verification/cases/example.toml",
                    "shape_digest": "sha256:" + "1" * 64,
                    "case_ids": ["CASE_A", "CASE_B"],
                }
            ],
            "shared_case_reference_groups": [],
            "malformed_class_overlap_groups": [],
            "source_body_similarity": "not_assessed",
        },
        "vscode_registration": {
            "index_path": "editors/vscode/src/test/suite/index.ts",
            "fact_count": 1,
            "test_file_count": 1,
            "registration_count": 1,
            "diagnostics": [],
            "registration_issues": {
                "duplicate_targets": [],
                "missing_targets": [],
                "unregistered_fact_files": [],
                "unregistered_files": [],
            },
            "files": [],
        },
        "duration_classification": {
            "scanner_facts": [
                {
                    "catalog_test_id": "TEST_SCANNER",
                    "classification_source": "hand_catalog",
                    "discovery_id": "DISC_00000000000000000000",
                    "duration_class": "fast",
                    "ignore_state": "not_ignored",
                    "name": "scanner_test",
                    "path": "crates/example/tests/large.rs",
                    "source_kind": "rust_integration_test",
                }
            ],
            "artifact_catalog_records": [
                {
                    "duration_class": "slow",
                    "path": "verification/cases/example.toml",
                    "subject_kind": "case_table_artifact",
                    "suite_tiers": ["nightly"],
                    "test_id": "TEST_ARTIFACT",
                }
            ],
            "commandless_suite_ids": [],
            "placeholder_suite_ids": [],
            "suite_tiers": [],
            "unassigned_tier_test_ids": [],
            "unknown_assigned_suite_ids": [],
        },
        "proposal_evaluations": [
            {
                "disposition": "no_refactor_needed",
                "observed_signals": [],
                "proposal_id": "TEST_REFACTOR_EXAMPLE_001",
                "source_paths": ["crates/example/tests/large.rs"],
                "supported": True,
            }
        ],
    }
    return TestRefactorAssessmentReport(
        provenance=RefactorAssessmentProvenance(
            command=(
                "python3",
                "scripts/report_test_refactor_assessment.py",
                "--json-out",
                "target/gate-artifacts/verification/test-refactor-assessment.json",
                "--markdown-out",
                "target/gate-artifacts/verification/test-refactor-assessment.md",
                "--timestamp",
                "2026-07-10T12:00:00Z",
            ),
            commit="0" * 40,
            timestamp="2026-07-10T12:00:00Z",
            platform="linux-test",
            input_paths=("verification/test-catalog.toml",),
            output_json="target/gate-artifacts/verification/test-refactor-assessment.json",
            output_markdown="target/gate-artifacts/verification/test-refactor-assessment.md",
        ),
        input_digest="sha256:" + "2" * 64,
        scope={
            "large_file_line_threshold": 1000,
            "large_threshold_source": "xtask/config/full_map_policy.json#kiss.existing_file_note_limit",
            "mixed_purpose_basis": "reviewed_catalog_mapping_diversity_only",
            "broad_claim_basis": "multiple_reviewed_invariants",
            "duplicate_basis": "exact_content_or_same_table_structural_shape_only",
            "duration_basis": "reviewed_catalog_duration_class_only",
            "debt_is_report_failure": False,
        },
        assessment=assessment,
        limitations=(
            "Size is a review signal, not a refactor decision.",
            "Unmapped facts remain unassessed for intent and duration.",
        ),
    )


if __name__ == "__main__":
    unittest.main()
