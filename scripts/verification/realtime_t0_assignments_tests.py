from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.verification.realtime_t0_assignments import (
    CYCLE,
    REVIEW_LOCK_PATH,
    TEST_BINDINGS,
    TRANSPORT,
    _behavior_for_assertion,
    _behavior_indexes,
    _disposition,
    _review_lock_document,
    _validate_review_lock,
)


class RealtimeT0AssignmentsTests(unittest.TestCase):
    def test_route_test_separates_fallback_from_legal_exchange(self) -> None:
        self.assertEqual(
            _behavior_for_assertion(
                path="crates/trust-runtime/tests/realtime_t0_integration.rs",
                name="realtime_t0_route_does_not_fallback_to_mesh_ip_path",
                expression="transport . fallback_denied_total ()",
                function="realtime_t0_route_does_not_fallback_to_mesh_ip_path",
            ),
            (TRANSPORT, "T0_ROUTE_REJECTS_MESH_FALLBACK"),
        )
        self.assertEqual(
            _behavior_for_assertion(
                path="crates/trust-runtime/tests/realtime_t0_integration.rs",
                name="realtime_t0_route_does_not_fallback_to_mesh_ip_path",
                expression="assert_eq ! (out , [1 , 2 , 3 , 4])",
                function="realtime_t0_route_does_not_fallback_to_mesh_ip_path",
            ),
            (TRANSPORT, "SAME_PROCESS_FIXED_PAYLOAD_EXCHANGE"),
        )

    def test_route_test_helper_is_bound_to_cross_process_behavior(self) -> None:
        self.assertEqual(
            _behavior_for_assertion(
                path="crates/trust-runtime/tests/realtime_t0_integration.rs",
                name="realtime_t0_route_does_not_fallback_to_mesh_ip_path",
                expression="assert_eq ! (out , [4 , 3 , 2 , 1])",
                function="maybe_run_shm_child_consumer",
            ),
            (TRANSPORT, "CROSS_PROCESS_FIXED_PAYLOAD_VISIBILITY"),
        )

    def test_same_test_name_from_wrong_path_is_rejected(self) -> None:
        with self.assertRaises(KeyError):
            _behavior_for_assertion(
                path="crates/other/tests/realtime_t0_integration.rs",
                name="realtime_t0_route_does_not_fallback_to_mesh_ip_path",
                expression="assert_eq ! (out , [1 , 2 , 3 , 4])",
                function="realtime_t0_route_does_not_fallback_to_mesh_ip_path",
            )

    def test_cycle_test_separates_order_from_budget(self) -> None:
        self.assertEqual(
            _behavior_for_assertion(
                path=(
                    "crates/trust-runtime/src/host/realtime/"
                    "realtime_tests_part_03.rs"
                ),
                name="t0_cycle_scheduler_enforces_pre_post_order_and_cloud_budget",
                expression="assert_eq ! (out_of_order . code , ContractViolation)",
                function="t0_cycle_scheduler_enforces_pre_post_order_and_cloud_budget",
            ),
            (CYCLE, "POST_BEFORE_PRE_REJECTION"),
        )
        self.assertEqual(
            _behavior_for_assertion(
                path=(
                    "crates/trust-runtime/src/host/realtime/"
                    "realtime_tests_part_03.rs"
                ),
                name="t0_cycle_scheduler_enforces_pre_post_order_and_cloud_budget",
                expression=(
                    "assert_eq ! (scheduler . exchange_points_seen (), "
                    "(true, true))"
                ),
                function="t0_cycle_scheduler_enforces_pre_post_order_and_cloud_budget",
            ),
            (CYCLE, "PRE_TASK_POST_TASK_ORDER"),
        )
        self.assertEqual(
            _behavior_for_assertion(
                path=(
                    "crates/trust-runtime/src/host/realtime/"
                    "realtime_tests_part_03.rs"
                ),
                name="t0_cycle_scheduler_enforces_pre_post_order_and_cloud_budget",
                expression="assert_eq ! (first , 2)",
                function="t0_cycle_scheduler_enforces_pre_post_order_and_cloud_budget",
            ),
            (CYCLE, "PER_CYCLE_CLOUD_BUDGET"),
        )

    def test_dispositions_distinguish_setup_and_oracle(self) -> None:
        self.assertEqual(
            _disposition(
                name="t0_bind_enforces_schema_hash_and_fixed_layout_contract",
                expression="transport . register_channel () . expect (\"register\")",
                kind="method:expect",
                function="t0_bind_enforces_schema_hash_and_fixed_layout_contract",
            ),
            "setup_or_precondition",
        )
        self.assertEqual(
            _disposition(
                name="t0_bind_rejects_non_t0_route_and_denies_fallback",
                expression="assert ! (err . message . contains (\"non-HardRT\"))",
                kind="macro:assert",
                function="t0_bind_rejects_non_t0_route_and_denies_fallback",
            ),
            "oracle_bound",
        )
        self.assertEqual(
            _disposition(
                name="t0_bind_rejects_non_t0_route_and_denies_fallback",
                expression="assert_eq ! (err . code , ContractViolation)",
                kind="macro:assert_eq",
                function="t0_bind_rejects_non_t0_route_and_denies_fallback",
            ),
            "oracle_bound",
        )

    def test_unused_child_helper_is_setup_outside_multi_process_test(self) -> None:
        self.assertEqual(
            _disposition(
                name="realtime_t0_determinism_holds_under_cloud_budget_pressure",
                expression="assert_eq ! (out , payload)",
                kind="macro:assert_eq",
                function="maybe_run_shm_child_consumer",
            ),
            "setup_or_precondition",
        )
        self.assertEqual(
            _disposition(
                name="realtime_t0_multi_process_shm_exchange_succeeds",
                expression="assert_eq ! (out , payload)",
                kind="macro:assert_eq",
                function="maybe_run_shm_child_consumer",
            ),
            "oracle_bound",
        )

    def test_semantic_lock_rejects_same_kind_expression_replacement(self) -> None:
        inventories = _one_inventory()
        lock = _review_lock_document(inventories)
        mutated = json.loads(json.dumps(inventories))
        mutated["DISC_TEST"]["assertions"][0]["expression"] = (
            "assert_eq ! (err . code , TransportFailure)"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / REVIEW_LOCK_PATH
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps(lock))
            with self.assertRaisesRegex(ValueError, "drifted from semantic lock"):
                _validate_review_lock(root, mutated, REVIEW_LOCK_PATH)

    def test_semantic_lock_rejects_binding_remap(self) -> None:
        inventories = _one_inventory()
        lock = _review_lock_document(inventories)
        lock["tests"][0]["assertions"][0]["behavior_label"] = (
            "BIND_SCHEMA_AND_FIXED_LAYOUT"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / REVIEW_LOCK_PATH
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps(lock))
            with self.assertRaisesRegex(ValueError, "drifted from semantic lock"):
                _validate_review_lock(root, inventories, REVIEW_LOCK_PATH)

    def test_named_behavior_index_resolution_is_exhaustive(self) -> None:
        grouped: dict[str, list[str]] = {}
        for invariant_id, label in TEST_BINDINGS.values():
            grouped.setdefault(invariant_id, [])
            if label not in grouped[invariant_id]:
                grouped[invariant_id].append(label)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            invariant_dir = root / "verification/invariants/runtime_safety"
            invariant_dir.mkdir(parents=True)
            for invariant_id, labels in grouped.items():
                rows = "\n".join(
                    f'[[behavior]]\npartition = {{ equals = "{label}" }}'
                    for label in labels
                )
                (invariant_dir / f"{invariant_id}.toml").write_text(rows)
            indexes = _behavior_indexes(root)
        self.assertEqual(set(indexes), set(TEST_BINDINGS.values()))


def _one_inventory() -> dict[str, object]:
    return {
        "DISC_TEST": {
            "path": (
                "crates/trust-runtime/src/host/realtime/"
                "realtime_tests_part_02.rs"
            ),
            "test_name": "t0_bind_rejects_non_t0_route_and_denies_fallback",
            "assertions": [
                {
                    "kind": "macro:assert_eq",
                    "function": "t0_bind_rejects_non_t0_route_and_denies_fallback",
                    "expression": (
                        "assert_eq ! (err . code , T0ErrorCode :: "
                        "ContractViolation)"
                    ),
                }
            ],
        }
    }


if __name__ == "__main__":
    unittest.main()
