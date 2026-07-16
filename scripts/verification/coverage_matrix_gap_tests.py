"""Tests for report-only coverage-matrix gap analysis."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.core import Validator
from scripts.verification.test_catalog_common import input_digest
from scripts.verification.coverage_matrix_gaps import (
    COVERAGE_STATES,
    TOOL_INPUT_PATHS,
    CoverageMatrixGapProvenance,
    CoverageMatrixGapReport,
    analyze_coverage_matrix_gaps,
    load_repository_inputs,
)
from scripts.verification.coverage_matrix_gap_validation import (
    validate_input_binding,
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
    validate_source_binding,
)
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


class CoverageMatrixGapTests(unittest.TestCase):
    def test_missing_slots_and_extra_cells_remain_separate_from_declared_states(self) -> None:
        analysis = analyze_coverage_matrix_gaps(**fixture_inputs())

        self.assertEqual(
            analysis["summary"],
            {
                "mapped_areas": 1,
                "mapped_area_invariants": 1,
                "out_of_scope_invariants": 1,
                "required_family_slots": 2,
                "assigned_required_slots": 1,
                "missing_required_slots": 1,
                "additional_recorded_cells": 1,
                "recorded_cells": 2,
                "case_files": 1,
                "case_observations": 2,
                "blocked_case_observations": 2,
                "state_counts": empty_state_counts(spec_gap=2),
            },
        )
        invariant = analysis["areas"][0]["invariants"][0]
        slots = {item["dimension"]: item for item in invariant["required_slots"]}
        self.assertEqual(slots["happy_path"]["assignment"], "assigned")
        self.assertEqual(slots["happy_path"]["coverage_state"], "spec_gap")
        self.assertEqual(slots["happy_path"]["blocked_case_ids"], ["CASE_HAPPY"])
        self.assertEqual(slots["boundary_low"]["assignment"], "missing_cell")
        self.assertIsNone(slots["boundary_low"]["coverage_state"])
        self.assertEqual(slots["boundary_low"]["blocked_case_ids"], ["CASE_BOUNDARY"])
        self.assertEqual(
            [item["dimension"] for item in invariant["additional_cells"]],
            ["resource_limit"],
        )
        self.assertEqual(
            analysis["out_of_scope_invariants"][0]["recorded_cells"][0]["coverage_state"],
            "spec_gap",
        )

    def test_all_seven_declared_states_are_preserved_without_quality_scoring(self) -> None:
        dimensions = [
            "happy_path",
            "boundary_low",
            "boundary_high",
            "below_min",
            "above_max",
            "wrong_type_or_shape",
            "missing_required",
        ]
        cells = []
        for dimension, state in zip(dimensions, COVERAGE_STATES, strict=True):
            cell = {
                "dimension": dimension,
                "state": state,
                "rationale": f"fixture {state}",
            }
            if state == "spec_gap":
                cell["spec_gap_ref"] = "SPEC_GAP_FIXTURE"
            if state == "not_applicable":
                cell["decision_ref"] = "SPEC_DECISION_FIXTURE"
            cells.append(cell)
        inputs = fixture_inputs()
        inputs["matrix"] = {
            "areas": [
                {
                    "id": "bytecode_vm",
                    "status": "mapped",
                    "required_case_families": dimensions,
                }
            ]
        }
        inputs["invariants"] = [fixture_invariant("INV_ALL", "bytecode_vm", cells)]
        inputs["case_tables"] = []

        analysis = analyze_coverage_matrix_gaps(**inputs)

        self.assertEqual(
            analysis["summary"]["state_counts"],
            {state: 1 for state in COVERAGE_STATES},
        )
        slots = analysis["areas"][0]["invariants"][0]["required_slots"]
        self.assertEqual(
            {item["dimension"]: item["coverage_state"] for item in slots},
            dict(zip(dimensions, COVERAGE_STATES, strict=True)),
        )
        self.assertNotIn("score", analysis["summary"])

    def test_duplicate_dimension_fails_closed(self) -> None:
        inputs = fixture_inputs()
        inputs["invariants"][0]["coverage"]["cells"].append(
            {
                "dimension": "happy_path",
                "state": "gap_open",
                "rationale": "duplicate fixture",
            }
        )

        with self.assertRaisesRegex(ValueError, "duplicates coverage dimension happy_path"):
            analyze_coverage_matrix_gaps(**inputs)

    def test_mapped_area_without_authorized_case_family_model_has_zero_slots(self) -> None:
        inputs = fixture_inputs()
        inputs["matrix"]["areas"].append(
            {
                "id": "compiler_iec",
                "status": "mapped",
                "required_case_families": [],
            }
        )
        inputs["invariants"].append(
            fixture_invariant(
                "INV_COMPILER",
                "compiler_iec",
                [
                    {
                        "dimension": "happy_path",
                        "state": "spec_gap",
                        "rationale": "recorded without inventing an area-wide family model",
                        "spec_gap_ref": "SPEC_GAP_FIXTURE",
                    }
                ],
            )
        )

        analysis = analyze_coverage_matrix_gaps(**inputs)

        area = next(row for row in analysis["areas"] if row["area"] == "compiler_iec")
        self.assertEqual(area["required_case_families"], [])
        self.assertEqual(area["required_family_slots"], 0)
        self.assertEqual(area["assigned_required_slots"], 0)
        self.assertEqual(area["missing_required_slots"], 0)
        self.assertEqual(
            area["invariants"][0]["additional_cells"][0]["dimension"],
            "happy_path",
        )

    def test_closed_spec_gap_is_reported_as_debt_without_relabeling_state(self) -> None:
        inputs = fixture_inputs()
        inputs["spec_gaps"]["SPEC_GAP_FIXTURE"]["resolution_status"] = "closed"

        analysis = analyze_coverage_matrix_gaps(**inputs)

        slot = analysis["areas"][0]["invariants"][0]["required_slots"][1]
        self.assertEqual(slot["dimension"], "happy_path")
        self.assertEqual(slot["coverage_state"], "spec_gap")
        self.assertEqual(slot["state_issues"], ["spec_gap_ref_not_open:SPEC_GAP_FIXTURE"])

    def test_analysis_is_deterministic_for_reordered_inputs(self) -> None:
        inputs = fixture_inputs()
        forward = analyze_coverage_matrix_gaps(**inputs)
        reverse = analyze_coverage_matrix_gaps(
            matrix={**inputs["matrix"], "areas": list(reversed(inputs["matrix"]["areas"]))},
            invariants=list(reversed(inputs["invariants"])),
            tests=list(reversed(inputs["tests"])),
            case_tables=list(reversed(inputs["case_tables"])),
            spec_gaps=dict(reversed(list(inputs["spec_gaps"].items()))),
        )

        self.assertEqual(forward, reverse)

    def test_closed_schema_and_semantic_validation_reject_tampering(self) -> None:
        report = fixture_report()
        payload = report.to_dict()
        schema = json.loads(
            (ROOT / "verification/schemas/coverage-matrix-gap-report.schema.json").read_text()
        )

        self.assertEqual(validate_schema_contract(schema), [])
        self.assertEqual(validate_json_schema_instance(payload, schema), [])
        self.assertEqual(validate_report_payload(payload, expected_analysis=report.analysis), [])

        tampered = copy.deepcopy(payload)
        tampered["summary"]["missing_required_slots"] = 0
        failures = validate_report_payload(tampered, expected_analysis=report.analysis)
        self.assertTrue(any("summary does not match current coverage analysis" in item for item in failures))
        tampered["unexpected"] = True
        self.assertIn(
            "$: additional property unexpected is forbidden",
            validate_json_schema_instance(tampered, schema),
        )
        dirty = copy.deepcopy(payload)
        dirty["commit"] = "dirty:" + "0" * 40
        self.assertIn(
            "commit must identify a clean full Git SHA",
            validate_report_payload(dirty, expected_analysis=report.analysis),
        )
        self.assertTrue(
            any(
                "$.commit" in failure
                for failure in validate_json_schema_instance(dirty, schema)
            )
        )

    def test_markdown_digest_binding_rejects_stale_summary(self) -> None:
        report = fixture_report()
        json_bytes = report.to_json().encode()
        markdown = report.to_markdown(json_digest=hashlib.sha256(json_bytes).hexdigest())
        stale = markdown.replace("- Missing required slots: 1", "- Missing required slots: 0")

        self.assertEqual(validate_markdown_binding(report.to_dict(), json_bytes, markdown), [])
        self.assertTrue(validate_markdown_binding(report.to_dict(), json_bytes, stale))
        self.assertTrue(
            validate_markdown_binding(
                report.to_dict(),
                json_bytes,
                markdown + "\nContradictory appendix.\n",
            )
        )
        compact_json = json.dumps(report.to_dict(), sort_keys=True).encode()
        compact_markdown = report.to_markdown(
            json_digest=hashlib.sha256(compact_json).hexdigest()
        )
        self.assertTrue(
            validate_markdown_binding(
                report.to_dict(),
                compact_json,
                compact_markdown,
            )
        )

    def test_command_and_timestamp_tampering_fail_closed(self) -> None:
        payload = fixture_report().to_dict()
        payload["command"][1] = "scripts/not-the-generator.py"
        self.assertIn(
            "command does not match canonical coverage-matrix gap invocation",
            validate_report_payload(payload),
        )

        payload = fixture_report().to_dict()
        payload["timestamp"] = "2026-07-10T16:00:00"
        payload["command"][-1] = payload["timestamp"]
        self.assertIn(
            "timestamp must be an ISO-8601 value with a timezone",
            validate_report_payload(payload),
        )

    def test_input_binding_detects_path_and_content_tampering(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = "verification/input.toml"
            (root / path).parent.mkdir(parents=True)
            (root / path).write_text("value = 1\n")
            payload = {
                "input_paths": [path],
                "input_digest": input_digest(root, [path]),
            }
            self.assertEqual(validate_input_binding(root, payload, [path]), [])

            (root / path).write_text("value = 2\n")
            self.assertIn(
                "input_digest does not match current report inputs",
                validate_input_binding(root, payload, [path]),
            )
            self.assertIn(
                "input_paths do not match current metadata, tool, case, and schema inputs",
                validate_input_binding(root, payload, [path, "verification/other.toml"]),
            )

    def test_source_binding_rejects_dirty_unknown_modified_and_untracked_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(
                ["git", "-C", str(root), "config", "user.name", "Verification Test"],
                check=True,
            )
            subprocess.run(
                ["git", "-C", str(root), "config", "user.email", "test@example.invalid"],
                check=True,
            )
            tracked = root / "verification/input.toml"
            tracked.parent.mkdir(parents=True)
            tracked.write_text("value = 1\n")
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            subprocess.run(
                ["git", "-C", str(root), "commit", "-q", "-m", "fixture"],
                check=True,
            )
            commit = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            self.assertEqual(
                validate_source_binding(root, commit, ["verification/input.toml"]),
                [],
            )

            tracked.write_text("value = 2\n")
            self.assertIn(
                "current report inputs differ from the clean source commit",
                validate_source_binding(root, commit, ["verification/input.toml"]),
            )
            untracked = root / "verification/new-tool.py"
            untracked.write_text("# fixture\n")
            self.assertTrue(
                any(
                    failure.startswith("source commit lacks report inputs:")
                    for failure in validate_source_binding(
                        root,
                        commit,
                        ["verification/input.toml", "verification/new-tool.py"],
                    )
                )
            )
            self.assertIn(
                "commit must identify a clean full Git SHA for at-rest validation",
                validate_source_binding(
                    root,
                    f"dirty:{commit}",
                    ["verification/input.toml"],
                ),
            )
            self.assertIn(
                "commit does not resolve in the repository: " + "f" * 40,
                validate_source_binding(root, "f" * 40, ["verification/input.toml"]),
            )

    def test_current_repository_baseline_counts_and_input_closure(self) -> None:
        validator = Validator()
        validator.load_records()
        validator.validate()
        self.assertEqual(
            [(failure.path.as_posix(), failure.message) for failure in validator.failures],
            [],
        )

        analysis, paths = load_repository_inputs(ROOT, validator)

        self.assertEqual(
            analysis["summary"],
            {
                "mapped_areas": 11,
                "mapped_area_invariants": 53,
                "out_of_scope_invariants": 0,
                "required_family_slots": 80,
                "assigned_required_slots": 17,
                "missing_required_slots": 63,
                "additional_recorded_cells": 51,
                "recorded_cells": 68,
                "case_files": 4,
                "case_observations": 31,
                "blocked_case_observations": 0,
                "state_counts": empty_state_counts(spec_gap=24, gap_open=23, covered=21),
            },
        )
        self.assertTrue(TOOL_INPUT_PATHS.issubset(paths))
        self.assertIn("verification/matrix.toml", paths)
        self.assertIn("verification/test-catalog.toml", paths)
        self.assertIn("verification/spec-gaps.toml", paths)
        self.assertIn("verification/spec-sources.toml", paths)
        self.assertEqual(
            len([path for path in paths if path.startswith("verification/cases/")]),
            4,
        )


def fixture_inputs() -> dict:
    return {
        "matrix": {
            "areas": [
                {
                    "id": "bytecode_vm",
                    "status": "mapped",
                    "required_case_families": ["happy_path", "boundary_low"],
                }
            ]
        },
        "invariants": [
            fixture_invariant(
                "INV_MAPPED",
                "bytecode_vm",
                [
                    {
                        "dimension": "happy_path",
                        "state": "spec_gap",
                        "rationale": "fixture gap",
                        "spec_gap_ref": "SPEC_GAP_FIXTURE",
                    },
                    {
                        "dimension": "resource_limit",
                        "state": "spec_gap",
                        "rationale": "fixture extra gap",
                        "spec_gap_ref": "SPEC_GAP_FIXTURE",
                    },
                ],
            ),
            fixture_invariant(
                "INV_OUT",
                "runtime_safety",
                [
                    {
                        "dimension": "hardware_or_network_fault",
                        "state": "spec_gap",
                        "rationale": "out-of-scope fixture",
                        "spec_gap_ref": "SPEC_GAP_FIXTURE",
                    }
                ],
            ),
        ],
        "tests": [
            {
                "id": "TEST_CASE_TABLE",
                "area": "bytecode_vm",
                "status": "planned",
                "subject_kind": "case_table_artifact",
                "test_class": "metadata_validation",
                "invariants": ["INV_MAPPED"],
                "case_file": "verification/cases/bytecode_vm/INV_MAPPED.toml",
            }
        ],
        "case_tables": [
            {
                "path": "verification/cases/bytecode_vm/INV_MAPPED.toml",
                "invariant": "INV_MAPPED",
                "cases": [
                    {
                        "id": "CASE_BOUNDARY",
                        "family": "boundary_low",
                        "state": "blocked",
                        "spec_gap_ref": "SPEC_GAP_FIXTURE",
                    },
                    {
                        "id": "CASE_HAPPY",
                        "family": "happy_path",
                        "state": "blocked",
                        "spec_gap_ref": "SPEC_GAP_FIXTURE",
                    },
                ],
            }
        ],
        "spec_gaps": {
            "SPEC_GAP_FIXTURE": {
                "id": "SPEC_GAP_FIXTURE",
                "resolution_status": "open",
            }
        },
    }


def fixture_invariant(invariant_id: str, area: str, cells: list[dict]) -> dict:
    return {
        "id": invariant_id,
        "area": area,
        "risk": "wrong_result",
        "status": "spec_gap",
        "contract_kind": "decision_table",
        "proof_level": "S0",
        "tests": ["TEST_CASE_TABLE"] if area == "bytecode_vm" else [],
        "coverage": {"cells": cells},
    }


def fixture_report() -> CoverageMatrixGapReport:
    analysis = analyze_coverage_matrix_gaps(**fixture_inputs())
    return CoverageMatrixGapReport(
        provenance=CoverageMatrixGapProvenance(
            command=(
                "python3",
                "scripts/report_coverage_matrix_gaps.py",
                "--json-out",
                "target/gate-artifacts/verification/coverage-matrix-gaps.json",
                "--markdown-out",
                "target/gate-artifacts/verification/coverage-matrix-gaps.md",
                "--timestamp",
                "2026-07-10T16:00:00Z",
            ),
            commit="0" * 40,
            timestamp="2026-07-10T16:00:00Z",
            platform="linux-test",
            input_paths=("verification/matrix.toml",),
            output_json="target/gate-artifacts/verification/coverage-matrix-gaps.json",
            output_markdown="target/gate-artifacts/verification/coverage-matrix-gaps.md",
        ),
        input_digest="sha256:" + "1" * 64,
        analysis=analysis,
    )


def empty_state_counts(**updates: int) -> dict[str, int]:
    counts = {state: 0 for state in COVERAGE_STATES}
    counts.update(updates)
    return counts


if __name__ == "__main__":
    unittest.main()
