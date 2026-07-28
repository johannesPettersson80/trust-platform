from __future__ import annotations

import importlib
import importlib.util
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = "scripts.verification.host_runtime_observability_assignments"
INVENTORY = (
    ROOT
    / "target/gate-artifacts/verification/"
    "host-runtime-15-assertions.json"
)


class HostRuntimeObservabilityAssignmentsTests(unittest.TestCase):
    def subject(self):
        self.assertIsNotNone(
            importlib.util.find_spec(MODULE),
            "the host-runtime observability assignment generator is missing",
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

    def test_reviewed_batch_is_exactly_15_tests_and_75_assertions(
        self,
    ) -> None:
        subject = self.subject()
        inventories = self.inventories()
        self.assertEqual(
            subject.REVIEWED_IDENTITIES,
            {
                discovery_id: (
                    row["path"],
                    row["test_name"],
                )
                for discovery_id, row in inventories.items()
            },
        )
        self.assertEqual(
            75,
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
                (subject.BACKEND, subject.VM_SELECTION): 3,
                (subject.BACKEND, subject.INVALID_BACKEND): 4,
                (subject.BACKEND, subject.RETIRED_INTERPRETER): 2,
                (subject.BACKEND, subject.FRESH_RUNTIME): 6,
                (subject.BACKEND, subject.LAZY_VM): 6,
                (subject.BACKEND, subject.METRICS_BACKEND): 7,
                (subject.HISTORIAN, subject.TYPED_INTERVAL): 14,
                (subject.HISTORIAN, subject.ALLOWLIST): 6,
                (subject.HISTORIAN, subject.RELOAD): 6,
                (subject.HISTORIAN, subject.PROMETHEUS): 4,
                (subject.METRICS, subject.CALL_CONTRIBUTION): 5,
                (subject.METRICS, subject.TOGGLE): 5,
                (subject.METRICS, subject.TOP_CONTRIBUTOR): 3,
                (subject.METRICS, subject.CYCLE_WINDOW): 4,
            },
            counts,
        )
        self.assertEqual(
            Counter({"oracle_bound": 64, "setup_or_precondition": 11}),
            dispositions,
        )

    def test_only_reviewed_construction_and_lock_steps_are_setup(self) -> None:
        subject = self.subject()
        expected = {
            (
                subject.RECORDING_TEST,
                "ASSERT_499C71C53B3562218E5491DA",
            ),
            (
                subject.RECORDING_TEST,
                "ASSERT_36D58026D01AB2EF5E4F62DA",
            ),
            (
                subject.ALLOWLIST_TEST,
                "ASSERT_779801194E82D1484F9FC0D2",
            ),
            (
                subject.ALLOWLIST_TEST,
                "ASSERT_539CF06A2F04C8691DA8061E",
            ),
            (
                subject.RELOAD_TEST,
                "ASSERT_D06D3A038CF8CC1F7D6BEC73",
            ),
            (
                subject.RELOAD_TEST,
                "ASSERT_81DF3F68943F0559B7677A6A",
            ),
            (
                subject.METRICS_BACKEND_TEST,
                "ASSERT_09C90E4F097C15A66C9D1B22",
            ),
            (
                subject.METRICS_BACKEND_TEST,
                "ASSERT_5C6FA57581BB4DA97844BB8F",
            ),
            (
                subject.METRICS_BACKEND_TEST,
                "ASSERT_B7B99FDB5DC834174A6978DD",
            ),
            (
                subject.METRICS_BACKEND_TEST,
                "ASSERT_F99B1E17B389C06AFFACB0BD",
            ),
            (
                subject.LAZY_VM_TEST,
                "ASSERT_EBA8A97C9863026A728450FF",
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
        discovery_id = "DISC_A7805653302309DFDE9B"
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
            "catalog_test_id": "TEST_AUTO_A7805653302309DFDE9B",
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
            "other production backend",
            "cross-version",
            "arbitrary source",
            "crash durability",
            "concurrent",
            "wildcard grammar",
            "window rollover",
            "performance budget",
            "complete prometheus",
            "iec-wide",
        ):
            self.assertIn(required, combined)

    def test_unknown_assertion_fails_closed(self) -> None:
        subject = self.subject()
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            subject._route(
                test_name=subject.RECORDING_TEST,
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
        self.assertEqual(15, document["summary"]["tests"])
        self.assertEqual(75, document["summary"]["assertions"])
        self.assertEqual(64, document["summary"]["direct_oracle"])
        self.assertEqual(11, document["summary"]["setup_or_precondition"])
        self.assertEqual(0, document["summary"]["non_oracle_observation"])
        self.assertEqual(
            75,
            sum(
                len(row["assertion_assignments"])
                for row in document["assignments"]
            ),
        )
        self.assertEqual(
            11,
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
                (subject.BACKEND, subject.VM_SELECTION),
                (subject.BACKEND, subject.INVALID_BACKEND),
                (subject.BACKEND, subject.RETIRED_INTERPRETER),
                (subject.BACKEND, subject.FRESH_RUNTIME),
                (subject.BACKEND, subject.LAZY_VM),
                (subject.BACKEND, subject.METRICS_BACKEND),
                (subject.HISTORIAN, subject.TYPED_INTERVAL),
                (subject.HISTORIAN, subject.ALLOWLIST),
                (subject.HISTORIAN, subject.RELOAD),
                (subject.HISTORIAN, subject.PROMETHEUS),
                (subject.METRICS, subject.CALL_CONTRIBUTION),
                (subject.METRICS, subject.TOGGLE),
                (subject.METRICS, subject.TOP_CONTRIBUTOR),
                (subject.METRICS, subject.CYCLE_WINDOW),
            },
            set(indexes),
        )

    def test_runtime_profile_closes_all_four_reviewed_paths(self) -> None:
        from scripts.verification.frozen_rust_nextest_target import (
            get_target_profile,
        )

        expected = {
            "crates/trust-runtime/src/host/execution_backend.rs": (
                ("--lib",),
                "execution_backend::tests::",
            ),
            "crates/trust-runtime/src/host/historian/tests.rs": (
                ("--lib",),
                "historian::tests::",
            ),
            "crates/trust-runtime/src/host/metrics.rs": (
                ("--lib",),
                "metrics::tests::",
            ),
            "crates/trust-runtime/tests/api_smoke.rs": (
                ("--test", "api_smoke"),
                "",
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
