"""Focused tests for frozen owned-nonmapping classification."""

from __future__ import annotations

import unittest

from scripts.verification.frozen_nonmapping_classifications import (
    classify_nonmappings,
)
from scripts.verification.test_catalog_denominator import (
    NONMAPPING_REASON_DEFINITIONS,
)


class FrozenNonmappingClassificationTests(unittest.TestCase):
    def test_exact_identity_is_reclassified_without_changing_disposition(self) -> None:
        denominator = {
            "schema_version": 1,
            "id": "TEST_CATALOG_DENOMINATOR_REVIEW_V1",
            "title": "Reviewed existing-test denominator",
            "denominator_basis": "all_live_existing_test_scanner_facts",
            "review_basis": "explicit_per_discovery_id_no_semantic_inference",
            "reason_definitions": dict(NONMAPPING_REASON_DEFINITIONS),
            "last_reviewed": "2026-07-25",
            "reviews": [
                {
                    "discovery_id": "DISC_00000000000000000001",
                    "discovery_source_kind": "rust_unit_test",
                    "path": "src/lib.rs",
                    "name": "fixture_only",
                    "ignore_state": "not_ignored",
                    "disposition": "reviewed_nonmapping",
                    "reason_code": "no_reviewed_spec_or_invariant_binding",
                    "last_reviewed": "2026-07-25",
                }
            ],
        }
        updated = classify_nonmappings(
            denominator=denominator,
            classifications=[
                {
                    "path": "src/lib.rs",
                    "name": "fixture_only",
                    "reason_code": "not_catalogable_product_oracle",
                }
            ],
            reviewed_date="2026-07-26",
        )
        self.assertEqual(
            "not_catalogable_product_oracle",
            updated["reviews"][0]["reason_code"],
        )
        self.assertEqual("reviewed_nonmapping", updated["reviews"][0]["disposition"])
        self.assertEqual("2026-07-26", updated["last_reviewed"])

    def test_missing_or_mapping_debt_reason_fails_closed(self) -> None:
        denominator = {
            "reason_definitions": dict(NONMAPPING_REASON_DEFINITIONS),
            "reviews": [],
        }
        with self.assertRaisesRegex(ValueError, "reason is not owned"):
            classify_nonmappings(
                denominator=denominator,
                classifications=[
                    {
                        "path": "src/lib.rs",
                        "name": "test",
                        "reason_code": "no_reviewed_spec_or_invariant_binding",
                    }
                ],
                reviewed_date="2026-07-26",
            )


if __name__ == "__main__":
    unittest.main()
