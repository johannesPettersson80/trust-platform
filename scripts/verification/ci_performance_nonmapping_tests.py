"""Tests for exact CI performance-probe nonmapping classification."""

from __future__ import annotations

import unittest

from scripts.verification.ci_performance_nonmapping import (
    PATH,
    PERFORMANCE_TESTS,
    SOURCE_REASON,
    TARGET_REASON,
    classify_ci_performance_probes,
)


def _document(reason: str = SOURCE_REASON) -> dict:
    reviews = [
        {
            "discovery_id": f"DISC_{index}",
            "discovery_source_kind": "rust_test",
            "path": PATH,
            "name": name,
            "ignore_state": "not_ignored",
            "disposition": "reviewed_nonmapping",
            "reason_code": reason,
            "last_reviewed": "2026-07-27",
        }
        for index, name in enumerate(sorted(PERFORMANCE_TESTS), start=1)
    ]
    reviews.append(
        {
            "discovery_id": "DISC_OTHER",
            "discovery_source_kind": "rust_test",
            "path": PATH,
            "name": "ci_json_summary_mode_contract_is_stable",
            "ignore_state": "not_ignored",
            "disposition": "reviewed_nonmapping",
            "reason_code": SOURCE_REASON,
            "last_reviewed": "2026-07-27",
        }
    )
    return {
        "schema_version": 1,
        "id": "TEST_CATALOG_DENOMINATOR",
        "title": "Reviewed existing-test denominator",
        "denominator_basis": "all_live_existing_test_scanner_facts",
        "review_basis": "exact_discovery_identity_and_catalog_subtraction",
        "last_reviewed": "2026-07-27",
        "reason_definitions": {},
        "reviews": reviews,
    }


class CiPerformanceNonmappingTests(unittest.TestCase):
    def test_changes_only_the_three_reviewed_timing_probes(self) -> None:
        document = _document()
        updated = classify_ci_performance_probes(
            document=document,
            reviewed_date="2026-07-28",
        )
        by_name = {row["name"]: row for row in updated["reviews"]}
        for name in PERFORMANCE_TESTS:
            self.assertEqual(TARGET_REASON, by_name[name]["reason_code"])
            self.assertEqual("2026-07-28", by_name[name]["last_reviewed"])
        other = by_name["ci_json_summary_mode_contract_is_stable"]
        self.assertEqual(SOURCE_REASON, other["reason_code"])
        self.assertEqual("2026-07-27", other["last_reviewed"])
        self.assertEqual("2026-07-28", updated["last_reviewed"])

    def test_is_idempotent_after_complete_classification(self) -> None:
        document = _document(TARGET_REASON)
        self.assertIsNot(
            document,
            classify_ci_performance_probes(
                document=document,
                reviewed_date="2026-07-28",
            ),
        )
        self.assertEqual(
            document,
            classify_ci_performance_probes(
                document=document,
                reviewed_date="2026-07-28",
            ),
        )

    def test_rejects_missing_probe(self) -> None:
        document = _document()
        document["reviews"].pop(0)
        with self.assertRaisesRegex(ValueError, "differs from reviewed probes"):
            classify_ci_performance_probes(
                document=document,
                reviewed_date="2026-07-28",
            )

    def test_rejects_partial_or_unexpected_reason_state(self) -> None:
        document = _document()
        document["reviews"][0]["reason_code"] = TARGET_REASON
        with self.assertRaisesRegex(ValueError, "partially or unexpectedly"):
            classify_ci_performance_probes(
                document=document,
                reviewed_date="2026-07-28",
            )

    def test_rejects_catalog_mapping_or_invalid_date(self) -> None:
        document = _document()
        document["reviews"][0]["disposition"] = "catalog_mapped"
        document["reviews"][0].pop("reason_code")
        document["reviews"][0]["catalog_test_id"] = "TEST_AUTO_X"
        with self.assertRaisesRegex(ValueError, "remain reviewed nonmappings"):
            classify_ci_performance_probes(
                document=document,
                reviewed_date="2026-07-28",
            )
        with self.assertRaisesRegex(ValueError, "YYYY-MM-DD"):
            classify_ci_performance_probes(
                document=_document(),
                reviewed_date="28-07-2026",
            )


if __name__ == "__main__":
    unittest.main()
