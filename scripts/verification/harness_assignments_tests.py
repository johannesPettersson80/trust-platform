from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.harness_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "harness-actionable-assertions.json"
)


class HarnessAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the reviewed harness bulk assignment generator is missing",
        )
        return importlib.import_module(MODULE)

    def inventories(self):
        subject = self.subject()
        return subject._canonical_inventories(
            ROOT,
            json.loads(INVENTORY.read_text()),
        )

    def test_generator_module_exists(self) -> None:
        self.subject()

    def test_reviewed_batch_is_exactly_six_tests_and_21_assertions(self) -> None:
        subject = self.subject()
        inventories = self.inventories()
        self.assertEqual(6, len(subject.TEST_NAMES))
        self.assertEqual(
            set(subject.TEST_NAMES),
            {row["test_name"] for row in inventories.values()},
        )
        self.assertEqual(
            21,
            sum(len(row["assertions"]) for row in inventories.values()),
        )

    def test_every_assertion_has_the_exact_reviewed_route(self) -> None:
        subject = self.subject()
        inventories = self.inventories()
        actual = {}
        dispositions = []
        for raw in inventories.values():
            name = raw["test_name"]
            for assertion in raw["assertions"]:
                assertion_id = assertion["assertion_id"]
                actual[assertion_id] = subject._assertion_route(
                    test_name=name,
                    assertion_id=assertion_id,
                )
                dispositions.append(
                    subject._disposition(
                        test_name=name,
                        assertion_id=assertion_id,
                    )
                )
        self.assertEqual(subject.ASSERTION_ROUTES, actual)
        self.assertEqual(15, dispositions.count("oracle_bound"))
        self.assertEqual(6, dispositions.count("setup_or_precondition"))
        self.assertEqual(21, len(dispositions))

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._assertion_route(
                test_name="from_source",
                assertion_id="ASSERT_NOT_REVIEWED",
            )

    def test_assignments_are_exhaustive_and_keep_constructor_unwraps_setup(
        self,
    ) -> None:
        subject = self.subject()
        document = subject.generate_assignments(
            root=ROOT,
            inventory_document=json.loads(INVENTORY.read_text()),
        )
        self.assertEqual(6, document["summary"]["tests"])
        self.assertEqual(21, document["summary"]["assertions"])
        self.assertEqual(15, document["summary"]["direct_oracle"])
        self.assertEqual(6, document["summary"]["setup_or_precondition"])
        self.assertEqual(
            21,
            sum(
                len(row["assertion_assignments"])
                for row in document["assignments"]
            ),
        )
        self.assertTrue(
            all(
                len(row["setup_assertion_ids"]) == 1
                for row in document["assignments"]
            )
        )

    def test_semantic_lock_rejects_expression_drift(self) -> None:
        subject = self.subject()
        inventories = self.inventories()
        lock = subject._review_lock_document(inventories)
        mutated = json.loads(json.dumps(inventories))
        first = next(iter(mutated.values()))
        first["assertions"][0]["expression"] += " drift"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "lock.json"
            path.write_text(json.dumps(lock))
            with self.assertRaisesRegex(ValueError, "drifted"):
                subject._validate_review_lock(
                    root,
                    mutated,
                    Path("lock.json"),
                )

    def test_semantic_lock_rejects_binding_remap(self) -> None:
        subject = self.subject()
        inventories = self.inventories()
        lock = subject._review_lock_document(inventories)
        lock["tests"][0]["assertions"][0]["behavior_label"] = (
            subject.EXACT_ACCOUNTING
        )
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

    def test_review_lock_records_scope_exclusions(self) -> None:
        subject = self.subject()
        lock = subject._review_lock_document(self.inventories())
        self.assertEqual(6, len(lock["tests"]))
        for row in lock["tests"]:
            self.assertEqual(
                list(subject.TEST_EXCLUSIONS[row["name"]]),
                row["exclusions"],
            )

    def test_named_behavior_index_resolution_covers_every_route(self) -> None:
        subject = self.subject()
        indexes = subject._behavior_indexes(ROOT)
        self.assertEqual(
            set(subject.ASSERTION_ROUTES.values()),
            set(indexes),
        )


if __name__ == "__main__":
    unittest.main()
