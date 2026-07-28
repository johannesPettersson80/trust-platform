from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.e2e_multifile_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "e2e-multifile-4-assertions.json"
)


class E2eMultifileAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the reviewed e2e multifile assignment generator is missing",
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

    def test_reviewed_batch_is_exactly_four_tests_and_14_assertions(
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
            14,
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
                (subject.INVARIANT, subject.USING_FUNCTION): 2,
                (subject.INVARIANT, subject.NAMESPACED_PROGRAM): 3,
                (subject.INVARIANT, subject.SIBLING_POU_GRAPH): 7,
                (subject.INVARIANT, subject.DUPLICATE_PROGRAM): 2,
            },
            counts,
        )
        self.assertEqual(
            Counter({"oracle_bound": 11, "setup_or_precondition": 3}),
            dispositions,
        )

    def test_only_successful_construction_unwraps_are_setup(self) -> None:
        subject = self.subject()
        setup_ids = {
            assertion_id
            for routes in subject.REVIEWED_ROUTES.values()
            for assertion_id, route in routes.items()
            if route[2] == "setup_or_precondition"
        }
        self.assertEqual(
            {
                "ASSERT_97DCB126F235C08D5E8C9AC8",
                "ASSERT_11D228DA2C09AEACA6896ED4",
                "ASSERT_54C67B88D58A19832A4BA2F2",
            },
            setup_ids,
        )
        self.assertEqual(
            "oracle_bound",
            subject._route(
                test_name=subject.DUPLICATE_TEST,
                assertion_id="ASSERT_319868A68361139583B7093E",
            )[2],
        )

    def test_semantic_exclusions_keep_each_claim_bounded(self) -> None:
        subject = self.subject()
        combined = " ".join(
            exclusion
            for exclusions in subject.TEST_EXCLUSIONS.values()
            for exclusion in exclusions
        ).lower()
        for required in (
            "nested",
            "source-order",
            "production",
            "interface dispatch",
            "class method",
            "same-file",
            "error code",
        ):
            self.assertIn(required, combined)

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._route(
                test_name=subject.NAMESPACE_TEST,
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
                subject.USING_FUNCTION: 0,
                subject.NAMESPACED_PROGRAM: 1,
                subject.SIBLING_POU_GRAPH: 2,
                subject.DUPLICATE_PROGRAM: 3,
            },
        )
        self.assertEqual(4, document["summary"]["tests"])
        self.assertEqual(14, document["summary"]["assertions"])
        self.assertEqual(11, document["summary"]["direct_oracle"])
        self.assertEqual(3, document["summary"]["setup_or_precondition"])
        self.assertEqual(0, document["summary"]["non_oracle_observation"])
        self.assertEqual(
            14,
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

    def test_behavior_lookup_matches_reviewed_invariant(self) -> None:
        subject = self.subject()
        self.assertEqual(
            {
                subject.USING_FUNCTION: 0,
                subject.NAMESPACED_PROGRAM: 1,
                subject.SIBLING_POU_GRAPH: 2,
                subject.DUPLICATE_PROGRAM: 3,
            },
            subject._behavior_indexes(ROOT),
        )


if __name__ == "__main__":
    unittest.main()
