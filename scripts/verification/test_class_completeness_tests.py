"""Tests for report-only test-class completeness analysis."""

from __future__ import annotations

import copy
import hashlib
import json
import unittest
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.test_catalog_common import make_fact
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance
from scripts.verification.test_class_completeness import (
    CompletenessProvenance,
    REPORT_CONTRACT_PATHS,
    TestClassCompletenessReport,
    analyze_test_class_completeness,
)
from scripts.verification.test_class_completeness_cli import generate_report
from scripts.verification.test_class_completeness_validation import (
    _validate_source_commit,
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)


class TestClassCompletenessTests(unittest.TestCase):
    def test_separates_scanner_mapping_from_required_class_completeness(self) -> None:
        facts, tests, matrix = fixture_inputs()

        analysis = analyze_test_class_completeness(matrix=matrix, tests=tests, facts=facts)

        self.assertEqual(
            analysis["summary"],
            {
                "scanner_facts": 2,
                "classified_scanner_facts": 1,
                "unmapped_scanner_facts": 1,
                "catalog_records": 3,
                "runnable_catalog_records": 2,
                "non_runnable_catalog_records": 1,
                "mapped_areas": 1,
                "required_class_slots": 3,
                "complete_class_slots": 2,
                "missing_class_slots": 1,
            },
        )
        self.assertEqual(
            analysis["scanner_classification"]["classified_mappings"],
            [{"discovery_id": facts[0].stable_id, "test_id": "TEST_MAGIC"}],
        )
        self.assertEqual(analysis["scanner_classification"]["unmapped_facts"], 1)

        classes = {item["test_class"]: item for item in analysis["areas"][0]["required_classes"]}
        self.assertTrue(classes["negative_malformed_input"]["complete"])
        self.assertTrue(classes["mutation"]["complete"])
        self.assertFalse(classes["metadata_validation"]["complete"])
        self.assertEqual(
            classes["metadata_validation"]["non_runnable_tests"],
            [
                {
                    "status": "planned",
                    "test_id": "TEST_CASE_TABLE",
                    "reason": "catalog_status:planned",
                }
            ],
        )

    def test_non_generated_subject_cannot_classify_a_scanner_fact(self) -> None:
        facts, tests, matrix = fixture_inputs()
        tests[2]["discovery_id"] = facts[1].stable_id

        analysis = analyze_test_class_completeness(matrix=matrix, tests=tests, facts=facts)

        self.assertEqual(analysis["summary"]["classified_scanner_facts"], 1)
        self.assertEqual(analysis["summary"]["unmapped_scanner_facts"], 1)

    def test_analysis_is_deterministic_for_reordered_inputs(self) -> None:
        facts, tests, matrix = fixture_inputs()

        forward = analyze_test_class_completeness(matrix=matrix, tests=tests, facts=facts)
        reverse = analyze_test_class_completeness(
            matrix={**matrix, "areas": list(reversed(matrix["areas"]))},
            tests=list(reversed(tests)),
            facts=list(reversed(facts)),
        )

        self.assertEqual(forward, reverse)

    def test_ignored_generated_fact_does_not_satisfy_a_required_class(self) -> None:
        facts, tests, matrix = fixture_inputs()
        facts[0] = replace(facts[0], ignore_state="ignored", ignore_reason="fixture")

        analysis = analyze_test_class_completeness(matrix=matrix, tests=tests, facts=facts)

        classes = {item["test_class"]: item for item in analysis["areas"][0]["required_classes"]}
        self.assertFalse(classes["negative_malformed_input"]["complete"])
        self.assertEqual(
            classes["negative_malformed_input"]["non_runnable_tests"],
            [
                {
                    "status": "mapped",
                    "test_id": "TEST_MAGIC",
                    "reason": "scanner_ignore_state:ignored",
                }
            ],
        )

    def test_report_renders_debt_without_turning_it_into_generation_failure(self) -> None:
        report = fixture_report()
        rendered_json = report.to_json()
        digest = hashlib.sha256(rendered_json.encode()).hexdigest()
        markdown = report.to_markdown(json_digest=digest)

        self.assertEqual(report.to_dict()["report_status"], "complete")
        self.assertIn("- Unmapped scanner facts: 1", markdown)
        self.assertIn("- Missing required class slots: 1", markdown)
        self.assertIn("`metadata_validation`", markdown)
        self.assertEqual(validate_report_payload(report.to_dict(), expected_analysis=report.analysis), [])
        self.assertEqual(
            validate_markdown_binding(report.to_dict(), rendered_json.encode(), markdown),
            [],
        )

    def test_semantic_and_digest_tampering_is_rejected_at_rest(self) -> None:
        report = fixture_report()
        original_json = report.to_json().encode()
        original_digest = hashlib.sha256(original_json).hexdigest()
        markdown = report.to_markdown(json_digest=original_digest)
        tampered = copy.deepcopy(report.to_dict())
        tampered["summary"]["unmapped_scanner_facts"] = 0
        tampered_json = (json.dumps(tampered, indent=2, sort_keys=True) + "\n").encode()

        semantic_failures = validate_report_payload(tampered, expected_analysis=report.analysis)
        binding_failures = validate_markdown_binding(tampered, tampered_json, markdown)

        self.assertTrue(any("summary does not match current completeness analysis" in item for item in semantic_failures))
        self.assertTrue(any("Generated JSON SHA-256" in item for item in binding_failures))

    def test_closed_schema_accepts_report_and_rejects_extra_field(self) -> None:
        report = fixture_report().to_dict()
        schema = json.loads(
            (ROOT / "verification/schemas/test-class-completeness-report.schema.json").read_text()
        )

        self.assertEqual(validate_schema_contract(schema), [])
        self.assertEqual(validate_json_schema_instance(report, schema), [])
        report["unexpected"] = True

        self.assertIn(
            "$: additional property unexpected is forbidden",
            validate_json_schema_instance(report, schema),
        )

    def test_schema_contract_rejects_runnable_status_enum_widening(self) -> None:
        schema = json.loads(
            (ROOT / "verification/schemas/test-class-completeness-report.schema.json").read_text()
        )
        schema["$defs"]["scope"]["properties"]["runnable_statuses"]["items"]["enum"].append(
            "planned"
        )

        failures = validate_schema_contract(schema)

        self.assertIn(
            "completeness schema enum for scope.runnable_statuses drifts from report contract",
            failures,
        )

    def test_forged_command_timestamp_and_unknown_commit_are_rejected(self) -> None:
        report = fixture_report().to_dict()
        report["command"] = ["false"]

        command_failures = validate_report_payload(report, expected_analysis=fixture_report().analysis)

        self.assertIn(
            "command does not match canonical completeness generator invocation",
            command_failures,
        )

        report = fixture_report().to_dict()
        report["timestamp"] = "not-an-iso-time"
        report["command"][-1] = "not-an-iso-time"
        timestamp_failures = validate_report_payload(
            report,
            expected_analysis=fixture_report().analysis,
        )
        self.assertIn("timestamp must be an ISO-8601 value with a timezone", timestamp_failures)
        self.assertIn(
            "commit does not resolve in the repository: " + "f" * 40,
            _validate_source_commit(ROOT, "f" * 40, ["verification/matrix.toml"]),
        )
        old_commit_failures = _validate_source_commit(
            ROOT,
            "0cdcad44a62879f2d8bddcaa009da45be8bb2491",
            list(REPORT_CONTRACT_PATHS),
        )
        self.assertTrue(
            any("source commit lacks report inputs" in item for item in old_commit_failures),
            old_commit_failures,
        )

    def test_generator_rejects_full_metadata_validation_failure_before_scanning(self) -> None:
        failure = SimpleNamespace(
            path=Path("verification/test-catalog.toml"),
            message="fixture metadata corruption",
        )
        validator = SimpleNamespace(
            failures=[failure],
            load_records=lambda: None,
            validate=lambda: None,
        )

        with patch(
            "scripts.verification.test_class_completeness_cli.Validator",
            return_value=validator,
        ):
            with self.assertRaisesRegex(ValueError, "fixture metadata corruption"):
                generate_report(
                    ROOT,
                    json_path=Path("target/gate-artifacts/verification/fixture.json"),
                    markdown_path=Path("target/gate-artifacts/verification/fixture.md"),
                    timestamp="2026-07-10T12:00:00Z",
                )


def fixture_inputs() -> tuple[list, list[dict], dict]:
    first = make_fact(
        source_kind="rust_integration_test",
        name="header_validation",
        path="crates/trust-runtime/tests/bytecode_container.rs",
        line=10,
        package="trust-runtime",
        command_hint="cargo test -p trust-runtime --test bytecode_container header_validation -- --exact",
        command_hint_authority="exact",
        discovery_confidence="exact_attribute",
    )
    second = make_fact(
        source_kind="rust_unit_test",
        name="unmapped_test",
        path="crates/trust-runtime/src/bytecode/tests.rs",
        line=20,
        package="trust-runtime",
        command_hint="cargo test -p trust-runtime unmapped_test -- --exact",
        command_hint_authority="exact",
        discovery_confidence="exact_attribute",
    )
    tests = [
        {
            "id": "TEST_MAGIC",
            "subject_kind": "generated_test",
            "discovery_id": first.stable_id,
            "area": "bytecode_vm",
            "test_class": "negative_malformed_input",
            "status": "mapped",
        },
        {
            "id": "TEST_MUTATION",
            "subject_kind": "mutation_shard_runner",
            "area": "bytecode_vm",
            "test_class": "mutation",
            "status": "mapped",
        },
        {
            "id": "TEST_CASE_TABLE",
            "subject_kind": "case_table_artifact",
            "area": "bytecode_vm",
            "test_class": "metadata_validation",
            "status": "planned",
        },
    ]
    matrix = {
        "areas": [
            {
                "id": "bytecode_vm",
                "status": "mapped",
                "required_test_classes": [
                    "metadata_validation",
                    "negative_malformed_input",
                    "mutation",
                ],
            }
        ]
    }
    return [first, second], tests, matrix


def fixture_report() -> TestClassCompletenessReport:
    facts, tests, matrix = fixture_inputs()
    analysis = analyze_test_class_completeness(matrix=matrix, tests=tests, facts=facts)
    return TestClassCompletenessReport(
        provenance=CompletenessProvenance(
            command=(
                "python3",
                "scripts/report_test_class_completeness.py",
                "--json-out",
                "target/gate-artifacts/verification/test-class-completeness.json",
                "--markdown-out",
                "target/gate-artifacts/verification/test-class-completeness.md",
                "--timestamp",
                "2026-07-10T12:00:00Z",
            ),
            commit="0" * 40,
            timestamp="2026-07-10T12:00:00Z",
            platform="linux-test",
            input_paths=("verification/matrix.toml", "verification/test-catalog.toml"),
            output_json="target/gate-artifacts/verification/test-class-completeness.json",
            output_markdown="target/gate-artifacts/verification/test-class-completeness.md",
        ),
        input_digest="sha256:" + "1" * 64,
        analysis=analysis,
    )


if __name__ == "__main__":
    unittest.main()
