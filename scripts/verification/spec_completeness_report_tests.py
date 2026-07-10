"""Protective tests for the Phase 4A specification-completeness report."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from .spec_completeness_contract import (
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)
from .spec_completeness_report import (
    PILOT_CLASSIFICATIONS,
    SpecCompletenessProvenance,
    SpecCompletenessReport,
    analyze_spec_completeness,
)
from .spec_completeness_live import validate_input_binding, validate_source_binding
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance


def invariant(
    invariant_id: str,
    *,
    area: str = "bytecode_vm",
    risk: str = "wrong_result",
    spec_status: str = "missing",
    cell_state: str = "spec_gap",
    gap_ref: str | None = "SPEC_GAP_A",
) -> dict:
    cell = {
        "dimension": "wrong_type_or_shape",
        "state": cell_state,
        "rationale": "fixture",
    }
    if gap_ref is not None:
        cell["spec_gap_ref"] = gap_ref
    return {
        "id": invariant_id,
        "area": area,
        "risk": risk,
        "status": "spec_gap" if spec_status != "specified" else "gap_open",
        "spec": {"status": spec_status, "source_refs": ["SPEC_A"]},
        "coverage": {"cells": [cell]},
    }


def fixture_analysis() -> dict:
    return analyze_spec_completeness(
        invariants={
            "INV_A": invariant("INV_A"),
            "INV_B": invariant(
                "INV_B",
                area="runtime_safety",
                spec_status="specified",
                cell_state="gap_open",
                gap_ref=None,
            ),
        },
        tests={
            "TEST_ORACLE": {
                "id": "TEST_ORACLE",
                "area": "bytecode_vm",
                "test_class": "unit",
                "status": "mapped",
                "expected_result": "specified",
                "oracle_ref": "SPEC_A",
            },
            "TEST_MISSING": {
                "id": "TEST_MISSING",
                "area": "bytecode_vm",
                "test_class": "mutation",
                "status": "mapped",
                "expected_result": "lacks oracle binding",
            },
            "TEST_PLANNED": {
                "id": "TEST_PLANNED",
                "area": "bytecode_vm",
                "test_class": "metadata_validation",
                "status": "planned",
                "expected_result": "planned is not runnable",
                "spec_gap_ref": "SPEC_GAP_A",
            },
        },
        ignored_tests={},
        spec_gaps={
            "SPEC_GAP_A": {
                "id": "SPEC_GAP_A",
                "area": "bytecode_vm",
                "risk": "wrong_result",
                "resolution_status": "open",
                "blocking_question": "fixture?",
            }
        },
        spec_sources={
            "SPEC_A": {
                "id": "SPEC_A",
                "authority": "normative_product",
                "source_status": "active",
                "area": "bytecode_vm",
            },
            "PUBLIC_A": {
                "id": "PUBLIC_A",
                "authority": "public_claim",
                "source_status": "active",
                "area": "release",
                "surface_ref": "README.md",
            },
        },
        matrix={
            "areas": [
                {
                    "id": "bytecode_vm",
                    "status": "mapped",
                    "required_test_classes": [
                        "unit",
                        "mutation",
                        "metadata_validation",
                        "iec_conformance",
                    ],
                }
            ]
        },
    )


def fixture_report() -> SpecCompletenessReport:
    analysis = fixture_analysis()
    return SpecCompletenessReport(
        provenance=SpecCompletenessProvenance(
            command=(
                "python3",
                "scripts/report_spec_completeness.py",
                "--json-out",
                "target/gate-artifacts/verification/spec-completeness.json",
                "--markdown-out",
                "target/gate-artifacts/verification/spec-completeness.md",
                "--timestamp",
                "2026-07-10T12:00:00+00:00",
            ),
            commit="a" * 40,
            timestamp="2026-07-10T12:00:00+00:00",
            platform="test-platform",
            input_paths=("verification/spec-gaps.toml",),
            output_json="target/gate-artifacts/verification/spec-completeness.json",
            output_markdown="target/gate-artifacts/verification/spec-completeness.md",
        ),
        input_digest="sha256:" + "b" * 64,
        analysis=analysis,
    )


class SpecCompletenessAnalysisTests(unittest.TestCase):
    def test_reports_all_three_completeness_debt_surfaces(self) -> None:
        analysis = fixture_analysis()
        self.assertEqual(["INV_A"], [item["invariant_id"] for item in analysis["invariants_without_spec"]])
        self.assertEqual(["TEST_MISSING"], [item["test_id"] for item in analysis["tests_without_oracle"]])
        self.assertEqual(["INV_A"], [item["invariant_id"] for item in analysis["spec_gap_cells"]])

    def test_bytecode_pilot_denominator_is_exhaustive_and_disjoint(self) -> None:
        pilot = fixture_analysis()["bytecode_pilot"]
        ids = [item["gap_id"] for item in pilot["gaps"]]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(pilot["summary"]["total"], len(ids))
        self.assertEqual(
            set(ids),
            {
                "SPEC_GAP_A",
                "TEST_CLASS_GAP:bytecode_vm:iec_conformance",
                "TEST_CLASS_GAP:bytecode_vm:metadata_validation",
            },
        )
        self.assertEqual(1, pilot["summary"]["by_classification"]["spec_gap"])
        self.assertEqual(2, pilot["summary"]["by_classification"]["test_gap"])
        self.assertEqual(0, pilot["summary"]["by_classification"]["hardware_tool_blocked"])
        self.assertEqual(0, pilot["summary"]["by_classification"]["not_applicable"])

    def test_planned_catalog_row_does_not_fill_required_test_slot(self) -> None:
        gaps = fixture_analysis()["bytecode_pilot"]["gaps"]
        metadata = next(item for item in gaps if item["gap_id"].endswith("metadata_validation"))
        self.assertEqual("test_gap", metadata["classification"])
        self.assertEqual(["TEST_PLANNED"], metadata["related_record_ids"])

    def test_ignored_mapped_test_does_not_fill_required_test_slot(self) -> None:
        analysis = analyze_spec_completeness(
            invariants={},
            tests={
                "TEST_A": {
                    "id": "TEST_A",
                    "area": "bytecode_vm",
                    "test_class": "unit",
                    "status": "mapped",
                    "expected_result": "x",
                    "oracle_ref": "SPEC_A",
                }
            },
            ignored_tests={"IGNORED_A": {"id": "IGNORED_A", "test_id": "TEST_A"}},
            spec_gaps={},
            spec_sources={},
            matrix={"areas": [{"id": "bytecode_vm", "status": "mapped", "required_test_classes": ["unit"]}]},
        )
        self.assertEqual(1, analysis["bytecode_pilot"]["summary"]["total"])

    def test_public_claims_are_explicitly_non_exhaustive_context(self) -> None:
        context = fixture_analysis()["public_claim_context"]
        self.assertFalse(context["exhaustive"])
        self.assertEqual("registered_spec_sources_only", context["basis"])
        self.assertEqual(["PUBLIC_A"], [item["source_id"] for item in context["claims"]])


class SpecCompletenessContractTests(unittest.TestCase):
    def test_payload_and_markdown_round_trip(self) -> None:
        report = fixture_report()
        payload = report.to_dict()
        self.assertEqual([], validate_report_payload(payload, expected_analysis=report.analysis))
        json_bytes = report.to_json().encode()
        markdown = report.to_markdown(json_digest=hashlib.sha256(json_bytes).hexdigest())
        self.assertEqual([], validate_markdown_binding(payload, json_bytes, markdown))

    def test_tampered_pilot_partition_fails(self) -> None:
        payload = fixture_report().to_dict()
        payload["bytecode_pilot"]["gaps"][0]["classification"] = "test_gap"
        failures = validate_report_payload(payload)
        self.assertTrue(any("by_classification" in item for item in failures))

    def test_public_claim_context_cannot_claim_exhaustiveness(self) -> None:
        payload = fixture_report().to_dict()
        payload["public_claim_context"]["exhaustive"] = True
        failures = validate_report_payload(payload)
        self.assertTrue(any("non-exhaustive" in item for item in failures))

    def test_noncanonical_json_fails_markdown_binding(self) -> None:
        report = fixture_report()
        payload = report.to_dict()
        json_bytes = (json.dumps(payload) + "\n").encode()
        markdown = report.to_markdown(json_digest=hashlib.sha256(json_bytes).hexdigest())
        failures = validate_markdown_binding(payload, json_bytes, markdown)
        self.assertTrue(any("not canonical" in item for item in failures))

    def test_schema_contract_matches_payload_vocabulary(self) -> None:
        schema = json.loads(
            Path("verification/schemas/spec-completeness-report.schema.json").read_text()
        )
        self.assertEqual([], validate_schema_contract(schema))
        self.assertEqual([], validate_json_schema_instance(fixture_report().to_dict(), schema))
        self.assertEqual(
            set(PILOT_CLASSIFICATIONS),
            set(schema["$defs"]["pilot_gap"]["properties"]["classification"]["enum"]),
        )

    def test_expected_analysis_rejects_semantic_tamper(self) -> None:
        report = fixture_report()
        payload = copy.deepcopy(report.to_dict())
        payload["invariants_without_spec"] = []
        failures = validate_report_payload(payload, expected_analysis=report.analysis)
        self.assertTrue(any("current specification-completeness analysis" in item for item in failures))

    def test_input_and_source_binding_reject_content_and_commit_tampering(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Verification Test"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.invalid"], check=True)
            relative = "verification/input.toml"
            tracked = root / relative
            tracked.parent.mkdir(parents=True)
            tracked.write_text("value = 1\n")
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-q", "-m", "fixture"], check=True)
            commit = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            payload = {
                "input_paths": [relative],
                "input_digest": input_digest(root, [relative]),
            }
            self.assertEqual([], validate_input_binding(root, payload, [relative]))
            self.assertEqual([], validate_source_binding(root, commit, [relative]))

            tracked.write_text("value = 2\n")
            self.assertTrue(validate_input_binding(root, payload, [relative]))
            self.assertTrue(validate_source_binding(root, commit, [relative]))
            self.assertIn(
                "commit must identify a clean full Git SHA for at-rest validation",
                validate_source_binding(root, f"dirty:{commit}", [relative]),
            )

            untracked = root / "verification/new.py"
            untracked.write_text("# fixture\n")
            self.assertTrue(
                any(
                    item.startswith("source commit lacks report inputs:")
                    for item in validate_source_binding(root, commit, [relative, "verification/new.py"])
                )
            )


if __name__ == "__main__":
    unittest.main()
