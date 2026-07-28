from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.verification.debug_authority_assignments import (
    CONTROL,
    REVIEW_LOCK_PATH,
    TEST_BINDINGS,
    _behavior_indexes,
    _disposition,
    _review_lock_document,
    _validate_review_lock,
)


class DebugAuthorityAssignmentsTests(unittest.TestCase):
    def test_exact_path_name_bindings_cover_reviewed_batch(self) -> None:
        self.assertEqual(8, len(TEST_BINDINGS))
        self.assertEqual(
            (CONTROL, "LIVE_BREAKPOINT_MUTATION"),
            TEST_BINDINGS[
                (
                    "crates/trust-runtime/tests/debug_stepping.rs",
                    "breakpoint_set_after_launch_hits_next_cycle",
                )
            ],
        )
        with self.assertRaises(KeyError):
            TEST_BINDINGS[
                (
                    "crates/other/tests/debug_stepping.rs",
                    "breakpoint_set_after_launch_hits_next_cycle",
                )
            ]

    def test_dispositions_keep_only_assert_macros_as_oracles(self) -> None:
        for kind in ("method:unwrap", "method:expect", "macro:panic"):
            with self.subTest(kind=kind):
                self.assertEqual("setup_or_precondition", _disposition(kind))
        for kind in ("macro:assert", "macro:assert_eq", "macro:assert_ne"):
            with self.subTest(kind=kind):
                self.assertEqual("oracle_bound", _disposition(kind))
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            _disposition("macro:debug_assert")

    def test_semantic_lock_rejects_expression_replacement(self) -> None:
        inventories = _one_inventory()
        lock = _review_lock_document(inventories)
        mutated = json.loads(json.dumps(inventories))
        mutated["DISC_TEST"]["assertions"][0]["expression"] = (
            "assert_eq ! (stop . reason , DebugStopReason :: Pause)"
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
            "CYCLIC_BREAKPOINT_REHIT"
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
            invariant_dir = root / "verification/invariants/editor_safety"
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
    path = "crates/trust-runtime/tests/debug_stepping.rs"
    name = "breakpoint_set_after_launch_hits_next_cycle"
    return {
        "DISC_TEST": {
            "path": path,
            "test_name": name,
            "assertions": [
                {
                    "kind": "macro:assert_eq",
                    "function": name,
                    "expression": (
                        "assert_eq ! (stop . reason , "
                        "DebugStopReason :: Breakpoint)"
                    ),
                }
            ],
        }
    }


if __name__ == "__main__":
    unittest.main()
