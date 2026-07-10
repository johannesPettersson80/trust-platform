"""Focused tests for planner integration with code-area routing."""

from __future__ import annotations

import json
import tomllib
import unittest

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.constants import AREAS
from scripts.verification.planner import Planner, result_to_json


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


if __name__ == "__main__":
    unittest.main()
