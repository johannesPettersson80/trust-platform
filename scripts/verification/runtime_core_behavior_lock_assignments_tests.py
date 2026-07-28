from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.runtime_core_behavior_lock_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "runtime-core-behavior-lock-4-assertions.json"
)


class RuntimeCoreBehaviorLockAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the runtime-core behavior-lock assignment generator is missing",
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

    def test_reviewed_batch_is_exactly_four_tests_and_65_assertions(
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
            65,
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
                (subject.ENCODER, subject.ENCODED_LOAD): 2,
                (subject.METADATA, subject.APPLY_BYTES): 1,
                (subject.METADATA, subject.CLEAN_VM_LOAD): 6,
                (subject.VM_VALUES, subject.TYPED_VM_CYCLE): 30,
                (subject.RESTART, subject.WARM_STORAGE): 3,
                (subject.RESTART, subject.COLD_STORAGE): 2,
                (subject.IO_CYCLE, subject.CYCLE_SNAPSHOT): 12,
                (subject.IO_CYCLE, subject.ALL_READS_BEFORE_WRITES): 9,
            },
            counts,
        )
        self.assertEqual(
            Counter({"oracle_bound": 42, "setup_or_precondition": 23}),
            dispositions,
        )

    def test_healthy_fault_state_replacement_is_direct_and_old_parity_fails(
        self,
    ) -> None:
        subject = self.subject()
        for assertion_id in (
            "ASSERT_3EF2C434B8D6A194C9DF715E",
            "ASSERT_F5DAFBB04BAD561E579AD2E3",
        ):
            self.assertEqual(
                (
                    subject.VM_VALUES,
                    subject.TYPED_VM_CYCLE,
                    "oracle_bound",
                ),
                subject._route(
                    test_name=subject.VM_TEST,
                    assertion_id=assertion_id,
                ),
            )
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._route(
                test_name=subject.VM_TEST,
                assertion_id="ASSERT_4A308AD1FC7F091254697002",
            )

    def test_restart_baseline_and_transitions_use_restart_authority(
        self,
    ) -> None:
        subject = self.subject()
        expected = {
            "ASSERT_98A87FEB323DB10D35C2BCC0": subject.WARM_STORAGE,
            "ASSERT_EF205D9B11BB11AD53EE33BB": subject.WARM_STORAGE,
            "ASSERT_ADC7046985D7877B1D7FF935": subject.WARM_STORAGE,
            "ASSERT_48E42FA84ADB897E419968AD": subject.COLD_STORAGE,
            "ASSERT_108AB550896C334082B23172": subject.COLD_STORAGE,
        }
        for assertion_id, behavior in expected.items():
            with self.subTest(assertion_id=assertion_id):
                self.assertEqual(
                    (subject.RESTART, behavior, "oracle_bound"),
                    subject._route(
                        test_name=subject.VM_TEST,
                        assertion_id=assertion_id,
                    ),
                )

    def test_denominator_routes_are_post_map_idempotent(self) -> None:
        subject = self.subject()
        discovery_id = "DISC_268F47FD499F57BFF436"
        base = {
            "discovery_id": discovery_id,
            "path": subject.PATH,
            "name": subject.STABLE_LOAD_TEST,
        }
        reviewed = {
            **base,
            "disposition": "reviewed_nonmapping",
            "reason_code": "no_reviewed_spec_or_invariant_binding",
        }
        mapped = {
            **base,
            "disposition": "catalog_mapped",
            "catalog_test_id": "TEST_AUTO_268F47FD499F57BFF436",
        }
        self.assertTrue(
            subject._denominator_row_is_live(
                reviewed,
                discovery_id=discovery_id,
                expected_name=subject.STABLE_LOAD_TEST,
            )
        )
        self.assertTrue(
            subject._denominator_row_is_live(
                mapped,
                discovery_id=discovery_id,
                expected_name=subject.STABLE_LOAD_TEST,
            )
        )
        self.assertFalse(
            subject._denominator_row_is_live(
                {**mapped, "catalog_test_id": "TEST_WRONG"},
                discovery_id=discovery_id,
                expected_name=subject.STABLE_LOAD_TEST,
            )
        )

    def test_semantic_exclusions_bound_each_vertical_claim(self) -> None:
        subject = self.subject()
        combined = " ".join(
            exclusion
            for exclusions in subject.TEST_EXCLUSIONS.values()
            for exclusion in exclusions
        ).lower()
        for required in (
            "canonical byte",
            "corrupt",
            "host evaluator",
            "iec-wide",
            "restart time",
            "multi-driver",
            "driver fault",
            "snapshot",
        ):
            self.assertIn(required, combined)

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._route(
                test_name=subject.STABLE_LOAD_TEST,
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
        self.assertEqual(65, document["summary"]["assertions"])
        self.assertEqual(42, document["summary"]["direct_oracle"])
        self.assertEqual(23, document["summary"]["setup_or_precondition"])
        self.assertEqual(0, document["summary"]["non_oracle_observation"])
        self.assertEqual(
            65,
            sum(
                len(row["assertion_assignments"])
                for row in document["assignments"]
            ),
        )
        self.assertEqual(
            23,
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
        self.assertEqual(7, indexes[(subject.ENCODER, subject.ENCODED_LOAD)])
        self.assertEqual(2, indexes[(subject.METADATA, subject.APPLY_BYTES)])
        self.assertEqual(3, indexes[(subject.METADATA, subject.CLEAN_VM_LOAD)])
        self.assertEqual(2, indexes[(subject.VM_VALUES, subject.TYPED_VM_CYCLE)])
        self.assertEqual(0, indexes[(subject.RESTART, subject.COLD_STORAGE)])
        self.assertEqual(1, indexes[(subject.RESTART, subject.WARM_STORAGE)])
        self.assertEqual(0, indexes[(subject.IO_CYCLE, subject.CYCLE_SNAPSHOT)])
        self.assertEqual(
            2,
            indexes[(subject.IO_CYCLE, subject.ALL_READS_BEFORE_WRITES)],
        )


if __name__ == "__main__":
    unittest.main()
