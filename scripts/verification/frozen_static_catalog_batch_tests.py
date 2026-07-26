from __future__ import annotations

import unittest

from scripts.verification.frozen_static_catalog_batch import (
    assign_behaviors,
    measured_durations,
    validate_execution_provenance,
)


class FrozenStaticCatalogBatchTests(unittest.TestCase):
    def test_assign_behaviors_requires_exactly_one_reviewed_range(self) -> None:
        inventories = {
            "DISC_A": {"assertions": [{"line": 10}]},
            "DISC_B": {"assertions": [{"line": 20}]},
        }
        self.assertEqual(
            {"DISC_A": 0, "DISC_B": 1},
            assign_behaviors(
                inventories=inventories,
                ranges=[
                    {"behavior_index": 0, "min_line": 1, "max_line": 10},
                    {"behavior_index": 1, "min_line": 11, "max_line": 20},
                ],
            ),
        )
        with self.assertRaisesRegex(ValueError, "0 behavior assignments"):
            assign_behaviors(
                inventories=inventories,
                ranges=[
                    {"behavior_index": 0, "min_line": 1, "max_line": 10},
                ],
            )
        with self.assertRaisesRegex(ValueError, "2 behavior assignments"):
            assign_behaviors(
                inventories=inventories,
                ranges=[
                    {"behavior_index": 0, "min_line": 1, "max_line": 20},
                    {"behavior_index": 1, "min_line": 10, "max_line": 20},
                ],
            )
        with self.assertRaisesRegex(
            ValueError,
            "assertions cross behavior assignments",
        ):
            assign_behaviors(
                inventories={
                    "DISC_A": {
                        "assertions": [{"line": 10}, {"line": 11}]
                    }
                },
                ranges=[
                    {"behavior_index": 0, "min_line": 1, "max_line": 10},
                    {"behavior_index": 1, "min_line": 11, "max_line": 20},
                ],
            )

    def test_measured_durations_require_exact_passing_title_set(self) -> None:
        results = {
            "tests": [
                {"title": "one", "duration": 0},
                {"title": "two", "duration": 12},
            ],
            "failures": [],
            "pending": [],
        }
        self.assertEqual(
            {"one": 0, "two": 12},
            measured_durations(
                results_document=results,
                expected_names={"one", "two"},
            ),
        )
        with self.assertRaisesRegex(ValueError, "do not equal"):
            measured_durations(
                results_document=results,
                expected_names={"one"},
            )
        self.assertEqual(
            {"one": 0},
            measured_durations(
                results_document=results,
                expected_names={"one"},
                allowed_extra_names={"two"},
            ),
        )
        with self.assertRaisesRegex(ValueError, "failures or pending"):
            measured_durations(
                results_document={**results, "failures": [{"title": "one"}]},
                expected_names={"one", "two"},
            )

    def test_execution_provenance_binds_command_and_live_inputs(self) -> None:
        import hashlib
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative, content in (
                ("runner.js", b"runner"),
                ("source.ts", b"source"),
                ("compiled.js", b"compiled"),
            ):
                (root / relative).write_bytes(content)
            plan = {
                "command": "node runner.js",
                "runner_path": "runner.js",
                "path": "source.ts",
                "compiled_test_path": "compiled.js",
            }
            execution = {
                "command": plan["command"],
                "runner_path": plan["runner_path"],
                "runner_sha256": "sha256:"
                + hashlib.sha256(b"runner").hexdigest(),
                "source_path": plan["path"],
                "source_sha256": "sha256:"
                + hashlib.sha256(b"source").hexdigest(),
                "compiled_test_path": plan["compiled_test_path"],
                "compiled_test_sha256": "sha256:"
                + hashlib.sha256(b"compiled").hexdigest(),
            }
            validate_execution_provenance(
                root=root,
                plan=plan,
                results_document={"execution": execution},
            )
            execution["command"] = "npm test"
            with self.assertRaisesRegex(ValueError, "command does not match"):
                validate_execution_provenance(
                    root=root,
                    plan=plan,
                    results_document={"execution": execution},
                )


if __name__ == "__main__":
    unittest.main()
