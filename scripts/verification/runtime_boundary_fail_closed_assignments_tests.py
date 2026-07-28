from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.runtime_boundary_fail_closed_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "runtime-boundary-12-assertions.json"
)


class RuntimeBoundaryFailClosedAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the runtime boundary fail-closed assignment generator is missing",
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

    def test_reviewed_batch_is_exactly_12_tests_and_45_assertions(
        self,
    ) -> None:
        subject = self.subject()
        inventories = self.inventories()
        self.assertEqual(
            subject.REVIEWED_IDENTITIES,
            {
                discovery_id: (row["path"], row["test_name"])
                for discovery_id, row in inventories.items()
            },
        )
        self.assertEqual(
            45,
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
                (subject.HARNESS, subject.STRUCTURED_PATH): 4,
                (subject.HARNESS, subject.DECLARED_NULL): 2,
                (subject.HARNESS, subject.SYMBOL_REJECTION): 11,
                (subject.HARNESS, subject.TYPO_REJECTION): 10,
                (subject.HOST_POU, subject.INTERFACE_NULL): 4,
                (subject.STORAGE, subject.QUEUED_DEBUG): 7,
                (subject.CORE_VALUES, subject.REF_PATH): 5,
                (subject.CORE_VALUES, subject.ARRAY_OFFSET): 1,
                (subject.VM_REFERENCE, subject.INVALID_REFERENCE): 1,
            },
            counts,
        )
        self.assertEqual(
            Counter({"oracle_bound": 36, "setup_or_precondition": 9}),
            dispositions,
        )

    def test_only_reviewed_construction_and_resolution_steps_are_setup(
        self,
    ) -> None:
        subject = self.subject()
        expected = {
            (subject.AMBIGUOUS_TEST, "ASSERT_31EC3FDD3562ABE26491630C"),
            (subject.UNKNOWN_TEST, "ASSERT_CEB87F9B2E0E53113939441A"),
            (subject.STRUCTURED_TEST, "ASSERT_EF7859FD9A23A4339810BC73"),
            (subject.NULL_REF_TEST, "ASSERT_6FFEC6C7D5403D66AFB5062C"),
            (subject.BIND_TYPO_TEST, "ASSERT_10AA42029DBA3455A540C6CF"),
            (subject.INPUT_TYPO_TEST, "ASSERT_EFABD97DC41ABA868AAE4CAC"),
            (subject.INTERFACE_TEST, "ASSERT_959F44AFE239BA2EA598DF64"),
            (subject.INTERFACE_TEST, "ASSERT_4848D847DAEBEC153C9B0B69"),
            (subject.INTERFACE_TEST, "ASSERT_73A502D86743EA6AE7C415D6"),
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
        discovery_id = "DISC_06771ADC38B4CC93B5E3"
        path, name = subject.REVIEWED_IDENTITIES[discovery_id]
        base = {
            "discovery_id": discovery_id,
            "path": path,
            "name": name,
        }
        reviewed = {
            **base,
            "disposition": "reviewed_nonmapping",
            "reason_code": "no_reviewed_spec_or_invariant_binding",
        }
        mapped = {
            **base,
            "disposition": "catalog_mapped",
            "catalog_test_id": "TEST_AUTO_06771ADC38B4CC93B5E3",
        }
        self.assertTrue(
            subject._denominator_row_is_live(
                reviewed,
                discovery_id=discovery_id,
                expected_path=path,
                expected_name=name,
            )
        )
        self.assertTrue(
            subject._denominator_row_is_live(
                mapped,
                discovery_id=discovery_id,
                expected_path=path,
                expected_name=name,
            )
        )
        self.assertFalse(
            subject._denominator_row_is_live(
                {**mapped, "catalog_test_id": "TEST_WRONG"},
                discovery_id=discovery_id,
                expected_path=path,
                expected_name=name,
            )
        )

    def test_semantic_exclusions_bound_every_authority_group(self) -> None:
        subject = self.subject()
        combined = " ".join(
            exclusion
            for exclusions in subject.TEST_EXCLUSIONS.values()
            for exclusion in exclusions
        ).lower()
        for required in (
            "candidate ordering",
            "case-insensitive",
            "arbitrary structured text",
            "successful mutation",
            "i/o address grammar",
            "debug authorization",
            "interface dispatch",
            "multi-dimensional array",
            "platform-width",
            "vm execution parity",
            "iec-wide",
        ):
            self.assertIn(required, combined)

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._route(
                test_name=subject.AMBIGUOUS_TEST,
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
        self.assertEqual(12, document["summary"]["tests"])
        self.assertEqual(45, document["summary"]["assertions"])
        self.assertEqual(36, document["summary"]["direct_oracle"])
        self.assertEqual(9, document["summary"]["setup_or_precondition"])
        self.assertEqual(0, document["summary"]["non_oracle_observation"])
        self.assertEqual(
            45,
            sum(
                len(row["assertion_assignments"])
                for row in document["assignments"]
            ),
        )

    def test_behavior_lookup_resolves_every_reviewed_selector(self) -> None:
        subject = self.subject()
        indexes = subject._behavior_indexes(ROOT)
        expected = {
            (invariant, behavior)
            for routes in subject.REVIEWED_ROUTES.values()
            for invariant, behavior, _disposition in routes.values()
        }
        self.assertEqual(expected, set(indexes))

    def test_runtime_profile_closes_all_four_reviewed_paths(self) -> None:
        from scripts.verification.frozen_rust_nextest_target import (
            get_target_profile,
        )

        expected = {
            "crates/trust-runtime/tests/boundary_resolver.rs": (
                ("--test", "boundary_resolver"),
                "",
            ),
            "crates/trust-runtime/tests/harness_fail_closed.rs": (
                ("--test", "harness_fail_closed"),
                "",
            ),
            "crates/trust-runtime/tests/init_fail_closed.rs": (
                ("--test", "init_fail_closed"),
                "",
            ),
            "crates/trust-runtime/src/value/reference.rs": (
                ("--lib",),
                "value::reference::tests::",
            ),
        }
        for path, (target_args, prefix) in expected.items():
            with self.subTest(path=path):
                profile = get_target_profile(
                    "runtime-remaining",
                    path=path,
                )
                self.assertEqual(target_args, profile.cargo_target_args)
                self.assertEqual(prefix, profile.test_prefix)


if __name__ == "__main__":
    unittest.main()
