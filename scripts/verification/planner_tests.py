"""Focused tests for planner integration with code-area routing."""

from __future__ import annotations

import json
import tomllib
import unittest

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.constants import AREAS
from scripts.verification.planner import Planner, result_to_json, risk_changes_from_matrices


class PlannerAreaRoutingTests(unittest.TestCase):
    def setUp(self) -> None:
        matrix = tomllib.loads((ROOT / "verification/matrix.toml").read_text())
        planner = Planner.__new__(Planner)
        planner.matrix = matrix
        planner.areas = {area["id"]: area for area in matrix["areas"]}
        planner.intent_requirements = {
            row["intent"]: row for row in matrix["intent_requirements"]
        }
        planner.required_specs = [
            {"id": f"REQ_{area}", "area": area, "blocks": "test_mapping"}
            for area in AREAS
        ]
        planner.spec_gap_records = []
        planner.spec_sources = {}
        planner.tests = []
        self.planner = planner

    def test_changed_paths_report_union_of_direct_suites_without_expanding_includes(self) -> None:
        result = self.planner.plan(
            "bugfix",
            [
                "crates/trust-hir/src/lib.rs",
                "crates/trust-runtime/src/hmi/tests/widget.rs",
            ],
            None,
            None,
        )

        self.assertEqual(result.areas, ["compiler_iec", "control_security", "hmi_ui"])
        self.assertIn("hir_type_diagnostics", result.matched_route_ids)
        self.assertIn("hmi_runtime_web_ui", result.matched_route_ids)
        self.assertEqual(result.required_suites, ["pr"])
        self.assertNotIn("nightly", result.required_suites)
        self.assertEqual(result.conditional_suites, [])
        self.assertEqual(
            json.loads(result_to_json(result))["required_suites"],
            ["pr"],
        )

    def test_invalid_changed_path_is_default_denied_and_reported(self) -> None:
        result = self.planner.plan(
            "docs",
            ["../crates/trust-hir/src/lib.rs"],
            None,
            None,
        )

        self.assertEqual(result.exit_code, 4)
        self.assertEqual(result.unmapped_files, ["../crates/trust-hir/src/lib.rs"])
        self.assertTrue(any("invalid changed path" in note for note in result.risk_notes))

    def test_direct_area_uses_only_its_declared_direct_suite_tiers(self) -> None:
        result = self.planner.plan("docs", None, "release", None)

        self.assertEqual(result.required_suites, ["pr"])
        self.assertEqual(result.matched_route_ids, [])

    def test_refactor_intent_adds_behavior_lock_route(self) -> None:
        result = self.planner.plan(
            "refactor",
            ["crates/trust-hir/src/lib.rs"],
            None,
            None,
        )

        self.assertIn("refactor_only", result.matched_route_ids)
        self.assertEqual(result.required_test_classes, ["behavior_lock", "metadata_validation"])
        self.assertEqual(result.required_suites, ["pr"])

    def test_docs_intent_does_not_inherit_product_route_test_classes(self) -> None:
        result = self.planner.plan(
            "docs",
            ["editors/vscode/README.md"],
            None,
            None,
        )

        self.assertEqual(result.required_test_classes, ["metadata_validation"])
        self.assertIn("vscode_extension", result.matched_route_ids)
        self.assertEqual(result.required_suites, ["pr"])

    def test_conditional_suites_are_reported_but_not_promoted(self) -> None:
        result = self.planner.plan(
            "bugfix",
            ["crates/trust-runtime/src/bytecode/format.rs"],
            None,
            None,
        )

        self.assertEqual(result.required_suites, ["pr"])
        self.assertEqual(result.conditional_suites, ["nightly"])
        payload = json.loads(result_to_json(result))
        self.assertEqual(payload["required_suites"], ["pr"])
        self.assertEqual(payload["conditional_suites"], ["nightly"])

    def test_missing_test_classes_remain_attributed_to_their_owning_area(self) -> None:
        bytecode_classes = {
            "metadata_validation",
            "negative_malformed_input",
            "failing_regression",
            "iec_conformance",
            "mutation",
        }
        self.planner.tests = [
            {
                "id": f"TEST_{test_class.upper()}",
                "area": "bytecode_vm",
                "status": "mapped",
                "test_class": test_class,
            }
            for test_class in bytecode_classes
        ]

        result = self.planner.plan(
            "bugfix",
            [
                "crates/trust-runtime/src/bytecode/format.rs",
                "crates/trust-runtime/src/runtime/cycle.rs",
            ],
            None,
            None,
        )

        self.assertNotIn("bytecode_vm", result.missing_test_classes_by_area)
        self.assertIn("runtime_safety", result.missing_test_classes_by_area)
        payload = json.loads(result_to_json(result))
        self.assertNotIn("bytecode_vm", payload["missing_test_classes_by_area"])
        self.assertIn("runtime_safety", payload["missing_test_classes_by_area"])

    def test_risk_downgrade_requires_a_valid_reviewed_decision(self) -> None:
        baseline = {
            "bytecode_vm": {
                "risk_default": "wrong_result",
                "high_risks": ["wrong_result", "silent_corruption"],
            }
        }
        current = {
            "bytecode_vm": {
                "risk_default": "maintenance",
                "high_risks": [],
            }
        }
        valid = {
            "SPEC_DECISION": {
                "authority": "reviewed_decision",
                "source_status": "active",
                "oracle_eligible": True,
            }
        }

        missing = risk_changes_from_matrices({"bytecode_vm"}, current, baseline)
        invented_current = copy_matrix(current, decision_ref="SPEC_INVENTED")
        invented = risk_changes_from_matrices(
            {"bytecode_vm"},
            invented_current,
            baseline,
            spec_sources=valid,
        )
        valid_current = copy_matrix(current, decision_ref="SPEC_DECISION")
        accepted = risk_changes_from_matrices(
            {"bytecode_vm"},
            valid_current,
            baseline,
            spec_sources=valid,
        )

        self.assertTrue(any("requires decision_ref" in item for item in missing))
        self.assertTrue(any("is not an active" in item for item in invented))
        self.assertFalse(any("decision_ref" in item for item in accepted))

    def test_risk_downgrade_rejects_wrong_authority_and_inactive_decisions(self) -> None:
        baseline = {
            "bytecode_vm": {
                "risk_default": "wrong_result",
                "high_risks": ["wrong_result"],
            }
        }
        current = copy_matrix(
            {"bytecode_vm": {"risk_default": "maintenance", "high_risks": []}},
            decision_ref="SPEC_DECISION",
        )
        bad_sources = [
            {
                "SPEC_DECISION": {
                    "authority": "normative_product",
                    "source_status": "active",
                    "oracle_eligible": True,
                }
            },
            {
                "SPEC_DECISION": {
                    "authority": "reviewed_deviation",
                    "source_status": "stale",
                    "oracle_eligible": True,
                }
            },
            {
                "SPEC_DECISION": {
                    "authority": "reviewed_decision",
                    "source_status": "active",
                    "oracle_eligible": False,
                }
            },
        ]
        for sources in bad_sources:
            with self.subTest(sources=sources):
                changes = risk_changes_from_matrices(
                    {"bytecode_vm"},
                    current,
                    baseline,
                    spec_sources=sources,
                )
                self.assertTrue(any("is not an active" in item for item in changes))


def copy_matrix(
    matrix: dict[str, dict[str, object]], *, decision_ref: str
) -> dict[str, dict[str, object]]:
    return {
        area_id: {**row, "decision_ref": decision_ref}
        for area_id, row in matrix.items()
    }


if __name__ == "__main__":
    unittest.main()
