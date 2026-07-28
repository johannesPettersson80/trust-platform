from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.runtime_reliability_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "runtime-reliability-4-assertions.json"
)


class RuntimeReliabilityAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the runtime-reliability assignment generator is missing",
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
                (subject.HARNESS_IO, subject.DIRECT_BOUND_IO): 7,
                (subject.RESTART_TIME, subject.WARM_TIME): 2,
                (subject.RETAIN_CODEC, subject.UNSAVED_DROP): 3,
                (subject.DEADLINE, subject.SPAWNED_WATCHDOG): 5,
                (subject.RETAIN_CODEC, subject.PROGRAM_WARM): 4,
                (subject.SAFE_RESTART, subject.COLD_SKIPS_LOAD): 2,
            },
            counts,
        )
        self.assertEqual(
            Counter({"oracle_bound": 17, "setup_or_precondition": 6}),
            dispositions,
        )

    def test_setup_routes_are_exact_and_saved_snapshot_is_setup(self) -> None:
        subject = self.subject()
        expected = {
            (
                subject.STARTUP_IO_TEST,
                "ASSERT_762D6440DC5230262D0DCED5",
            ),
            (
                subject.UNSAVED_DROP_TEST,
                "ASSERT_8EC5134DC034DFA4707A4869",
            ),
            (
                subject.WATCHDOG_TEST,
                "ASSERT_40D4D15A6F0A003E5F3D3AC4",
            ),
            (
                subject.WATCHDOG_TEST,
                "ASSERT_E52B014D66F48216BFB65463",
            ),
            (
                subject.RETAIN_ROUNDTRIP_TEST,
                "ASSERT_F18708F5B196486556E99305",
            ),
            (
                subject.RETAIN_ROUNDTRIP_TEST,
                "ASSERT_D221D1BF992D4D373641D979",
            ),
        }
        actual = {
            (test_name, assertion_id)
            for test_name, routes in subject.REVIEWED_ROUTES.items()
            for assertion_id, route in routes.items()
            if route[2] == "setup_or_precondition"
        }
        self.assertEqual(expected, actual)

    def test_denominator_routes_are_post_map_idempotent(self) -> None:
        subject = self.subject()
        discovery_id = "DISC_EE4014B513F4E29CDCC3"
        base = {
            "discovery_id": discovery_id,
            "path": subject.PATH,
            "name": subject.RETAIN_ROUNDTRIP_TEST,
        }
        reviewed = {
            **base,
            "disposition": "reviewed_nonmapping",
            "reason_code": "no_reviewed_spec_or_invariant_binding",
        }
        mapped = {
            **base,
            "disposition": "catalog_mapped",
            "catalog_test_id": "TEST_AUTO_EE4014B513F4E29CDCC3",
        }
        self.assertTrue(
            subject._denominator_row_is_live(
                reviewed,
                discovery_id=discovery_id,
                expected_name=subject.RETAIN_ROUNDTRIP_TEST,
            )
        )
        self.assertTrue(
            subject._denominator_row_is_live(
                mapped,
                discovery_id=discovery_id,
                expected_name=subject.RETAIN_ROUNDTRIP_TEST,
            )
        )
        self.assertFalse(
            subject._denominator_row_is_live(
                {**mapped, "catalog_test_id": "TEST_WRONG"},
                discovery_id=discovery_id,
                expected_name=subject.RETAIN_ROUNDTRIP_TEST,
            )
        )

    def test_semantic_exclusions_bound_each_claim(self) -> None:
        subject = self.subject()
        combined = " ".join(
            exclusion
            for exclusions in subject.TEST_EXCLUSIONS.values()
            for exclusion in exclusions
        ).lower()
        for required in (
            "physical i/o",
            "after restart",
            "physical power loss",
            "durability",
            "100 ms",
            "latency guarantee",
            "stepclock",
            "safe-state",
            "pending-restart",
            "arbitrary retain",
            "iec-wide",
        ):
            self.assertIn(required, combined)

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._route(
                test_name=subject.STARTUP_IO_TEST,
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
        self.assertEqual(17, document["summary"]["direct_oracle"])
        self.assertEqual(6, document["summary"]["setup_or_precondition"])
        self.assertEqual(0, document["summary"]["non_oracle_observation"])
        self.assertEqual(
            23,
            sum(
                len(row["assertion_assignments"])
                for row in document["assignments"]
            ),
        )
        self.assertEqual(
            6,
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

    def test_behavior_lookup_resolves_every_reviewed_selector(self) -> None:
        subject = self.subject()
        indexes = subject._behavior_indexes(ROOT)
        self.assertEqual(
            {
                (subject.HARNESS_IO, subject.DIRECT_BOUND_IO),
                (subject.RESTART_TIME, subject.WARM_TIME),
                (subject.RETAIN_CODEC, subject.UNSAVED_DROP),
                (subject.DEADLINE, subject.SPAWNED_WATCHDOG),
                (subject.RETAIN_CODEC, subject.PROGRAM_WARM),
                (subject.SAFE_RESTART, subject.COLD_SKIPS_LOAD),
            },
            set(indexes),
        )
        self.assertTrue(all(index >= 0 for index in indexes.values()))


if __name__ == "__main__":
    unittest.main()
