from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.restart_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "restart-actionable-assertions.json"
)


class RestartAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the reviewed restart bulk assignment generator is missing",
        )
        return importlib.import_module(MODULE)

    def inventories(self):
        subject = self.subject()
        document = json.loads(INVENTORY.read_text())
        return subject._canonical_inventories(ROOT, document)

    def test_reviewed_batch_is_exactly_five_tests_and_44_assertions(self) -> None:
        subject = self.subject()
        inventories = self.inventories()
        self.assertEqual(5, len(subject.TEST_EXCLUSIONS))
        self.assertEqual(
            set(subject.TEST_EXCLUSIONS),
            {row["test_name"] for row in inventories.values()},
        )
        self.assertEqual(
            44,
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
                (
                    subject.RESTART_INVARIANT,
                    subject.RESTART_FAILURE,
                ): 11,
                (
                    subject.MIGRATION_INVARIANT,
                    subject.RECURSIVE_IDENTITY,
                ): 25,
                (
                    subject.MIGRATION_INVARIANT,
                    subject.NESTED_REJECTION,
                ): 8,
            },
            counts,
        )
        self.assertEqual(
            Counter({"oracle_bound": 29, "setup_or_precondition": 15}),
            dispositions,
        )

    def test_array_test_is_split_between_success_and_rejection(self) -> None:
        subject = self.subject()
        name = (
            "apply_retain_snapshot_canonicalizes_array_of_struct_"
            "and_rejects_bad_element"
        )
        self.assertEqual(
            (
                subject.MIGRATION_INVARIANT,
                subject.RECURSIVE_IDENTITY,
                "oracle_bound",
            ),
            subject._route(
                test_name=name,
                assertion_id="ASSERT_432801090882DC9FDA8982A6",
            ),
        )
        self.assertEqual(
            (
                subject.MIGRATION_INVARIANT,
                subject.NESTED_REJECTION,
                "oracle_bound",
            ),
            subject._route(
                test_name=name,
                assertion_id="ASSERT_35E1ACFA94D01511FFEB8589",
            ),
        )

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._route(
                test_name=next(iter(subject.TEST_EXCLUSIONS)),
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
        indexes = {
            (subject.RESTART_INVARIANT, subject.RESTART_FAILURE): 4,
            (subject.MIGRATION_INVARIANT, subject.RECURSIVE_IDENTITY): 5,
            (subject.MIGRATION_INVARIANT, subject.NESTED_REJECTION): 6,
        }
        document = subject._generate_assignment_document(
            self.inventories(),
            indexes,
        )
        self.assertEqual(5, document["summary"]["tests"])
        self.assertEqual(44, document["summary"]["assertions"])
        self.assertEqual(29, document["summary"]["direct_oracle"])
        self.assertEqual(15, document["summary"]["setup_or_precondition"])
        self.assertEqual(
            44,
            sum(
                len(row["assertion_assignments"])
                for row in document["assignments"]
            ),
        )


if __name__ == "__main__":
    unittest.main()
