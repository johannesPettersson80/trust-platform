from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.ci_cicd_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "ci-actionable-assertions.json"
)


class CiCicdAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the reviewed CI/CD bulk assignment generator is missing",
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

    def test_reviewed_batch_is_exactly_three_tests_and_61_assertions(
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
            61,
            sum(len(row["assertions"]) for row in inventories.values()),
        )

    def test_timing_probe_identities_are_never_selected(self) -> None:
        subject = self.subject()
        source = json.loads(INVENTORY.read_text())["inventories"]
        self.assertTrue(set(subject.EXCLUDED_TIMING_IDENTITIES) <= set(source))
        selected = self.selected_document()["inventories"]
        self.assertTrue(set(subject.EXCLUDED_TIMING_IDENTITIES).isdisjoint(selected))

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
                    subject.INVARIANT,
                    subject.JSON_SUCCESS,
                ): 32,
                (
                    subject.INVARIANT,
                    subject.GREEN_JUNIT,
                ): 13,
                (
                    subject.INVARIANT,
                    subject.BROKEN_JUNIT,
                ): 16,
            },
            counts,
        )
        self.assertEqual(
            Counter({"setup_or_precondition": 33, "oracle_bound": 28}),
            dispositions,
        )

    def test_semantic_exclusions_forbid_performance_overclaim(self) -> None:
        subject = self.subject()
        for exclusions in subject.TEST_EXCLUSIONS.values():
            text = " ".join(exclusions).lower()
            self.assertIn("bounded", text)
            self.assertIn("timing", text)
            self.assertIn("performance", text)
            self.assertIn("clean-install", text)

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
        document = subject._generate_assignment_document(
            self.inventories(),
            {
                subject.JSON_SUCCESS: 0,
                subject.GREEN_JUNIT: 1,
                subject.BROKEN_JUNIT: 2,
            },
        )
        self.assertEqual(3, document["summary"]["tests"])
        self.assertEqual(61, document["summary"]["assertions"])
        self.assertEqual(28, document["summary"]["direct_oracle"])
        self.assertEqual(33, document["summary"]["setup_or_precondition"])
        self.assertEqual(0, document["summary"]["non_oracle_observation"])
        self.assertEqual(
            61,
            sum(
                len(row["assertion_assignments"])
                for row in document["assignments"]
            ),
        )
        self.assertEqual(
            33,
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

    def test_behavior_lookup_matches_reviewed_invariant(self) -> None:
        subject = self.subject()
        self.assertEqual(
            {
                subject.JSON_SUCCESS: 0,
                subject.GREEN_JUNIT: 1,
                subject.BROKEN_JUNIT: 2,
            },
            subject._behavior_indexes(ROOT),
        )


if __name__ == "__main__":
    unittest.main()
