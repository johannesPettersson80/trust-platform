"""Focused tests for the reviewed expr_calls semantic assignments."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification import expr_calls_assignments as subject


class ExprCallsAssignmentsTests(unittest.TestCase):
    def test_reviewed_batch_has_exact_six_test_three_route_contract(self) -> None:
        path = "crates/trust-runtime/tests/expr_calls.rs"
        self.assertEqual(
            {
                "DISC_D125E8A7C82805AEE276": (path, "function_call_expr"),
                "DISC_4670B9A0570719575EB2": (
                    path,
                    "function_call_named_args",
                ),
                "DISC_3FEF14488929BE4E886D": (
                    path,
                    "function_call_output_positional",
                ),
                "DISC_C61007734167A2A4CE7E": (path, "function_block_call"),
                "DISC_2D17DC9BA441E47099C3": (
                    path,
                    "function_in_out_with_conversion_expression_regression_issue_13",
                ),
                "DISC_9754A8A2443EE9A1BED9": (
                    path,
                    "function_in_out_without_conversion_expression_baseline",
                ),
            },
            subject.REVIEWED_IDENTITIES,
        )
        self.assertEqual(6, len(subject.TEST_BINDINGS))
        self.assertEqual(
            {
                subject.CALL_BINDING,
                subject.IEC_CALL_BINDING,
                subject.ENCODER_SEAM,
            },
            {binding[0] for binding in subject.TEST_BINDINGS.values()},
        )
        self.assertEqual(
            (
                subject.ENCODER_SEAM,
                "EXPLICIT_VOID_FUNCTION_RETURN_TYPE",
            ),
            subject.TEST_BINDINGS[
                (
                    path,
                    "function_in_out_with_conversion_expression_regression_issue_13",
                )
            ],
        )

    def test_dispositions_keep_setup_separate_from_direct_oracles(self) -> None:
        for kind in ("method:unwrap", "method:expect"):
            with self.subTest(kind=kind):
                self.assertEqual(
                    "setup_or_precondition",
                    subject._disposition(kind),
                )
        for kind in ("method:assert_eq", "macro:assert"):
            with self.subTest(kind=kind):
                self.assertEqual("oracle_bound", subject._disposition(kind))
        with self.assertRaisesRegex(ValueError, "unreviewed expr_calls"):
            subject._disposition("method:unwrap_err")

    def test_live_reviewed_artifact_generates_exact_assignments(self) -> None:
        root = Path(__file__).resolve().parents[2]
        document = json.loads((root / subject.INVENTORY_PATH).read_text())

        result = subject.generate_assignments(
            root=root,
            inventory_document=document,
        )

        self.assertEqual(
            {
                "tests": 6,
                "assertions": 12,
                "direct_oracle": 6,
                "setup_or_precondition": 6,
                "non_oracle_observation": 0,
                "assertion_kinds": {
                    "macro:assert": 2,
                    "method:assert_eq": 4,
                    "method:expect": 2,
                    "method:unwrap": 4,
                },
            },
            result["summary"],
        )
        self.assertEqual(6, len(result["assignments"]))
        for row in result["assignments"]:
            self.assertEqual(2, len(row["assertion_assignments"]))
            self.assertEqual(1, len(row["setup_assertion_ids"]))
            self.assertEqual([], row["non_oracle_assertion_ids"])

    def test_semantic_lock_rejects_expression_and_binding_drift(self) -> None:
        inventories = _one_inventory()
        with patch.object(
            subject,
            "TEST_BINDINGS",
            {("crate/tests.rs", "case"): ("INV", "BEHAVIOR")},
        ):
            lock = subject._review_lock_document(inventories)
            mutated = json.loads(json.dumps(inventories))
            mutated["DISC_TEST"]["assertions"][0]["expression"] = (
                "assert_eq ! (value, 999)"
            )
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                path = root / "lock.json"
                path.write_text(json.dumps(lock))
                with self.assertRaisesRegex(
                    ValueError,
                    "drifted from semantic lock",
                ):
                    subject._validate_review_lock(
                        root,
                        mutated,
                        Path("lock.json"),
                    )

            changed_lock = json.loads(json.dumps(lock))
            changed_lock["tests"][0]["assertions"][0]["behavior_label"] = "OTHER"
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                path = root / "lock.json"
                path.write_text(json.dumps(changed_lock))
                with self.assertRaisesRegex(
                    ValueError,
                    "drifted from semantic lock",
                ):
                    subject._validate_review_lock(
                        root,
                        inventories,
                        Path("lock.json"),
                    )

    def test_unknown_or_incomplete_inventory_fails_closed(self) -> None:
        root = Path(__file__).resolve().parents[2]
        document = json.loads((root / subject.INVENTORY_PATH).read_text())
        document["inventories"].pop("DISC_D125E8A7C82805AEE276")
        with self.assertRaisesRegex(ValueError, "exact reviewed identities"):
            subject.generate_assignments(
                root=root,
                inventory_document=document,
            )


def _one_inventory() -> dict[str, object]:
    return {
        "DISC_TEST": {
            "path": "crate/tests.rs",
            "test_name": "case",
            "source_digest": "sha256:source",
            "inventory_digest": "sha256:inventory",
            "assertions": [
                {
                    "assertion_id": "ASSERT_TEST",
                    "expression_digest": "sha256:expression",
                    "kind": "macro:assert",
                    "function": "case",
                    "expression": "assert_eq ! (value, 7)",
                }
            ],
        }
    }


if __name__ == "__main__":
    unittest.main()
