"""Focused tests for index-based hidden-ground-truth semantic review."""

from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from scripts.verification.mapping_pilot_review import (
    build_pilot_review,
    generate_pilot_review,
    resolve_pilot_fact,
)


def _candidate() -> dict[str, object]:
    return {
        "discovery_id": "DISC_00000000000000000001",
        "candidate_invariant_ids": ["INV_WRONG", "INV_RIGHT"],
    }


def _inventory() -> dict[str, object]:
    return {
        "source_digest": "sha256:" + "1" * 64,
        "inventory_digest": "sha256:" + "2" * 64,
        "assertions": [
            {"assertion_id": "ASSERT_" + "A" * 24},
            {"assertion_id": "ASSERT_" + "B" * 24},
        ],
        "unresolved_assertion_helpers": [],
    }


class MappingPilotReviewTests(unittest.TestCase):
    def test_pilot_review_rejects_catalog_plane_output_before_loading_candidates(
        self,
    ) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            unsafe = root / "verification/test-catalog.toml"
            with self.assertRaisesRegex(ValueError, "artifact quarantine"):
                generate_pilot_review(
                    root=root,
                    candidates_path=root / "missing-candidates.json",
                    output_path=unsafe,
                    candidate_index=1,
                    invariant_index=1,
                    oracle_assertion_bindings={1: [1]},
                    setup_assertion_indices=[],
                    non_oracle_assertion_indices=[],
                    reviewer_ids=["reviewer-a"],
                )

    @mock.patch(
        "scripts.verification.mapping_pilot_review.resolve_focused_rust_facts"
    )
    @mock.patch(
        "scripts.verification.mapping_pilot_review.load_test_catalog_denominator"
    )
    def test_pilot_review_resolves_one_file_without_full_scan(
        self,
        load_denominator: mock.Mock,
        resolve_focused: mock.Mock,
    ) -> None:
        fact = mock.Mock(
            stable_id="DISC_" + "A" * 20,
            source_kind="rust_unit_test",
            path="crates/example/src/lib.rs",
            name="exact_case",
            ignore_state="not_ignored",
        )
        resolve_focused.return_value = [fact]
        load_denominator.return_value = {"reviews": []}

        resolved = resolve_pilot_fact(
            root=Path("/workspace"),
            discovery_id=fact.stable_id,
        )

        self.assertEqual(fact.stable_id, resolved["discovery_id"])

    def test_display_indices_generate_exact_ids_and_complete_assertion_review(self) -> None:
        review = build_pilot_review(
            candidate=_candidate(),
            fact={},
            assertion_inventory=_inventory(),
            invariants={
                "INV_RIGHT": {
                    "risk": "maintenance",
                    "behavior": [{"rationale": "exact"}],
                }
            },
            invariant_index=2,
            oracle_assertion_bindings={2: [1]},
            setup_assertion_indices=[1],
            non_oracle_assertion_indices=[],
            reviewer_ids=["reviewer-a"],
        )

        self.assertEqual(["INV_RIGHT"], review["selected_invariant_ids"])
        self.assertEqual(
            ["INV_RIGHT#behavior:0"], review["selected_behavior_refs"]
        )
        self.assertEqual(
            ["ASSERT_" + "A" * 24, "ASSERT_" + "B" * 24],
            [
                assertion["assertion_id"]
                for assertion in review["assertion_reviews"]
            ],
        )
        self.assertEqual(
            ["setup_or_precondition", "oracle_bound"],
            [
                assertion["disposition"]
                for assertion in review["assertion_reviews"]
            ],
        )

    def test_each_oracle_assertion_binds_only_its_selected_behaviors(self) -> None:
        review = build_pilot_review(
            candidate=_candidate(),
            fact={},
            assertion_inventory=_inventory(),
            invariants={
                "INV_RIGHT": {
                    "risk": "maintenance",
                    "behavior": [
                        {"rationale": "accept"},
                        {"rationale": "reject"},
                    ],
                }
            },
            invariant_index=2,
            oracle_assertion_bindings={1: [1], 2: [2]},
            setup_assertion_indices=[],
            non_oracle_assertion_indices=[],
            reviewer_ids=["reviewer-a"],
        )
        by_id = {
            row["assertion_id"]: row["behavior_refs"]
            for row in review["assertion_reviews"]
        }

        self.assertEqual(
            ["INV_RIGHT#behavior:0"],
            by_id["ASSERT_" + "A" * 24],
        )
        self.assertEqual(
            ["INV_RIGHT#behavior:1"],
            by_id["ASSERT_" + "B" * 24],
        )

    def test_sampling_or_unresolved_helper_cannot_generate_acceptance(self) -> None:
        with self.assertRaisesRegex(ValueError, "classify every"):
            build_pilot_review(
                candidate=_candidate(),
                fact={},
                assertion_inventory=_inventory(),
                invariants={
                    "INV_RIGHT": {
                        "risk": "maintenance",
                        "behavior": [{}],
                    }
                },
                invariant_index=2,
                oracle_assertion_bindings={2: [1]},
                setup_assertion_indices=[],
                non_oracle_assertion_indices=[],
                reviewer_ids=["reviewer-a"],
            )

        inventory = _inventory()
        inventory["unresolved_assertion_helpers"] = ["expect_external:unresolved"]
        with self.assertRaisesRegex(ValueError, "unresolved assertion helpers"):
            build_pilot_review(
                candidate=_candidate(),
                fact={},
                assertion_inventory=inventory,
                invariants={
                    "INV_RIGHT": {
                        "risk": "maintenance",
                        "behavior": [{}],
                    }
                },
                invariant_index=2,
                oracle_assertion_bindings={2: [1]},
                setup_assertion_indices=[1],
                non_oracle_assertion_indices=[],
                reviewer_ids=["reviewer-a"],
            )


if __name__ == "__main__":
    unittest.main()
