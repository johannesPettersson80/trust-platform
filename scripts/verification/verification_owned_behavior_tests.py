"""Focused contract tests for the verification-owned Phase 18 route."""

from __future__ import annotations

import unittest

from scripts.verification.verification_owned_behavior_assertions import (
    build_assertion_facts,
)
from scripts.verification.verification_owned_behavior_campaign import (
    _new_test_identity,
)
from scripts.verification.verification_owned_behavior_routes import (
    BEHAVIOR_TEST_NAMES,
    PROVENANCE_REFRESH_TEST_NAMES,
)


class VerificationOwnedBehaviorTests(unittest.TestCase):
    def test_routes_cover_eleven_demands_with_fifteen_tests(self) -> None:
        self.assertEqual(11, len(BEHAVIOR_TEST_NAMES))
        demand_names = {
            name
            for route_names in BEHAVIOR_TEST_NAMES.values()
            for name in route_names
        }
        refresh_names = {
            name
            for route_names in PROVENANCE_REFRESH_TEST_NAMES.values()
            for name in route_names
        }
        self.assertEqual(12, len(demand_names))
        self.assertEqual(3, len(refresh_names))
        self.assertFalse(demand_names & refresh_names)
        self.assertEqual(
            14,
            sum(len(names) for names in BEHAVIOR_TEST_NAMES.values()),
        )
        self.assertEqual(
            3,
            sum(
                len(names)
                for names in PROVENANCE_REFRESH_TEST_NAMES.values()
            ),
        )

    def test_new_exact_targets_match_the_real_cargo_binaries(self) -> None:
        case = _new_test_identity(
            {
                "discovery_id": "DISC_CASE",
                "path": "crates/verification-cases/src/lib.rs",
                "name": "blocked_cases_are_recorded_without_executing_the_runner",
            }
        )
        self.assertEqual("verification-cases", case["package"])
        self.assertEqual("lib", case["target_kind"])
        self.assertEqual(
            "tests::blocked_cases_are_recorded_without_executing_the_runner",
            case["full_name"],
        )

        inventory = _new_test_identity(
            {
                "discovery_id": "DISC_IEC",
                "path": "crates/trust-hir/tests/iec_table_coverage.rs",
                "name": "malformed_iec_table_map_is_rejected",
            }
        )
        self.assertEqual("trust-hir", inventory["package"])
        self.assertEqual("test", inventory["target_kind"])
        self.assertEqual("iec_table_coverage", inventory["target_name"])

    def test_assertion_requests_preserve_exact_discovery_identity(self) -> None:
        assignments = [
            {
                "discovery_id": "DISC_CASE",
                "source_kind": "rust_unit_test",
                "path": "crates/verification-cases/src/lib.rs",
                "name": "blocked_cases_are_recorded_without_executing_the_runner",
            },
            {
                "discovery_id": "DISC_IEC",
                "source_kind": "rust_integration_test",
                "path": "crates/trust-hir/tests/iec_table_coverage.rs",
                "name": "iec_table_coverage_report",
            },
        ]

        facts = build_assertion_facts(assignments)

        self.assertEqual(
            {"DISC_CASE", "DISC_IEC"},
            {row["discovery_id"] for row in facts},
        )


if __name__ == "__main__":
    unittest.main()
