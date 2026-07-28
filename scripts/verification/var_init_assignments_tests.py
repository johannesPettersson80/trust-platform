from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.var_init_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "var-init-4-assertions.json"
)


class VarInitAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the var-init assignment generator is missing",
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

    def test_reviewed_batch_is_exactly_four_tests_and_16_assertions(
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
            16,
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
                (subject.COMPILER, subject.CONSTANT_INITIALIZER): 5,
                (subject.COMPILER, subject.MUTABLE_REJECTED): 2,
                (subject.VM, subject.VALUE_MATERIALIZATION): 9,
            },
            counts,
        )
        self.assertEqual(
            Counter({"oracle_bound": 13, "setup_or_precondition": 3}),
            dispositions,
        )

    def test_only_array_harness_construction_is_setup(self) -> None:
        subject = self.subject()
        expected = {
            (
                subject.REPETITION_TEST,
                "ASSERT_2AC73780C0E98CC95A82EE84",
            ),
            (
                subject.ARRAY_TEST,
                "ASSERT_F4B72BCB1943D6B081E2E966",
            ),
            (
                subject.PARTIAL_TEST,
                "ASSERT_805005BA08A494380668F538",
            ),
        }
        actual = {
            (test_name, assertion_id)
            for test_name, routes in subject.REVIEWED_ROUTES.items()
            for assertion_id, route in routes.items()
            if route[2] == "setup_or_precondition"
        }
        self.assertEqual(expected, actual)

    def test_compiler_acceptance_and_rejection_are_direct(self) -> None:
        subject = self.subject()
        expected = (
            (
                "ASSERT_7593023EB0CD59B863CB6D14",
                subject.CONSTANT_INITIALIZER,
            ),
            (
                "ASSERT_3F2139596B6B54151698D5B0",
                subject.MUTABLE_REJECTED,
            ),
            (
                "ASSERT_9D4D7EEF6E3A67AEE9A608B0",
                subject.MUTABLE_REJECTED,
            ),
            (
                "ASSERT_28C10143C6A4841D4E0E63BB",
                subject.CONSTANT_INITIALIZER,
            ),
            (
                "ASSERT_5C616E1BF7D5174B512955E8",
                subject.CONSTANT_INITIALIZER,
            ),
            (
                "ASSERT_4B2AC64D1E5D714358BAE912",
                subject.CONSTANT_INITIALIZER,
            ),
            (
                "ASSERT_72B6840D17E62353275AC0CA",
                subject.CONSTANT_INITIALIZER,
            ),
        )
        for assertion_id, behavior in expected:
            with self.subTest(assertion_id=assertion_id):
                self.assertEqual(
                    (subject.COMPILER, behavior, "oracle_bound"),
                    subject._route(
                        test_name=subject.IEC_TABLE14_TEST,
                        assertion_id=assertion_id,
                    ),
                )

    def test_denominator_routes_are_post_map_idempotent(self) -> None:
        subject = self.subject()
        discovery_id = "DISC_76C36D65723E0778AA23"
        base = {
            "discovery_id": discovery_id,
            "path": subject.PATH,
            "name": subject.IEC_TABLE14_TEST,
        }
        reviewed = {
            **base,
            "disposition": "reviewed_nonmapping",
            "reason_code": "no_reviewed_spec_or_invariant_binding",
        }
        mapped = {
            **base,
            "disposition": "catalog_mapped",
            "catalog_test_id": "TEST_AUTO_76C36D65723E0778AA23",
        }
        self.assertTrue(
            subject._denominator_row_is_live(
                reviewed,
                discovery_id=discovery_id,
                expected_name=subject.IEC_TABLE14_TEST,
            )
        )
        self.assertTrue(
            subject._denominator_row_is_live(
                mapped,
                discovery_id=discovery_id,
                expected_name=subject.IEC_TABLE14_TEST,
            )
        )
        self.assertFalse(
            subject._denominator_row_is_live(
                {**mapped, "catalog_test_id": "TEST_WRONG"},
                discovery_id=discovery_id,
                expected_name=subject.IEC_TABLE14_TEST,
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
            "multidimensional",
            "nested repetition",
            "excess element",
            "warning",
            "mutable reference",
            "constant expression",
            "arbitrary type",
            "iec-wide",
        ):
            self.assertIn(required, combined)

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._route(
                test_name=subject.IEC_TABLE14_TEST,
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

    def test_assignment_document_is_exhaustive_and_has_no_nonoracle(
        self,
    ) -> None:
        subject = self.subject()
        document = subject._generate_assignment_document(
            self.inventories(),
            subject._behavior_indexes(ROOT),
        )
        self.assertEqual(4, document["summary"]["tests"])
        self.assertEqual(16, document["summary"]["assertions"])
        self.assertEqual(13, document["summary"]["direct_oracle"])
        self.assertEqual(3, document["summary"]["setup_or_precondition"])
        self.assertEqual(0, document["summary"]["non_oracle_observation"])
        self.assertEqual(
            16,
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

    def test_behavior_lookup_resolves_every_reviewed_selector(self) -> None:
        subject = self.subject()
        indexes = subject._behavior_indexes(ROOT)
        self.assertEqual(
            {
                (subject.COMPILER, subject.CONSTANT_INITIALIZER),
                (subject.COMPILER, subject.MUTABLE_REJECTED),
                (subject.VM, subject.VALUE_MATERIALIZATION),
            },
            set(indexes),
        )
        self.assertTrue(all(index >= 0 for index in indexes.values()))


if __name__ == "__main__":
    unittest.main()
