from __future__ import annotations

import unittest

from scripts.verification.frozen_static_catalog_batch import (
    _catalog_command,
    assign_behavior_refs,
    assign_behaviors,
    measured_durations,
    selection_digest,
    validate_applied_batch,
    validate_execution_provenance,
)
from scripts.verification.frozen_rust_nextest_target import (
    RUNTIME_CONTROL_PROFILE,
    no_default_feature_test_names,
)
from scripts.verification.frozen_static_semantic_assignments import (
    resolve_behavior_assignments,
)


class FrozenStaticCatalogBatchTests(unittest.TestCase):
    def test_selection_digest_binds_exact_frozen_rows(self) -> None:
        row = {
            "discovery_id": "DISC_A",
            "discovery_source_kind": "vscode_test",
            "path": "test.ts",
            "name": "one",
            "ignore_state": "active",
        }
        baseline = selection_digest([row])
        self.assertEqual(baseline, selection_digest([dict(row)]))
        self.assertNotEqual(
            baseline,
            selection_digest([{**row, "name": "renamed"}]),
        )

    def test_assign_behaviors_requires_exactly_one_reviewed_range(self) -> None:
        inventories = {
            "DISC_A": {"assertions": [{"line": 10}]},
            "DISC_B": {"assertions": [{"line": 20}]},
        }
        self.assertEqual(
            {"DISC_A": (0,), "DISC_B": (1,)},
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
        self.assertEqual(
            {"DISC_A": (0, 1)},
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
            ),
        )

    def test_assign_behavior_refs_supports_one_atomic_multi_invariant_batch(
        self,
    ) -> None:
        self.assertEqual(
            {
                "DISC_A": (("INV_A", 0),),
                "DISC_B": (("INV_B", 2),),
            },
            assign_behavior_refs(
                inventories={
                    "DISC_A": {"assertions": [{"line": 10}]},
                    "DISC_B": {"assertions": [{"line": 20}]},
                },
                ranges=[
                    {
                        "invariant_id": "INV_A",
                        "behavior_index": 0,
                        "min_line": 1,
                        "max_line": 10,
                    },
                    {
                        "invariant_id": "INV_B",
                        "behavior_index": 2,
                        "min_line": 11,
                        "max_line": 20,
                    },
                ],
                default_invariant_id=None,
                allowed_invariant_ids={"INV_A", "INV_B"},
            ),
        )

    def test_name_scoped_assignments_disambiguate_shared_helper_lines(
        self,
    ) -> None:
        inventories = {
            "DISC_A": {
                "test_name": "case_a",
                "assertions": [{"line": 33}],
            },
            "DISC_B": {
                "test_name": "case_b",
                "assertions": [{"line": 33}],
            },
        }
        base = {
            "inventories": inventories,
            "default_invariant_id": None,
            "allowed_invariant_ids": {"INV_A", "INV_B"},
        }

        self.assertEqual(
            {
                "DISC_A": (("INV_A", 0),),
                "DISC_B": (("INV_B", 2),),
            },
            resolve_behavior_assignments(
                **base,
                plan={
                    "test_assignments": [
                        {
                            "name": "case_a",
                            "invariant_id": "INV_A",
                            "behavior_index": 0,
                        },
                        {
                            "name": "case_b",
                            "invariant_id": "INV_B",
                            "behavior_index": 2,
                        },
                    ]
                },
            ),
        )

        invalid_plans = (
            (
                {
                    "test_assignments": [
                        {
                            "name": "case_a",
                            "invariant_id": "INV_A",
                            "behavior_index": 0,
                        }
                    ]
                },
                "missing names",
            ),
            (
                {
                    "test_assignments": [
                        {
                            "name": "case_a",
                            "invariant_id": "INV_A",
                            "behavior_index": 0,
                        },
                        {
                            "name": "case_a",
                            "invariant_id": "INV_A",
                            "behavior_index": 1,
                        },
                    ]
                },
                "duplicate name",
            ),
            (
                {
                    "test_assignments": [
                        {
                            "name": "case_a",
                            "invariant_id": "INV_A",
                            "behavior_index": 0,
                        },
                        {
                            "name": "case_b",
                            "invariant_id": "INV_B",
                            "behavior_index": 2,
                        },
                        {
                            "name": "case_c",
                            "invariant_id": "INV_A",
                            "behavior_index": 0,
                        },
                    ]
                },
                "extra names",
            ),
            (
                {
                    "test_assignments": [
                        {
                            "name": "case_a",
                            "invariant_id": "INV_UNAPPROVED",
                            "behavior_index": 0,
                        },
                        {
                            "name": "case_b",
                            "invariant_id": "INV_B",
                            "behavior_index": 2,
                        },
                    ]
                },
                "unapproved invariant",
            ),
            (
                {
                    "test_assignments": [
                        {
                            "name": "case_a",
                            "invariant_id": "INV_A",
                            "behavior_index": -1,
                        },
                        {
                            "name": "case_b",
                            "invariant_id": "INV_B",
                            "behavior_index": 2,
                        },
                    ]
                },
                "behavior_index",
            ),
            (
                {
                    "test_assignments": [
                        {
                            "name": "case_a",
                            "invariant_id": "INV_A",
                            "behavior_index": True,
                        },
                        {
                            "name": "case_b",
                            "invariant_id": "INV_B",
                            "behavior_index": 2,
                        },
                    ]
                },
                "behavior_index",
            ),
            (
                {
                    "ranges": [],
                    "test_assignments": [
                        {
                            "name": "case_a",
                            "invariant_id": "INV_A",
                            "behavior_index": 0,
                        },
                        {
                            "name": "case_b",
                            "invariant_id": "INV_B",
                            "behavior_index": 2,
                        },
                    ],
                },
                "cannot combine",
            ),
        )
        for plan, message in invalid_plans:
            with self.subTest(message=message):
                with self.assertRaisesRegex(ValueError, message):
                    resolve_behavior_assignments(**base, plan=plan)

    def test_assertion_scoped_assignments_bind_every_assertion_exactly(
        self,
    ) -> None:
        inventories = {
            "DISC_MULTI": {
                "test_name": "multi_case",
                "assertions": [
                    {"assertion_id": "ASSERT_SNAPSHOT", "line": 49},
                    {"assertion_id": "ASSERT_MISSING", "line": 50},
                ],
            }
        }
        base = {
            "inventories": inventories,
            "default_invariant_id": None,
            "allowed_invariant_ids": {"INV_INVENTORY"},
        }
        plan = {
            "assertion_assignments": [
                {
                    "name": "multi_case",
                    "assertions": [
                        {
                            "assertion_id": "ASSERT_SNAPSHOT",
                            "invariant_id": "INV_INVENTORY",
                            "behavior_index": 3,
                        },
                        {
                            "assertion_id": "ASSERT_MISSING",
                            "invariant_id": "INV_INVENTORY",
                            "behavior_index": 4,
                        },
                    ],
                }
            ]
        }

        self.assertEqual(
            {
                "DISC_MULTI": (
                    ("INV_INVENTORY", 3),
                    ("INV_INVENTORY", 4),
                )
            },
            resolve_behavior_assignments(**base, plan=plan),
        )
        plan["assertion_assignments"][0]["assertions"].pop()
        with self.assertRaisesRegex(ValueError, "reviewed assertion IDs"):
            resolve_behavior_assignments(**base, plan=plan)

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
        with self.assertRaisesRegex(ValueError, "state is not passed"):
            measured_durations(
                results_document={
                    **results,
                    "execution": {"kind": "rust_nextest_jsonl"},
                    "tests": [
                        {"title": "one", "duration": 0, "state": "failed"},
                        {"title": "two", "duration": 12, "state": "passed"},
                    ],
                    "stats": {
                        "tests": 2,
                        "passes": 1,
                        "failures": 1,
                        "pending": 0,
                    },
                },
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

    def test_rust_execution_provenance_binds_raw_results(self) -> None:
        import hashlib
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            raw_content = (
                b'{"type":"test","event":"ok",'
                b'"name":"control::tests::one","exec_time":0.001}\n'
            )
            for relative, content in (
                ("runner.py", b"runner"),
                ("source.rs", b"source"),
                ("raw.jsonl", raw_content),
            ):
                (root / relative).write_bytes(content)
            plan = {
                "command": "cargo nextest run exact-filter",
                "execution_kind": "rust_nextest_jsonl",
                "runner_path": "runner.py",
                "path": "source.rs",
                "raw_results_path": "raw.jsonl",
                "package": "trust-runtime",
                "target_kind": "lib",
                "binary_id": "trust-runtime",
                "test_prefix": "control::tests::",
            }
            execution = {
                "kind": "rust_nextest_jsonl",
                "batch_command": plan["command"],
                "runner_path": plan["runner_path"],
                "runner_sha256": "sha256:"
                + hashlib.sha256(b"runner").hexdigest(),
                "source_path": plan["path"],
                "source_sha256": "sha256:"
                + hashlib.sha256(b"source").hexdigest(),
                "raw_results_path": plan["raw_results_path"],
                "raw_results_sha256": "sha256:"
                + hashlib.sha256(raw_content).hexdigest(),
                "nextest_version": "0.9.140",
                "package": plan["package"],
                "target_kind": plan["target_kind"],
                "binary_id": plan["binary_id"],
                "test_prefix": plan["test_prefix"],
            }
            validate_execution_provenance(
                root=root,
                plan=plan,
                results_document={
                    "execution": execution,
                    "tests": [
                        {"title": "one", "duration": 1, "state": "passed"}
                    ],
                },
            )
            with self.assertRaisesRegex(ValueError, "do not match raw"):
                validate_execution_provenance(
                    root=root,
                    plan=plan,
                    results_document={
                        "execution": execution,
                        "tests": [
                            {
                                "title": "one",
                                "duration": 999,
                                "state": "passed",
                            }
                        ],
                    },
                )

    def test_st_execution_provenance_binds_raw_results_and_git_head(
        self,
    ) -> None:
        import hashlib
        import json
        import tempfile
        from pathlib import Path
        from unittest.mock import patch

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            head = "a" * 40
            raw_document = {
                "version": 1,
                "project": "fixture",
                "summary": {
                    "total": 1,
                    "passed": 1,
                    "failed": 0,
                    "errors": 0,
                    "duration_ms": 7,
                },
                "tests": [
                    {
                        "name": "case_one",
                        "kind": "TEST_PROGRAM",
                        "status": "passed",
                        "file": "fixture/src/tests.st",
                        "line": 4,
                        "source": "PROGRAM case_one",
                        "message": None,
                        "duration_ms": 7,
                    }
                ],
            }
            raw_content = (
                json.dumps(raw_document, sort_keys=True) + "\n"
            ).encode()
            for relative, content in (
                ("scripts/verification/frozen_st_batch.py", b"runner"),
                ("scripts/run_frozen_st_batch.py", b"shim"),
                ("fixture/src/tests.st", b"source"),
                ("raw.json", raw_content),
            ):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(content)
            plan = {
                "execution_kind": "st_json",
                "command": (
                    "trust-dev test --project fixture --filter case_ "
                    "--output json"
                ),
                "runner_path": "scripts/verification/frozen_st_batch.py",
                "shim_path": "scripts/run_frozen_st_batch.py",
                "path": "fixture/src/tests.st",
                "raw_results_path": "raw.json",
                "git_head": head,
                "project_path": "fixture",
                "test_prefix": "case_",
            }
            execution = {
                "kind": "st_json",
                "command": plan["command"],
                "runner_path": plan["runner_path"],
                "runner_sha256": "sha256:"
                + hashlib.sha256(b"runner").hexdigest(),
                "shim_path": plan["shim_path"],
                "shim_sha256": "sha256:"
                + hashlib.sha256(b"shim").hexdigest(),
                "source_path": plan["path"],
                "source_sha256": "sha256:"
                + hashlib.sha256(b"source").hexdigest(),
                "raw_results_path": plan["raw_results_path"],
                "raw_results_sha256": "sha256:"
                + hashlib.sha256(raw_content).hexdigest(),
                "git_head": head,
                "project_path": plan["project_path"],
                "test_prefix": plan["test_prefix"],
            }
            results = {
                "execution": execution,
                "tests": [
                    {"title": "case_one", "duration": 7, "state": "passed"}
                ],
                "failures": [],
                "pending": [],
                "stats": {
                    "tests": 1,
                    "passes": 1,
                    "failures": 0,
                    "pending": 0,
                },
            }
            with patch(
                "scripts.verification.frozen_static_catalog_batch._git_head",
                return_value=head,
            ):
                validate_execution_provenance(
                    root=root,
                    plan=plan,
                    results_document=results,
                )
            execution["git_head"] = "b" * 40
            with patch(
                "scripts.verification.frozen_static_catalog_batch._git_head",
                return_value=head,
            ), self.assertRaisesRegex(ValueError, "git_head"):
                validate_execution_provenance(
                    root=root,
                    plan=plan,
                    results_document=results,
                )

    def test_rust_catalog_command_is_generated_canonically(self) -> None:
        plan = {
            "execution_kind": "rust_nextest_jsonl",
            "package": "trust-runtime",
            "target_kind": "lib",
            "binary_id": "trust-runtime",
            "test_prefix": "control::tests::",
        }
        command = _catalog_command(plan, test_name="one")
        self.assertIn("binary_id(=trust-runtime)", command)
        self.assertIn("test(=control::tests::one)", command)
        self.assertNotIn("--no-default-features", command)
        disabled_command = _catalog_command(
            plan,
            test_name="without_wire",
            no_default_features=True,
        )
        self.assertIn("--lib --no-default-features", disabled_command)
        with self.assertRaisesRegex(ValueError, "generated canonically"):
            _catalog_command(
                {
                    **plan,
                    "catalog_command_template": "cargo test {name}",
                },
                test_name="one",
            )

    def test_runtime_cfg_source_selects_per_test_catalog_feature_flag(
        self,
    ) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "core.rs"
            source.write_text(
                """
#[test]
fn default_case() {}

#[cfg(not(feature = "ads-wire"))]
#[test]
fn without_wire() {}
"""
            )
            self.assertEqual(
                {"without_wire"},
                no_default_feature_test_names(
                    profile=RUNTIME_CONTROL_PROFILE,
                    source=source.read_text(),
                    test_names=("default_case", "without_wire"),
                ),
            )

    def test_hir_integration_catalog_command_is_generated_canonically(
        self,
    ) -> None:
        plan = {
            "execution_kind": "rust_nextest_jsonl",
            "target_profile": "hir-semantic-control-flow",
            "package": "trust-hir",
            "target_kind": "test",
            "binary_id": "trust-hir::semantic_type_checking",
            "test_prefix": "control_flow_and_calls::",
        }

        command = _catalog_command(
            plan,
            test_name="test_for_loop_bounds_integer",
        )

        self.assertIn("-p trust-hir --test semantic_type_checking", command)
        self.assertIn(
            "binary_id(=trust-hir::semantic_type_checking)",
            command,
        )
        self.assertIn(
            "test(=control_flow_and_calls::test_for_loop_bounds_integer)",
            command,
        )
        with self.assertRaisesRegex(ValueError, "closed target profile"):
            _catalog_command(
                {
                    **plan,
                    "binary_id": "trust-hir::another_target",
                },
                test_name="test_for_loop_bounds_integer",
            )

    def test_multi_invariant_post_apply_validation_rejects_cross_links(
        self,
    ) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "verification/invariants/test").mkdir(parents=True)
            (root / "verification/test-catalog.toml").write_text(
                """
[[tests]]
id = "TEST_A"
discovery_id = "DISC_A"
invariants = ["INV_A"]
mapping_behavior_refs = ["INV_A#behavior:0"]

[[tests]]
id = "TEST_B"
discovery_id = "DISC_B"
invariants = ["INV_B"]
mapping_behavior_refs = ["INV_B#behavior:0"]
"""
            )
            (root / "verification/test-catalog-denominator.toml").write_text(
                """
[[reviews]]
discovery_id = "DISC_A"
disposition = "catalog_mapped"

[[reviews]]
discovery_id = "DISC_B"
disposition = "catalog_mapped"
"""
            )
            (root / "verification/test-catalog-proposals.toml").write_text(
                "proposals = []\n"
            )

            def write_invariant(
                invariant_id: str,
                tests: list[str],
                missing: list[str] | None = None,
            ) -> None:
                rendered_tests = ", ".join(f'"{value}"' for value in tests)
                rendered_missing = ", ".join(
                    f'"{value}"' for value in (missing or [])
                )
                (root / f"verification/invariants/test/{invariant_id}.toml").write_text(
                    f'id = "{invariant_id}"\n'
                    f"tests = [{rendered_tests}]\n"
                    f"missing = [{rendered_missing}]\n"
                )

            write_invariant("INV_A", ["TEST_A"])
            write_invariant("INV_B", ["TEST_B"])
            arguments = {
                "root": root,
                "selected_ids": {"DISC_A", "DISC_B"},
                "invariant_ids": {"INV_A", "INV_B"},
                "invariant_test_baselines": {
                    "INV_A": set(),
                    "INV_B": set(),
                },
                "invariant_missing_baselines": {
                    "INV_A": set(),
                    "INV_B": set(),
                },
            }
            self.assertEqual([], validate_applied_batch(**arguments))

            write_invariant(
                "INV_A",
                ["TEST_A"],
                ["cataloged_behavior_tests"],
            )
            arguments["invariant_missing_baselines"]["INV_A"] = {
                "cataloged_behavior_tests"
            }
            self.assertEqual([], validate_applied_batch(**arguments))

            arguments["invariant_missing_baselines"]["INV_A"] = set()
            failures = validate_applied_batch(**arguments)
            self.assertTrue(
                any("introduced catalog mapping debt" in failure for failure in failures)
            )

            write_invariant("INV_A", ["TEST_A"])
            arguments["invariant_missing_baselines"]["INV_A"] = set()
            write_invariant("INV_A", ["TEST_A", "TEST_B"])
            write_invariant("INV_B", [])
            failures = validate_applied_batch(**arguments)
            self.assertTrue(
                any("INV_A test links" in failure for failure in failures)
            )
            self.assertTrue(
                any("INV_B test links" in failure for failure in failures)
            )


if __name__ == "__main__":
    unittest.main()
