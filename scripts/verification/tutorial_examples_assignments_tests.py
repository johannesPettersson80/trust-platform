from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.tutorial_examples_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "tutorial-examples-4-assertions.json"
)


class TutorialExamplesAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the tutorial-examples assignment generator is missing",
        )
        return importlib.import_module(MODULE)

    def selected_document(self):
        subject = self.subject()
        return subject.select_live_inventories(
            root=ROOT,
            inventory_document=json.loads(INVENTORY.read_text()),
        )

    def inventories(self):
        subject = self.subject()
        return subject._canonical_inventories(
            ROOT,
            self.selected_document(),
        )

    def test_reviewed_batch_is_exactly_four_tests_and_23_assertions(
        self,
    ) -> None:
        subject = self.subject()
        inventories = self.inventories()
        self.assertEqual(
            subject.REVIEWED_IDENTITIES,
            {
                discovery_id: row["test_name"]
                for discovery_id, row in inventories.items()
            },
        )
        self.assertEqual(
            23,
            sum(len(row["assertions"]) for row in inventories.values()),
        )

    def test_all_assertions_have_exact_reviewed_routes(self) -> None:
        subject = self.subject()
        counts: Counter[tuple[str, str]] = Counter()
        dispositions: Counter[str] = Counter()
        for raw in self.inventories().values():
            for assertion in raw["assertions"]:
                invariant, behavior, disposition = subject._route(
                    test_name=raw["test_name"],
                    assertion_id=assertion["assertion_id"],
                )
                counts[(invariant, behavior)] += 1
                dispositions[disposition] += 1
        self.assertEqual(
            {
                (subject.TUTORIALS, subject.COMPILE): 2,
                (subject.TUTORIALS, subject.BLINKER): 6,
                (subject.TUTORIALS, subject.TRAFFIC): 3,
                (subject.TUTORIALS, subject.MOTOR): 12,
            },
            counts,
        )
        self.assertEqual(
            Counter({"oracle_bound": 20, "setup_or_precondition": 3}),
            dispositions,
        )

    def test_only_behavior_compile_expects_are_setup(self) -> None:
        subject = self.subject()
        expected = {
            (
                subject.BLINKER_TEST,
                "ASSERT_550377BB91DB706D1C3A97D5",
            ),
            (
                subject.TRAFFIC_TEST,
                "ASSERT_95AB4F440ECA3A14839193D6",
            ),
            (
                subject.MOTOR_TEST,
                "ASSERT_11F4BD4AEDDD512F31BFA127",
            ),
        }
        actual = {
            (test_name, assertion_id)
            for test_name, routes in subject.REVIEWED_ROUTES.items()
            for assertion_id, route in routes.items()
            if route[2] == "setup_or_precondition"
        }
        self.assertEqual(expected, actual)

    def test_cycle_health_and_compile_assertions_are_direct(self) -> None:
        subject = self.subject()
        reviewed = (
            (
                subject.COMPILE_TEST,
                "ASSERT_5472BA873EE5C7311647265A",
                subject.COMPILE,
            ),
            (
                subject.COMPILE_TEST,
                "ASSERT_5039D55B6590AE8A9558C75F",
                subject.COMPILE,
            ),
            (
                subject.BLINKER_TEST,
                "ASSERT_D4425418EE6A1168C683EF88",
                subject.BLINKER,
            ),
            (
                subject.TRAFFIC_TEST,
                "ASSERT_CC58CEE92FB05766CAD3D7B0",
                subject.TRAFFIC,
            ),
            (
                subject.MOTOR_TEST,
                "ASSERT_B746A270D668C9168A40DD9A",
                subject.MOTOR,
            ),
        )
        for test_name, assertion_id, behavior in reviewed:
            with self.subTest(
                test_name=test_name,
                assertion_id=assertion_id,
            ):
                self.assertEqual(
                    (subject.TUTORIALS, behavior, "oracle_bound"),
                    subject._route(
                        test_name=test_name,
                        assertion_id=assertion_id,
                    ),
                )

    def test_denominator_routes_are_post_map_idempotent(self) -> None:
        subject = self.subject()
        discovery_id = "DISC_F46BC74C7DE6111123BE"
        base = {
            "discovery_id": discovery_id,
            "path": subject.PATH,
            "name": subject.BLINKER_TEST,
        }
        reviewed = {
            **base,
            "disposition": "reviewed_nonmapping",
            "reason_code": "no_reviewed_spec_or_invariant_binding",
        }
        mapped = {
            **base,
            "disposition": "catalog_mapped",
            "catalog_test_id": "TEST_AUTO_F46BC74C7DE6111123BE",
        }
        self.assertTrue(
            subject._denominator_row_is_live(
                reviewed,
                discovery_id=discovery_id,
                expected_name=subject.BLINKER_TEST,
            )
        )
        self.assertTrue(
            subject._denominator_row_is_live(
                mapped,
                discovery_id=discovery_id,
                expected_name=subject.BLINKER_TEST,
            )
        )
        self.assertFalse(
            subject._denominator_row_is_live(
                {**mapped, "catalog_test_id": "TEST_WRONG"},
                discovery_id=discovery_id,
                expected_name=subject.BLINKER_TEST,
            )
        )

    def test_semantic_exclusions_bound_each_tutorial_claim(self) -> None:
        subject = self.subject()
        combined = " ".join(
            exclusion
            for exclusions in subject.TEST_EXCLUSIONS.values()
            for exclusion in exclusions
        ).lower()
        for required in (
            "future tutorial",
            "serialized byte",
            "lsp",
            "ton q",
            "pt changes",
            "threshold-cycle",
            "phase",
            "simultaneous start",
            "fault reset",
            "physical motor",
            "iec-wide",
        ):
            self.assertIn(required, combined)

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._route(
                test_name=subject.BLINKER_TEST,
                assertion_id="ASSERT_NOT_REVIEWED",
            )

    def test_semantic_lock_rejects_expression_drift(self) -> None:
        subject = self.subject()
        inventories = self.inventories()
        lock = subject._review_lock_document(inventories)
        lock["tests"][0]["assertions"][0]["expression"] += " drift"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "lock.json"
            path.write_text(json.dumps(lock))
            with self.assertRaisesRegex(ValueError, "drifted"):
                subject._validate_review_lock(
                    root,
                    inventories,
                    Path("lock.json"),
                )

    def test_assignment_document_is_exhaustive(self) -> None:
        subject = self.subject()
        document = subject._generate_assignment_document(
            self.inventories(),
            subject._behavior_indexes(ROOT),
        )
        self.assertEqual(4, document["summary"]["tests"])
        self.assertEqual(23, document["summary"]["assertions"])
        self.assertEqual(20, document["summary"]["direct_oracle"])
        self.assertEqual(3, document["summary"]["setup_or_precondition"])
        self.assertEqual(0, document["summary"]["non_oracle_observation"])
        self.assertEqual(
            23,
            sum(
                len(row["assertion_assignments"])
                for row in document["assignments"]
            ),
        )
        self.assertEqual(
            3,
            sum(
                len(row["setup_assertion_ids"])
                for row in document["assignments"]
            ),
        )
        self.assertTrue(
            all(
                not row["non_oracle_assertion_ids"]
                for row in document["assignments"]
            )
        )

    def test_behavior_lookup_matches_current_invariant_order(self) -> None:
        subject = self.subject()
        indexes = subject._behavior_indexes(ROOT)
        self.assertEqual(0, indexes[(subject.TUTORIALS, subject.COMPILE)])
        self.assertEqual(1, indexes[(subject.TUTORIALS, subject.BLINKER)])
        self.assertEqual(2, indexes[(subject.TUTORIALS, subject.TRAFFIC)])
        self.assertEqual(3, indexes[(subject.TUTORIALS, subject.MOTOR)])


if __name__ == "__main__":
    unittest.main()
