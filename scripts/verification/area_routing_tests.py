"""Tests for exhaustive code-area metadata and changed-file routing."""

from __future__ import annotations

import copy
import subprocess
import tomllib
import unittest
from pathlib import Path

from scripts.verification.area_routing import (
    AreaRoutingError,
    MILESTONE_SUITE_IDS,
    classify_changed_path,
    intent_overlay,
    normalize_changed_path,
    taxonomy_route_ids,
    validate_area_routing,
)
from scripts.verification.metadata_validator.constants import AREAS, ROOT
from scripts.verification.metadata_validator.core import Validator


MATRIX_PATH = ROOT / "verification/matrix.toml"
TAXONOMY_PATH = (
    ROOT / "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md"
)


class AreaRoutingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.matrix = tomllib.loads(MATRIX_PATH.read_text())
        self.taxonomy = TAXONOMY_PATH.read_text()

    def test_live_matrix_represents_all_taxonomy_rows_in_reviewed_order(self) -> None:
        route_ids = taxonomy_route_ids(self.taxonomy)

        self.assertEqual(len(route_ids), 29)
        self.assertEqual(
            route_ids,
            [route["id"] for route in self.matrix["code_areas"]],
        )
        self.assertEqual(
            validate_area_routing(
                self.matrix,
                self.taxonomy,
                canonical_areas=AREAS,
                suite_ids=MILESTONE_SUITE_IDS,
            ),
            [],
        )

    def test_missing_or_reordered_taxonomy_route_fails_drift_check(self) -> None:
        missing = copy.deepcopy(self.matrix)
        missing["code_areas"].pop()
        reordered = copy.deepcopy(self.matrix)
        reordered["code_areas"][0], reordered["code_areas"][1] = (
            reordered["code_areas"][1],
            reordered["code_areas"][0],
        )

        self.assertTrue(
            any(
                "taxonomy route IDs" in failure
                for failure in validate_area_routing(
                    missing, self.taxonomy, canonical_areas=AREAS
                )
            )
        )
        self.assertTrue(
            any(
                "reviewed taxonomy order" in failure
                for failure in validate_area_routing(
                    reordered, self.taxonomy, canonical_areas=AREAS
                )
            )
        )

    def test_path_normalization_rejects_non_workspace_or_noncanonical_paths(self) -> None:
        invalid = (
            "/etc/passwd",
            "../crates/trust-hir/src/lib.rs",
            "crates/../verification/matrix.toml",
            "./crates/trust-hir/src/lib.rs",
            "crates//trust-hir/src/lib.rs",
            r"crates\trust-hir\src\lib.rs",
            "crates/trust-hir/src/lib.rs\nforged",
            " crates/trust-hir/src/lib.rs",
        )

        for value in invalid:
            with self.subTest(value=value), self.assertRaises(AreaRoutingError):
                normalize_changed_path(value)

    def test_overlapping_path_routes_union_areas_classes_and_direct_suites(self) -> None:
        result = classify_changed_path(
            self.matrix,
            "crates/trust-runtime/src/hmi/tests/widget.rs",
        )

        self.assertFalse(result.unmapped)
        self.assertIn("hmi_runtime_web_ui", result.route_ids)
        self.assertIn("hmi_ui", result.area_ids)
        self.assertIn("control_security", result.area_ids)
        self.assertIn("browser_webview_visual", result.required_test_classes)
        self.assertEqual(result.suite_tiers, ("pr",))
        self.assertEqual(result.conditional_suite_tiers, ())

    def test_single_star_wildcard_does_not_cross_path_segments(self) -> None:
        direct = classify_changed_path(self.matrix, "scripts/perf_bench.py")
        nested = classify_changed_path(self.matrix, "scripts/nested/perf_bench.py")

        self.assertIn("performance_hot_paths", direct.route_ids)
        self.assertIn("verification_tooling", nested.route_ids)
        self.assertNotIn("performance_hot_paths", nested.route_ids)

    def test_double_star_wildcard_preserves_recursive_routes(self) -> None:
        cargo_manifest = classify_changed_path(self.matrix, "crates/a/b/Cargo.toml")
        webview = classify_changed_path(
            self.matrix,
            "editors/vscode/src/a/b/webview/panel.ts",
        )

        self.assertIn("security_supply_chain", cargo_manifest.route_ids)
        self.assertIn("hmi_runtime_web_ui", webview.route_ids)

    def test_verification_program_control_and_evidence_paths_are_routed(self) -> None:
        paths = (
            "AGENTS.md",
            ".codex/skills/trust-test-authoring/SKILL.md",
            "docs/internal/testing/checklists/plc-verification-program/implementation-board.md",
            "docs/internal/testing/evidence/plc-verification-program/2026-07-18/evidence.md",
        )
        for path in paths:
            with self.subTest(path=path):
                result = classify_changed_path(self.matrix, path)
                self.assertFalse(result.unmapped)
                self.assertIn("verification_tooling", result.route_ids)
                self.assertIn("verification", result.area_ids)

    def test_pre_matrix_release_paths_are_all_explicitly_routed(self) -> None:
        expected_routes = {
            ".gitignore": "platform_package",
            "justfile": "verification_tooling",
            "docs/IEC_DECISIONS.md": "hir_type_diagnostics",
            "docs/IEC_DEVIATIONS.md": "hir_type_diagnostics",
            "docs/internal/architecture/runtime-safety-fail-closed-contract.md":
                "runtime_scheduler_lifecycle",
            "docs/internal/testing/checklists/plc-verification-program-checklist.md":
                "verification_tooling",
            "docs/internal/testing/checklists/runtime-safety-fail-closed-checklist.md":
                "verification_tooling",
            "docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners/cdp_peer_topology_failure.js":
                "vscode_extension",
            "docs/specs/02-data-types.md": "hir_type_diagnostics",
            "docs/specs/06-statements.md": "hir_type_diagnostics",
            "docs/specs/07-standard-functions.md": "hir_type_diagnostics",
            "docs/specs/08-standard-function-blocks.md": "hir_type_diagnostics",
            "docs/specs/09-semantic-rules.md": "hir_type_diagnostics",
            "docs/specs/10-runtime-semantics.md": "runtime_scheduler_lifecycle",
            "docs/specs/11-runtime-engine.md": "runtime_scheduler_lifecycle",
            "docs/specs/12-bytecode.md": "bytecode_encoder_validator_container",
            "docs/specs/13-debug-adapter.md": "debug_authority_lifecycle",
            "docs/specs/14-lsp.md": "lsp_protocol_boundary",
            "docs/specs/19-project-model.md": "trust_dev_cli",
            "docs/specs/22-developer-workflows.md": "trust_dev_cli",
            "docs/specs/23-connector-status.md": "ads_opcua_connectors",
            "docs/specs/24-release-evidence.md": "public_docs_release_version",
            "docs/specs/README.md": "hir_type_diagnostics",
            "docs/specs/coverage/standard-functions-coverage.md":
                "hir_type_diagnostics",
            "tests/fixtures/mp001/hir-type-checking-test-discovery-baseline.list":
                "verification_tooling",
            "tests/fixtures/mp001/lsp-test-discovery-baseline.list":
                "verification_tooling",
        }

        for path, route_id in expected_routes.items():
            with self.subTest(path=path):
                result = classify_changed_path(self.matrix, path)
                self.assertFalse(result.unmapped)
                self.assertIn(route_id, result.route_ids)

    def test_unmatched_path_is_default_denied(self) -> None:
        result = classify_changed_path(self.matrix, "unmodeled/new_surface.xyz")

        self.assertTrue(result.unmapped)
        self.assertEqual(result.route_ids, ())
        self.assertEqual(result.area_ids, ())
        self.assertEqual(result.suite_tiers, ())

    def test_refactor_row_is_an_intent_overlay_not_a_path_inference(self) -> None:
        overlay = intent_overlay(self.matrix, "refactor")
        ordinary = classify_changed_path(
            self.matrix,
            "crates/trust-hir/src/lib.rs",
        )

        self.assertEqual(overlay.route_ids, ("refactor_only",))
        self.assertEqual(overlay.area_ids, ())
        self.assertIn("behavior_lock", overlay.required_test_classes)
        self.assertEqual(overlay.suite_tiers, ("pr",))
        self.assertEqual(overlay.conditional_suite_tiers, ())
        self.assertNotIn("refactor_only", ordinary.route_ids)

    def test_runtime_and_verification_helper_fallbacks_are_exhaustive(self) -> None:
        expected = {
            "crates/trust-runtime/Cargo.toml": "supply_chain_platform",
            "crates/trust-runtime/src/control.rs": "control_security",
            "crates/trust-runtime/src/hmi.rs": "hmi_ui",
            "crates/trust-runtime/src/web.rs": "control_security",
            "crates/trust-runtime/src/connectors/contract.rs": "protocols",
            "crates/trust-runtime-core/src/scheduler.rs": "runtime_safety",
            "crates/trust-runtime-core/src/retain.rs": "runtime_safety",
            "crates/verification-cases/src/lib.rs": "verification",
        }

        for path, area in expected.items():
            with self.subTest(path=path):
                route = classify_changed_path(self.matrix, path)
                self.assertFalse(route.unmapped)
                self.assertIn(area, route.area_ids)

        tracked = subprocess.run(
            [
                "git",
                "ls-files",
                "crates/trust-runtime",
                "crates/trust-runtime-core",
                "crates/verification-cases",
            ],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            check=True,
        ).stdout.splitlines()
        self.assertEqual(
            [path for path in tracked if classify_changed_path(self.matrix, path).unmapped],
            [],
        )

    def test_matrix_root_intent_and_loaded_suite_references_are_closed(self) -> None:
        root_extra = copy.deepcopy(self.matrix)
        root_extra["unexpected"] = "value"
        intent_extra = copy.deepcopy(self.matrix)
        intent_extra["intent_requirements"][0]["unexpected"] = "value"
        missing_hardware = set(MILESTONE_SUITE_IDS) - {"hardware_lab"}

        root_failures = validate_area_routing(
            root_extra,
            self.taxonomy,
            canonical_areas=AREAS,
            suite_ids=MILESTONE_SUITE_IDS,
        )
        intent_failures = validate_area_routing(
            intent_extra,
            self.taxonomy,
            canonical_areas=AREAS,
            suite_ids=MILESTONE_SUITE_IDS,
        )
        suite_failures = validate_area_routing(
            self.matrix,
            self.taxonomy,
            canonical_areas=AREAS,
            suite_ids=missing_hardware,
        )

        self.assertTrue(any("planning matrix fields drift" in item for item in root_failures))
        self.assertTrue(any("intent_requirements[0] fields drift" in item for item in intent_failures))
        self.assertTrue(any("unknown values ['hardware_lab']" in item for item in suite_failures))

    def test_area_decision_ref_is_optional_but_unknown_fields_stay_closed(self) -> None:
        with_decision = copy.deepcopy(self.matrix)
        with_decision["areas"][0]["decision_ref"] = "SPEC_IEC_DECISIONS_001"
        with_extra = copy.deepcopy(with_decision)
        with_extra["areas"][0]["invented"] = "value"

        self.assertEqual(
            validate_area_routing(
                with_decision,
                self.taxonomy,
                canonical_areas=AREAS,
                suite_ids=MILESTONE_SUITE_IDS,
            ),
            [],
        )
        self.assertTrue(
            any(
                "areas[0] fields drift" in failure
                for failure in validate_area_routing(
                    with_extra,
                    self.taxonomy,
                    canonical_areas=AREAS,
                    suite_ids=MILESTONE_SUITE_IDS,
                )
            )
        )

    def test_full_validator_checks_area_decision_ref_authority(self) -> None:
        invalid = Validator()
        invalid.load_records()
        invalid.matrix["areas"][0]["decision_ref"] = "SPEC_UNKNOWN_DECISION"
        invalid.validate_matrix()

        valid = Validator()
        valid.load_records()
        valid.matrix["areas"][0]["decision_ref"] = "SPEC_IEC_DECISIONS_001"
        valid.validate_matrix()

        self.assertTrue(
            any("decision_ref must name an active" in item.message for item in invalid.failures),
            [item.message for item in invalid.failures],
        )
        self.assertFalse(
            any("decision_ref" in item.message for item in valid.failures),
            [item.message for item in valid.failures],
        )

    def test_full_validator_uses_loaded_milestone_suite_ids(self) -> None:
        validator = Validator()
        validator.matrix = copy.deepcopy(self.matrix)
        validator.suites = {
            suite_id: {"id": suite_id}
            for suite_id in MILESTONE_SUITE_IDS - {"hardware_lab"}
        }

        validator.validate_matrix()

        self.assertTrue(
            any(
                "unknown values ['hardware_lab']" in failure.message
                for failure in validator.failures
            ),
            [failure.message for failure in validator.failures],
        )


if __name__ == "__main__":
    unittest.main()
