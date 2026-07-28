from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.syntax_kind_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "syntax-actionable-assertions.json"
)


class SyntaxKindAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the reviewed syntax-kind bulk assignment generator is missing",
        )
        return importlib.import_module(MODULE)

    def inventories(self):
        subject = self.subject()
        document = json.loads(INVENTORY.read_text())
        return subject._canonical_inventories(ROOT, document)

    def test_reviewed_batch_is_exactly_five_tests_and_30_assertions(self) -> None:
        subject = self.subject()
        inventories = self.inventories()
        self.assertEqual(5, len(subject.TEST_EXCLUSIONS))
        self.assertEqual(
            set(subject.TEST_EXCLUSIONS),
            {row["test_name"] for row in inventories.values()},
        )
        self.assertEqual(
            30,
            sum(len(row["assertions"]) for row in inventories.values()),
        )

    def test_all_assertions_have_exact_reviewed_behaviors(self) -> None:
        subject = self.subject()
        counts: Counter[str] = Counter()
        dispositions: Counter[str] = Counter()
        for raw in self.inventories().values():
            for assertion in raw["assertions"]:
                invariant, behavior, disposition = subject._route(
                    test_name=raw["test_name"],
                    assertion_id=assertion["assertion_id"],
                )
                self.assertEqual(subject.INVARIANT, invariant)
                counts[behavior] += 1
                dispositions[disposition] += 1
        self.assertEqual(
            {
                subject.TOKEN_CONVERSION: 3,
                subject.TRIVIA: 5,
                subject.TOKEN_NODE: 6,
                subject.GENERIC_EXPRESSION: 5,
                subject.AGGREGATE_INITIALIZER: 2,
                subject.INITIALIZER_RHS: 3,
                subject.TRIVIA_EXCLUSION: 3,
                subject.POU_SEQUENCE: 3,
            },
            counts,
        )
        self.assertEqual(Counter({"oracle_bound": 30}), dispositions)

    def test_initializer_assertions_are_split_four_ways(self) -> None:
        subject = self.subject()
        routes = subject.REVIEWED_ROUTES[
            "test_initializer_classifier_sets"
        ]
        counts = Counter(route[1] for route in routes.values())
        self.assertEqual(
            {
                subject.GENERIC_EXPRESSION: 5,
                subject.AGGREGATE_INITIALIZER: 2,
                subject.INITIALIZER_RHS: 3,
                subject.TRIVIA_EXCLUSION: 3,
            },
            counts,
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
        behaviors = {
            route[1]
            for routes in subject.REVIEWED_ROUTES.values()
            for route in routes.values()
        }
        indexes = {behavior: index for index, behavior in enumerate(behaviors)}
        document = subject._generate_assignment_document(
            self.inventories(),
            indexes,
        )
        self.assertEqual(5, document["summary"]["tests"])
        self.assertEqual(30, document["summary"]["assertions"])
        self.assertEqual(30, document["summary"]["direct_oracle"])
        self.assertEqual(0, document["summary"]["setup_or_precondition"])
        self.assertEqual(0, document["summary"]["non_oracle_observation"])


if __name__ == "__main__":
    unittest.main()
