from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.scheduler_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "scheduler-actionable-assertions.json"
)


class SchedulerAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the reviewed scheduler bulk assignment generator is missing",
        )
        return importlib.import_module(MODULE)

    def inventories(self):
        subject = self.subject()
        document = json.loads(INVENTORY.read_text())
        return subject._canonical_inventories(ROOT, document)

    def test_reviewed_batch_is_exactly_five_tests_and_42_assertions(self) -> None:
        subject = self.subject()
        inventories = self.inventories()
        self.assertEqual(5, len(subject.TEST_EXCLUSIONS))
        self.assertEqual(
            set(subject.TEST_EXCLUSIONS),
            {row["test_name"] for row in inventories.values()},
        )
        self.assertEqual(
            42,
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
                (subject.READINESS_INVARIANT, subject.MULTIPLE_DUE): 2,
                (subject.E2E_INVARIANT, subject.PERIODIC_EVENT): 10,
                (subject.E2E_INVARIANT, subject.BACKWARD_CLOCK): 11,
                (subject.E2E_INVARIANT, subject.FORWARD_CLOCK): 7,
                (subject.E2E_INVARIANT, subject.FAULT_LATCH): 7,
                (subject.E2E_INVARIANT, subject.TRACE_REPRODUCIBILITY): 5,
            },
            counts,
        )
        self.assertEqual(
            Counter(
                {
                    "oracle_bound": 27,
                    "setup_or_precondition": 9,
                    "non_oracle_observation": 6,
                }
            ),
            dispositions,
        )

    def test_only_overrun_counts_cross_route_to_multiple_due(self) -> None:
        subject = self.subject()
        cross_routed = {
            assertion_id
            for routes in subject.REVIEWED_ROUTES.values()
            for assertion_id, route in routes.items()
            if route[:2]
            == (subject.READINESS_INVARIANT, subject.MULTIPLE_DUE)
        }
        self.assertEqual(
            {
                "ASSERT_044CC0B303AC001971DECD7B",
                "ASSERT_CFDAE48447F0702C0D4EEFF3",
            },
            cross_routed,
        )

    def test_forward_jump_is_not_labeled_as_backward_clock_proof(self) -> None:
        subject = self.subject()
        routes = subject.REVIEWED_ROUTES[subject.RESUMED_TEST]
        self.assertTrue(
            all(
                route[1] in {subject.FORWARD_CLOCK, subject.MULTIPLE_DUE}
                for route in routes.values()
            )
        )
        self.assertTrue(
            all(route[1] != subject.BACKWARD_CLOCK for route in routes.values())
        )

    def test_unreachable_wider_counter_arms_are_non_oracles(self) -> None:
        subject = self.subject()
        for assertion_id in (
            "ASSERT_7B2E2FD046F6927265890213",
            "ASSERT_CEA06C2FF6156619A49DA18F",
            "ASSERT_7EA16F5EA854C96B06117431",
            "ASSERT_87481090FD103AE14118A6D4",
            "ASSERT_A128031DE0C28AC2536DADFD",
            "ASSERT_55A903BEE41C0DEEA412BB6C",
        ):
            routes = [
                route
                for assertions in subject.REVIEWED_ROUTES.values()
                for current_id, route in assertions.items()
                if current_id == assertion_id
            ]
            self.assertEqual(1, len(routes))
            self.assertEqual("non_oracle_observation", routes[0][2])

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
        required = {
            route[:2]
            for routes in subject.REVIEWED_ROUTES.values()
            for route in routes.values()
        }
        indexes = {
            route: index for index, route in enumerate(sorted(required))
        }
        document = subject._generate_assignment_document(
            self.inventories(),
            indexes,
        )
        self.assertEqual(5, document["summary"]["tests"])
        self.assertEqual(42, document["summary"]["assertions"])
        self.assertEqual(27, document["summary"]["direct_oracle"])
        self.assertEqual(9, document["summary"]["setup_or_precondition"])
        self.assertEqual(6, document["summary"]["non_oracle_observation"])


if __name__ == "__main__":
    unittest.main()
