"""Focused tests for the metadata-only catalogue gap report."""

from __future__ import annotations

import unittest

from scripts.verification.frozen_catalog_gaps import analyze_frozen_catalog_gaps


class FrozenCatalogGapsTests(unittest.TestCase):
    def test_separates_missing_spec_authority_from_owned_nonmapping(self) -> None:
        report = analyze_frozen_catalog_gaps(
            denominator={
                "reviews": [
                    _review("DISC_A", reason="no_reviewed_spec_or_invariant_binding"),
                    _review("DISC_B", reason="gate_inventory_owned"),
                    _mapped_review("DISC_C"),
                ]
            },
            catalog={"tests": []},
            invariants=[],
        )

        self.assertEqual(1, report["summary"]["missing_spec_authority_tests"])
        self.assertEqual(1, report["summary"]["owned_nonmapping_tests"])
        self.assertEqual(1, report["summary"]["catalog_mapped"])
        self.assertEqual(0, report["summary"]["confirmed_missing_tests"])
        self.assertEqual(
            "blocked_until_existing_test_mapping_complete",
            report["missing_test_demand_status"],
        )

    def test_confirms_unmapped_specified_behaviors_only_after_existing_queue_closes(
        self,
    ) -> None:
        invariant = (
            "verification/invariants/example/INV_ONE.toml",
            {
                "id": "INV_ONE",
                "owner": "example",
                "spec": {"status": "specified"},
                "behavior": [
                    {"case_family": "happy_path", "rationale": "Exact result."},
                    {"case_family": "boundary_high", "rationale": "Exact bound."},
                ],
                "coverage": {
                    "cells": [
                        {"dimension": "happy_path", "state": "covered"},
                        {"dimension": "boundary_high", "state": "gap_open"},
                    ]
                },
                "missing": ["cataloged_behavior_tests"],
                "tests": ["TEST_ONE"],
            },
        )
        report = analyze_frozen_catalog_gaps(
            denominator={"reviews": [_mapped_review("DISC_C")]},
            catalog={
                "tests": [
                    {
                        "id": "TEST_ONE",
                        "mapping_behavior_refs": ["INV_ONE#behavior:0"],
                    }
                ]
            },
            invariants=[invariant],
        )

        self.assertEqual(
            "ready",
            report["missing_test_demand_status"],
        )
        self.assertEqual(
            ["INV_ONE#behavior:1"],
            [
                row["behavior_ref"]
                for row in report["confirmed_missing_tests"]
            ],
        )


def _review(discovery_id: str, *, reason: str) -> dict[str, str]:
    return {
        "discovery_id": discovery_id,
        "discovery_source_kind": "rust_unit_test",
        "path": "crates/example/src/lib.rs",
        "name": discovery_id.lower(),
        "ignore_state": "not_ignored",
        "disposition": "reviewed_nonmapping",
        "reason_code": reason,
        "last_reviewed": "2026-07-26",
    }


def _mapped_review(discovery_id: str) -> dict[str, str]:
    return {
        "discovery_id": discovery_id,
        "discovery_source_kind": "rust_unit_test",
        "path": "crates/example/src/lib.rs",
        "name": discovery_id.lower(),
        "ignore_state": "not_ignored",
        "disposition": "catalog_mapped",
        "catalog_test_id": f"TEST_{discovery_id}",
        "last_reviewed": "2026-07-26",
    }


if __name__ == "__main__":
    unittest.main()
