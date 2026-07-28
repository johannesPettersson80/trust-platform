from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.hot_reload_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "hot-reload-actionable-assertions.json"
)


class HotReloadAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the reviewed hot-reload bulk assignment generator is missing",
        )
        return importlib.import_module(MODULE)

    def inventories(self):
        subject = self.subject()
        document = json.loads(INVENTORY.read_text())
        return subject._canonical_inventories(ROOT, document)

    def test_reviewed_batch_is_exactly_six_tests_and_84_assertions(self) -> None:
        subject = self.subject()
        inventories = self.inventories()
        self.assertEqual(6, len(subject.TEST_BINDINGS))
        self.assertEqual(
            set(subject.TEST_BINDINGS),
            {row["test_name"] for row in inventories.values()},
        )
        self.assertEqual(
            84,
            sum(len(row["assertions"]) for row in inventories.values()),
        )

    def test_every_assertion_has_an_exact_reviewed_disposition(self) -> None:
        subject = self.subject()
        inventories = self.inventories()
        dispositions = []
        for raw in inventories.values():
            name = raw["test_name"]
            dispositions.extend(
                subject._disposition(
                    test_name=name,
                    assertion_id=assertion["assertion_id"],
                )
                for assertion in raw["assertions"]
            )
        self.assertEqual(37, dispositions.count("oracle_bound"))
        self.assertEqual(47, dispositions.count("setup_or_precondition"))
        self.assertEqual(84, len(dispositions))

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._disposition(
                test_name=next(iter(subject.TEST_BINDINGS)),
                assertion_id="ASSERT_NOT_REVIEWED",
            )

    def test_all_review_locks_preserve_exact_exclusions(self) -> None:
        subject = self.subject()
        lock = subject._review_lock_document(self.inventories())
        self.assertEqual(6, len(lock["tests"]))
        for row in lock["tests"]:
            self.assertEqual(
                list(subject.TEST_EXCLUSIONS[row["name"]]),
                row["exclusions"],
            )
            self.assertIn(subject.GLOBAL_EXCLUSION, row["exclusions"])

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
        inventories = self.inventories()
        behavior_indexes = {
            label: index
            for index, label in enumerate(subject.TEST_BINDINGS.values())
        }
        document = subject._generate_assignment_document(
            inventories,
            behavior_indexes,
        )
        self.assertEqual(6, document["summary"]["tests"])
        self.assertEqual(84, document["summary"]["assertions"])
        self.assertEqual(37, document["summary"]["direct_oracle"])
        self.assertEqual(47, document["summary"]["setup_or_precondition"])
        self.assertEqual(
            84,
            sum(
                len(row["assertion_assignments"])
                for row in document["assignments"]
            ),
        )


if __name__ == "__main__":
    unittest.main()
