from __future__ import annotations

import unittest

from scripts.verification.authority_route_execution_plan import (
    build_authority_route_execution_plan,
)


PATH = (
    "crates/trust-lsp/src/config/configuration_contract_tests.rs"
)


class AuthorityRouteExecutionPlanTests(unittest.TestCase):
    def test_groups_exact_routes_into_closed_path_batches(self) -> None:
        result = build_authority_route_execution_plan(
            routes=[
                {
                    "discovery_id": "DISC_ROUTE",
                    "path": PATH,
                    "name": "reviewed_case",
                }
            ],
            facts=[
                {
                    "stable_id": "DISC_ROUTE",
                    "source_kind": "rust_unit_test",
                    "path": PATH,
                    "name": "reviewed_case",
                    "ignore_state": "not_ignored",
                },
                {
                    "stable_id": "DISC_MAPPED",
                    "source_kind": "rust_unit_test",
                    "path": PATH,
                    "name": "mapped_case",
                    "ignore_state": "not_ignored",
                },
            ],
            denominator_reviews=[
                {
                    "discovery_id": "DISC_ROUTE",
                    "discovery_source_kind": "rust_unit_test",
                    "path": PATH,
                    "name": "reviewed_case",
                    "ignore_state": "not_ignored",
                    "disposition": "reviewed_nonmapping",
                },
                {
                    "discovery_id": "DISC_MAPPED",
                    "discovery_source_kind": "rust_unit_test",
                    "path": PATH,
                    "name": "mapped_case",
                    "ignore_state": "not_ignored",
                    "disposition": "catalog_mapped",
                },
            ],
            catalog_tests=[
                {
                    "discovery_id": "DISC_MAPPED",
                    "discovery_source_kind": "rust_unit_test",
                    "path": PATH,
                    "name": "mapped_case",
                    "status": "mapped",
                },
                {
                    "id": "TEST_MANUAL_NON_SCANNER",
                    "path": "tests/manual.case",
                    "name": "manual_case",
                    "status": "mapped",
                },
            ],
        )

        self.assertEqual(
            [
                {
                    "expected_catalog_mapped": 1,
                    "expected_reviewed_nonmapping": 1,
                    "path": PATH,
                    "profile": "lsp-remaining",
                    "selected_discovery_ids": ["DISC_ROUTE"],
                }
            ],
            result["batches"],
        )
        self.assertEqual(
            {"DISC_ROUTE": {"path": PATH}},
            result["inventories"],
        )
        self.assertEqual(
            {"batches": 1, "catalog_companions": 1, "routes": 1},
            result["summary"],
        )

    def test_rejects_a_route_without_a_closed_execution_profile(self) -> None:
        path = "crates/unknown/src/tests.rs"
        with self.assertRaisesRegex(ValueError, "no closed execution profile"):
            build_authority_route_execution_plan(
                routes=[
                    {
                        "discovery_id": "DISC_UNKNOWN",
                        "path": path,
                        "name": "case",
                    }
                ],
                facts=[
                    {
                        "stable_id": "DISC_UNKNOWN",
                        "source_kind": "rust_unit_test",
                        "path": path,
                        "name": "case",
                        "ignore_state": "not_ignored",
                    }
                ],
                denominator_reviews=[
                    {
                        "discovery_id": "DISC_UNKNOWN",
                        "discovery_source_kind": "rust_unit_test",
                        "path": path,
                        "name": "case",
                        "ignore_state": "not_ignored",
                        "disposition": "reviewed_nonmapping",
                    }
                ],
                catalog_tests=[],
            )


if __name__ == "__main__":
    unittest.main()
