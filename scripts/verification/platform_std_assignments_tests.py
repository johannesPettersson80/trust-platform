from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.platform_std_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/platform-std-4-assertions.json"
)


class PlatformStdAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the reviewed platform std assignment generator is missing",
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

    def test_reviewed_batch_is_exactly_four_tests_and_13_assertions(
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
            13,
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
                (subject.INVARIANT, subject.STDCLOCK): 1,
                (subject.INVARIANT, subject.NON_PACING_TICK): 3,
                (subject.INVARIANT, subject.TIME_LOGICAL): 3,
                (subject.INVARIANT, subject.INJECTED_READINESS): 6,
            },
            counts,
        )
        self.assertEqual(
            Counter({"setup_or_precondition": 7, "oracle_bound": 6}),
            dispositions,
        )

    def test_exact_direct_oracles_are_locked(self) -> None:
        subject = self.subject()
        direct_ids = {
            assertion_id
            for routes in subject.REVIEWED_ROUTES.values()
            for assertion_id, route in routes.items()
            if route[2] == "oracle_bound"
        }
        self.assertEqual(
            {
                "ASSERT_6E5D52488F89670FFCC0E798",
                "ASSERT_3B62C2259A4B19A57704214F",
                "ASSERT_7D87EF4A72CF70CEE1C474D8",
                "ASSERT_EA17E7989E59E9695EF1C9CD",
                "ASSERT_6CCB23303B92847FDD391982",
                "ASSERT_9EFB5A6F7AB986FDA7C3D9D2",
            },
            direct_ids,
        )

    def test_semantic_exclusions_bound_claims_and_exclude_current_dt(
        self,
    ) -> None:
        subject = self.subject()
        combined = " ".join(
            exclusion
            for exclusions in subject.TEST_EXCLUSIONS.values()
            for exclusion in exclusions
        ).lower()
        for required in (
            "current_dt",
            "strict progress",
            "resolution",
            "spawned resource loop",
            "all tests",
            "wall-clock",
            "timezone",
        ):
            self.assertIn(required, combined)

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._route(
                test_name=subject.MONOTONIC_TEST,
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
                subject.STDCLOCK: 0,
                subject.NON_PACING_TICK: 1,
                subject.TIME_LOGICAL: 2,
                subject.INJECTED_READINESS: 3,
            },
        )
        self.assertEqual(4, document["summary"]["tests"])
        self.assertEqual(13, document["summary"]["assertions"])
        self.assertEqual(6, document["summary"]["direct_oracle"])
        self.assertEqual(7, document["summary"]["setup_or_precondition"])
        self.assertEqual(0, document["summary"]["non_oracle_observation"])
        self.assertEqual(
            13,
            sum(
                len(row["assertion_assignments"])
                for row in document["assignments"]
            ),
        )
        self.assertEqual(
            7,
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

    def test_behavior_lookup_matches_reviewed_invariant_order(self) -> None:
        subject = self.subject()
        self.assertEqual(
            {
                subject.STDCLOCK: 0,
                subject.NON_PACING_TICK: 1,
                subject.TIME_LOGICAL: 2,
                subject.INJECTED_READINESS: 3,
            },
            subject._behavior_indexes(ROOT),
        )


if __name__ == "__main__":
    unittest.main()
