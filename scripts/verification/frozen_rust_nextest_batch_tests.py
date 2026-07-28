from __future__ import annotations

import hashlib
import json
import shlex
import tempfile
import unittest
from pathlib import Path

from scripts.verification.frozen_rust_nextest_batch import (
    ADS_WIRE_DISABLED_CFG,
    BINARY_ID,
    CATALOG_PATH,
    DENOMINATOR_PATH,
    RUNNER_PATH,
    SHIM_PATH,
    TEST_NAME_PREFIX,
    FrozenSelection,
    build_nextest_command,
    build_normalized_document,
    derive_frozen_selection,
    derive_frozen_selection_from_evidence,
    main,
    normalize_nextest_jsonl,
    split_feature_shards,
    _selected_discovery_ids_for_path,
)
from scripts.verification.frozen_rust_nextest_target import (
    HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
    TARGET_PROFILES,
    get_target_profile,
)


def _write_toml(path: Path, content: str) -> None:
    path.write_text(content.strip() + "\n")


def _sha256(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


class FrozenRustNextestBatchTests(unittest.TestCase):
    def test_combined_selection_inventory_filters_requested_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inventory = root / "selected.json"
            inventory.write_text(
                json.dumps(
                    {
                        "schema_version": 2,
                        "inventories": {
                            "DISC_A": {"path": "tests/a.rs"},
                            "DISC_B": {"path": "tests/b.rs"},
                            "DISC_C": {"path": "tests/a.rs"},
                        },
                    }
                )
            )

            self.assertEqual(
                frozenset({"DISC_A", "DISC_C"}),
                _selected_discovery_ids_for_path(
                    root,
                    Path("selected.json"),
                    "tests/a.rs",
                ),
            )
            with self.assertRaisesRegex(ValueError, "no tests"):
                _selected_discovery_ids_for_path(
                    root,
                    Path("selected.json"),
                    "tests/missing.rs",
                )

    def test_selection_is_exact_reviewed_plus_catalog_mapped_union(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            verification = root / "verification"
            verification.mkdir()
            _write_toml(
                verification / "test-catalog-denominator.toml",
                """
                [[reviews]]
                discovery_id = "DISC_REVIEWED"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "reviewed_case"
                disposition = "reviewed_nonmapping"
                ignore_state = "not_ignored"

                [[reviews]]
                discovery_id = "DISC_MAPPED"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "mapped_case"
                disposition = "catalog_mapped"
                ignore_state = "not_ignored"

                [[reviews]]
                discovery_id = "DISC_OTHER"
                discovery_source_kind = "rust_unit_test"
                path = "tests/other.rs"
                name = "other_case"
                disposition = "reviewed_nonmapping"
                ignore_state = "not_ignored"
                """,
            )
            _write_toml(
                verification / "test-catalog.toml",
                """
                [[tests]]
                discovery_id = "DISC_MAPPED"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "mapped_case"
                status = "mapped"
                """,
            )

            selection = derive_frozen_selection(
                root=root,
                path="tests/core.rs",
                expected_reviewed_nonmapping=1,
                expected_catalog_mapped=1,
            )

            self.assertEqual(("mapped_case", "reviewed_case"), selection.names)
            self.assertEqual(
                (
                    f"{TEST_NAME_PREFIX}mapped_case",
                    f"{TEST_NAME_PREFIX}reviewed_case",
                ),
                selection.full_names,
            )

    def test_selection_can_filter_reviewed_nonmapping_reason(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            verification = root / "verification"
            verification.mkdir()
            _write_toml(
                verification / "test-catalog-denominator.toml",
                """
                [[reviews]]
                discovery_id = "DISC_READY"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "ready_case"
                disposition = "reviewed_nonmapping"
                reason_code = "no_reviewed_spec_or_invariant_binding"
                ignore_state = "not_ignored"

                [[reviews]]
                discovery_id = "DISC_BLOCKED"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "blocked_case"
                disposition = "reviewed_nonmapping"
                reason_code = "assertion_oracle_unresolved"
                ignore_state = "not_ignored"
                """,
            )
            _write_toml(
                verification / "test-catalog.toml",
                "schema_version = 2\ntests = []",
            )

            selection = derive_frozen_selection(
                root=root,
                path="tests/core.rs",
                expected_reviewed_nonmapping=1,
                expected_catalog_mapped=0,
                nonmapping_reason_code=(
                    "no_reviewed_spec_or_invariant_binding"
                ),
            )

            self.assertEqual(("ready_case",), selection.names)

    def test_selection_can_use_exact_assertion_inventory_ids(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            verification = root / "verification"
            verification.mkdir()
            _write_toml(
                verification / "test-catalog-denominator.toml",
                """
                [[reviews]]
                discovery_id = "DISC_SELECTED"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "selected_case"
                disposition = "reviewed_nonmapping"
                reason_code = "no_reviewed_spec_or_invariant_binding"
                ignore_state = "not_ignored"

                [[reviews]]
                discovery_id = "DISC_DEFERRED"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "deferred_case"
                disposition = "reviewed_nonmapping"
                reason_code = "no_reviewed_spec_or_invariant_binding"
                ignore_state = "not_ignored"
                """,
            )
            _write_toml(
                verification / "test-catalog.toml",
                "schema_version = 2\ntests = []",
            )

            selection = derive_frozen_selection(
                root=root,
                path="tests/core.rs",
                expected_reviewed_nonmapping=1,
                expected_catalog_mapped=0,
                nonmapping_reason_code=(
                    "no_reviewed_spec_or_invariant_binding"
                ),
                selected_discovery_ids=frozenset({"DISC_SELECTED"}),
            )

            self.assertEqual(("selected_case",), selection.names)

    def test_selection_rejects_invalid_frozen_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            verification = root / "verification"
            verification.mkdir()
            _write_toml(
                verification / "test-catalog-denominator.toml",
                """
                [[reviews]]
                discovery_id = "DISC_A"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "same"
                disposition = "reviewed_nonmapping"
                ignore_state = "not_ignored"

                [[reviews]]
                discovery_id = "DISC_B"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "same"
                disposition = "catalog_mapped"
                ignore_state = "not_ignored"
                """,
            )
            _write_toml(
                verification / "test-catalog.toml",
                """
                [[tests]]
                discovery_id = "DISC_WRONG"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "same"
                status = "mapped"
                """,
            )

            with self.assertRaisesRegex(ValueError, "mapped denominator identities"):
                derive_frozen_selection(
                    root=root,
                    path="tests/core.rs",
                    expected_reviewed_nonmapping=1,
                    expected_catalog_mapped=1,
                )

            _write_toml(
                verification / "test-catalog.toml",
                """
                [[tests]]
                discovery_id = "DISC_B"
                discovery_source_kind = "rust_unit_test"
                path = "tests/core.rs"
                name = "same"
                status = "mapped"
                """,
            )
            with self.assertRaisesRegex(ValueError, "duplicate selected test name"):
                derive_frozen_selection(
                    root=root,
                    path="tests/core.rs",
                    expected_reviewed_nonmapping=1,
                    expected_catalog_mapped=1,
                )
            with self.assertRaisesRegex(ValueError, "expected 2"):
                derive_frozen_selection(
                    root=root,
                    path="tests/core.rs",
                    expected_reviewed_nonmapping=2,
                    expected_catalog_mapped=1,
                )

    def test_nextest_command_uses_one_exact_sorted_or_filter(self) -> None:
        command = build_nextest_command(
            (
                f"{TEST_NAME_PREFIX}second",
                f"{TEST_NAME_PREFIX}first",
            )
        )
        self.assertEqual(
            [
                "cargo",
                "nextest",
                "run",
                "-p",
                "trust-runtime",
                "--lib",
                "--no-fail-fast",
                "--message-format",
                "libtest-json-plus",
                "--message-format-version",
                "0.1",
                "--user-config-file",
                "none",
                "-E",
                (
                    f"(binary_id(={BINARY_ID}) & "
                    f"test(={TEST_NAME_PREFIX}first)) | "
                    f"(binary_id(={BINARY_ID}) & "
                    f"test(={TEST_NAME_PREFIX}second))"
                ),
            ],
            command,
        )

    def test_hir_profile_uses_exact_integration_target_filter(self) -> None:
        prefix = HIR_SEMANTIC_CONTROL_FLOW_PROFILE.test_prefix

        command = build_nextest_command(
            (
                f"{prefix}second",
                f"{prefix}first",
            ),
            profile=HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
        )

        self.assertEqual(
            [
                "cargo",
                "nextest",
                "run",
                "-p",
                "trust-hir",
                "--test",
                "semantic_type_checking",
            ],
            command[:7],
        )
        expression = command[-1]
        self.assertIn(
            "binary_id(=trust-hir::semantic_type_checking)",
            expression,
        )
        self.assertIn(
            "test(=control_flow_and_calls::first)",
            expression,
        )
        self.assertNotIn("assignments_and_var_access::", expression)
        with self.assertRaisesRegex(
            ValueError,
            "has no no-default-features shard",
        ):
            build_nextest_command(
                (f"{prefix}first",),
                no_default_features=True,
                profile=HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
            )

    def test_hir_remaining_profile_is_closed_over_reviewed_source_paths(self) -> None:
        profile = get_target_profile(
            "hir-remaining",
            path=(
                "crates/trust-hir/tests/semantic_type_checking/"
                "basics_and_warnings.rs"
            ),
        )

        self.assertEqual("trust-hir", profile.package)
        self.assertEqual("semantic_type_checking", profile.target_name)
        self.assertEqual(
            "trust-hir::semantic_type_checking",
            profile.binary_id,
        )
        self.assertEqual("basics_and_warnings::", profile.test_prefix)
        self.assertEqual(
            "trust-hir::semantic_type_checking$",
            profile.encoded_test_name_prefix,
        )
        with self.assertRaisesRegex(ValueError, "outside the closed HIR"):
            get_target_profile(
                "hir-remaining",
                path="crates/trust-hir/tests/not_reviewed.rs",
            )

    def test_syntax_remaining_profile_is_closed_over_reviewed_source_paths(self) -> None:
        self.assertIn("syntax-remaining", TARGET_PROFILES)
        integration = get_target_profile(
            "syntax-remaining",
            path="crates/trust-syntax/tests/parser_expressions.rs",
        )

        self.assertEqual("trust-syntax", integration.package)
        self.assertEqual("parser_expressions", integration.target_name)
        self.assertEqual(
            "trust-syntax::parser_expressions",
            integration.binary_id,
        )
        self.assertEqual("", integration.test_prefix)
        self.assertEqual(
            "trust-syntax::parser_expressions$",
            integration.encoded_test_name_prefix,
        )

        for excluded_path in (
            "crates/trust-syntax/src/syntax/mod.rs",
            "crates/trust-syntax/tests/parser_project_examples.rs",
            "crates/trust-syntax/tests/not_reviewed.rs",
        ):
            with self.assertRaisesRegex(ValueError, "outside the closed syntax"):
                get_target_profile(
                    "syntax-remaining",
                    path=excluded_path,
                )

    def test_wasm_analysis_profile_is_closed_over_reviewed_parity_parts(self) -> None:
        self.assertIn("wasm-analysis-remaining", TARGET_PROFILES)
        profile = get_target_profile(
            "wasm-analysis-remaining",
            path=(
                "crates/trust-wasm-analysis/tests/mp010_parity/"
                "mp010_parity_part_03.rs"
            ),
        )

        self.assertEqual("trust-wasm-analysis", profile.package)
        self.assertEqual(("--test", "mp010_parity"), profile.cargo_target_args)
        self.assertEqual(
            "trust-wasm-analysis::mp010_parity",
            profile.binary_id,
        )
        self.assertEqual("mp010_parity_part_03::", profile.test_prefix)
        self.assertEqual(
            "trust-wasm-analysis::mp010_parity$",
            profile.encoded_test_name_prefix,
        )
        with self.assertRaisesRegex(ValueError, "outside the closed WASM analysis"):
            get_target_profile(
                "wasm-analysis-remaining",
                path=(
                    "crates/trust-wasm-analysis/tests/mp010_parity/"
                    "mp010_parity_part_05.rs"
                ),
            )

    def test_ads_core_profile_is_closed_over_codec_and_symbol_paths(self) -> None:
        self.assertIn("ads-core-remaining", TARGET_PROFILES)
        mapping = get_target_profile(
            "ads-core-remaining",
            path="crates/trust-ads-core/src/mapping.rs",
        )
        symbols = get_target_profile(
            "ads-core-remaining",
            path="crates/trust-ads-core/src/symbols.rs",
        )
        quality = get_target_profile(
            "ads-core-remaining",
            path="crates/trust-ads-core/src/quality.rs",
        )

        self.assertEqual("trust-ads-core", mapping.package)
        self.assertEqual(("--lib",), mapping.cargo_target_args)
        self.assertEqual("trust-ads-core", mapping.binary_id)
        self.assertEqual("mapping::tests::", mapping.test_prefix)
        self.assertEqual("symbols::tests::", symbols.test_prefix)
        self.assertEqual("quality::tests::", quality.test_prefix)
        self.assertEqual(
            "trust-ads-core::trust_ads_core$",
            mapping.encoded_test_name_prefix,
        )
        with self.assertRaisesRegex(ValueError, "outside the closed ADS core"):
            get_target_profile(
                "ads-core-remaining",
                path="crates/trust-ads-core/src/lib.rs",
            )

    def test_debug_and_runtime_core_profiles_are_closed_over_reviewed_paths(
        self,
    ) -> None:
        debug = get_target_profile(
            "debug-remaining",
            path="crates/trust-debug/src/adapter/tests_part_03.rs",
        )
        runtime_core = get_target_profile(
            "runtime-core-remaining",
            path="crates/trust-runtime-core/src/value/reference.rs",
        )

        self.assertEqual("trust-debug", debug.package)
        self.assertEqual(
            "adapter::tests::tests_part_03::",
            debug.test_prefix,
        )
        self.assertEqual("trust-runtime-core", runtime_core.package)
        self.assertEqual(
            "value::reference::tests::",
            runtime_core.test_prefix,
        )
        with self.assertRaisesRegex(ValueError, "outside the closed debug"):
            get_target_profile(
                "debug-remaining",
                path="crates/trust-debug/src/not_reviewed.rs",
            )
        with self.assertRaisesRegex(
            ValueError,
            "outside the closed runtime-core",
        ):
            get_target_profile(
                "runtime-core-remaining",
                path="crates/trust-runtime-core/src/not_reviewed.rs",
            )

    def test_runtime_remaining_profile_resolves_reviewed_integration_path(
        self,
    ) -> None:
        profile = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/bounded_value_semantics.rs",
        )

        self.assertEqual("trust-runtime", profile.package)
        self.assertEqual(
            ("--test", "bounded_value_semantics"),
            profile.cargo_target_args,
        )
        self.assertEqual(
            "trust-runtime::bounded_value_semantics",
            profile.binary_id,
        )
        bundle_builder = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/src/host/bundle_builder/tests.rs",
        )
        self.assertEqual(("--lib",), bundle_builder.cargo_target_args)
        self.assertEqual(
            "bundle_builder::tests::",
            bundle_builder.test_prefix,
        )
        safety = get_target_profile(
            "runtime-remaining",
            path=(
                "crates/trust-runtime/tests/"
                "runtime_safety_fail_closed.rs"
            ),
        )
        self.assertEqual(
            ("--test", "runtime_safety_fail_closed"),
            safety.cargo_target_args,
        )
        st_test_cli = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/st_test_cli_command.rs",
        )
        self.assertEqual(
            ("--test", "st_test_cli_command"),
            st_test_cli.cargo_target_args,
        )
        self.assertEqual("", st_test_cli.test_prefix)
        self.assertEqual(
            "trust-runtime::runtime_safety_fail_closed",
            safety.binary_id,
        )
        simulation = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/simulation_workflow.rs",
        )
        self.assertEqual(
            ("--test", "simulation_workflow"),
            simulation.cargo_target_args,
        )
        self.assertEqual(
            "trust-runtime::simulation_workflow",
            simulation.binary_id,
        )
        plcopen_command = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/plcopen_command.rs",
        )
        self.assertEqual(
            ("--test", "plcopen_command"),
            plcopen_command.cargo_target_args,
        )
        self.assertEqual(
            "trust-runtime::plcopen_command",
            plcopen_command.binary_id,
        )

    def test_runtime_remaining_profile_resolves_host_evaluator_paths(
        self,
    ) -> None:
        expected = {
            "crates/trust-runtime/src/host/eval/expr/call/tests.rs": "eval::expr::call::tests::",
            "crates/trust-runtime/src/host/eval/tests/expr_access.rs": "eval::tests::expr_access::",
            "crates/trust-runtime/src/host/eval/tests/expr_bool.rs": "eval::tests::expr_bool::",
            "crates/trust-runtime/src/host/eval/tests/expr_coercion.rs": "eval::tests::expr_coercion::",
            "crates/trust-runtime/src/host/eval/tests/expr_full.rs": "eval::tests::expr_full::",
            "crates/trust-runtime/src/host/eval/tests/expr_literals.rs": "eval::tests::expr_literals::",
            "crates/trust-runtime/src/host/eval/tests/expr_time_ops.rs": "eval::tests::expr_time_ops::",
            "crates/trust-runtime/src/host/eval/tests/mod.rs": "eval::tests::",
            "crates/trust-runtime/src/host/eval/tests/pou_en_eno.rs": "eval::tests::pou_en_eno::",
            "crates/trust-runtime/src/host/eval/tests/pou_params.rs": "eval::tests::pou_params::",
            "crates/trust-runtime/src/host/eval/tests/reference.rs": "eval::tests::reference::",
            "crates/trust-runtime/src/host/eval/tests/stmt_basic.rs": "eval::tests::stmt_basic::",
            "crates/trust-runtime/src/host/eval/tests/stmt_case.rs": "eval::tests::stmt_case::",
            "crates/trust-runtime/src/host/eval/tests/stmt_if.rs": "eval::tests::stmt_if::",
            "crates/trust-runtime/src/host/eval/tests/stmt_loops.rs": "eval::tests::stmt_loops::",
            "crates/trust-runtime/src/host/eval/tests/stmt_return.rs": "eval::tests::stmt_return::",
            "crates/trust-runtime/src/host/instance.rs": "instance::tests::",
        }
        for path, prefix in expected.items():
            profile = get_target_profile("runtime-remaining", path=path)
            self.assertEqual(("--lib",), profile.cargo_target_args)
            self.assertEqual("trust-runtime", profile.binary_id)
            self.assertEqual(prefix, profile.test_prefix)

    def test_plcopen_remaining_profile_resolves_reviewed_unit_paths(
        self,
    ) -> None:
        for path in (
            "crates/trust-plcopen/src/plcopen/tests/export_profile.rs",
            "crates/trust-plcopen/src/plcopen/tests/roundtrip_import.rs",
            "crates/trust-plcopen/src/plcopen/tests/shims.rs",
        ):
            profile = get_target_profile(
                "plcopen-remaining",
                path=path,
            )
            self.assertEqual("trust-plcopen", profile.package)
            self.assertEqual(("--lib",), profile.cargo_target_args)
            self.assertEqual("trust-plcopen", profile.binary_id)
            self.assertEqual("tests::", profile.test_prefix)
            self.assertEqual("rust_unit_test", profile.discovery_source_kind)
        with self.assertRaisesRegex(ValueError, "outside the closed PLCopen"):
            get_target_profile(
                "plcopen-remaining",
                path="crates/trust-plcopen/src/plcopen/import.rs",
            )

    def test_runtime_remaining_profile_resolves_openot_targets(self) -> None:
        unit = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/src/openot_authoring/tests.rs",
        )
        integration = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/openot_telemetry.rs",
        )

        self.assertEqual(("--lib",), unit.cargo_target_args)
        self.assertEqual("openot_authoring::tests::", unit.test_prefix)
        self.assertEqual("rust_unit_test", unit.discovery_source_kind)
        self.assertEqual(
            ("--test", "openot_telemetry"),
            integration.cargo_target_args,
        )
        self.assertEqual(
            "trust-runtime::openot_telemetry",
            integration.binary_id,
        )
        with self.assertRaisesRegex(ValueError, "outside the closed runtime"):
            get_target_profile(
                "runtime-remaining",
                path="crates/trust-runtime/src/not_reviewed.rs",
            )

    def test_runtime_remaining_profile_resolves_memory_unit_target(self) -> None:
        profile = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/src/memory/tests.rs",
        )

        self.assertEqual(("--lib",), profile.cargo_target_args)
        self.assertEqual("trust-runtime", profile.binary_id)
        self.assertEqual("memory::tests::", profile.test_prefix)

    def test_runtime_remaining_profile_resolves_evaluator_unit_targets(
        self,
    ) -> None:
        expected = {
            "crates/trust-runtime/src/host/eval/tests/errors.rs": (
                "eval::tests::errors::"
            ),
            "crates/trust-runtime/src/host/eval/tests/expr_ops.rs": (
                "eval::tests::expr_ops::"
            ),
            "crates/trust-runtime/src/host/eval/tests/pou_fb.rs": (
                "eval::tests::pou_fb::"
            ),
            "crates/trust-runtime/src/host/eval/tests/pou_function.rs": (
                "eval::tests::pou_function::"
            ),
            "crates/trust-runtime/src/host/helper_eval/const_expr.rs": (
                "helper_eval::const_expr::tests::"
            ),
            "crates/trust-runtime/src/host/helper_eval/storage_expr.rs": (
                "helper_eval::storage_expr::tests::"
            ),
            "crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs": (
                "helper_eval::storage_lvalue::tests::"
            ),
        }
        for path, prefix in expected.items():
            profile = get_target_profile("runtime-remaining", path=path)
            self.assertEqual(("--lib",), profile.cargo_target_args)
            self.assertEqual("trust-runtime", profile.binary_id)
            self.assertEqual(prefix, profile.test_prefix)

    def test_dev_remaining_profile_resolves_catalog_unit_targets(self) -> None:
        expected = {
            "crates/trust-dev/src/agent.rs": "agent::tests::",
            "crates/trust-dev/src/ci.rs": "ci::tests::",
            "crates/trust-dev/src/commit.rs": "commit::tests::",
            "crates/trust-dev/src/docs/tests.rs": "docs::tests::",
            "crates/trust-dev/src/test_cmd/tests.rs": "test::tests::",
        }
        for path, prefix in expected.items():
            profile = get_target_profile("dev-remaining", path=path)
            self.assertEqual(("--bin", "trust-dev"), profile.cargo_target_args)
            self.assertEqual("trust-dev", profile.package)
            self.assertEqual("trust-dev::bin/trust-dev", profile.binary_id)
            self.assertEqual(prefix, profile.test_prefix)
            self.assertEqual(
                "trust-dev::trust-dev$",
                profile.encoded_test_name_prefix,
            )
            self.assertEqual(frozenset({"#[cfg(unix)]"}), profile.allowed_cfg_attributes)

    def test_runtime_remaining_profile_resolves_mesh_unit_target(self) -> None:
        profile = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/src/host/mesh/tests.rs",
        )
        self.assertEqual(("--lib",), profile.cargo_target_args)
        self.assertEqual("trust-runtime", profile.binary_id)
        self.assertEqual("mesh::tests::", profile.test_prefix)

    def test_runtime_remaining_profile_resolves_runner_loop_unit_target(
        self,
    ) -> None:
        profile = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/src/scheduler/runner_loop.rs",
        )
        self.assertEqual(("--lib",), profile.cargo_target_args)
        self.assertEqual("trust-runtime", profile.binary_id)
        self.assertEqual(
            "scheduler::runner_loop_poison_tests::",
            profile.test_prefix,
        )

    def test_runtime_remaining_profile_resolves_realtime_t0_targets(
        self,
    ) -> None:
        for path in (
            "crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs",
            "crates/trust-runtime/src/host/realtime/realtime_tests_part_02.rs",
            "crates/trust-runtime/src/host/realtime/realtime_tests_part_03.rs",
        ):
            with self.subTest(path=path):
                profile = get_target_profile("runtime-remaining", path=path)
                self.assertEqual(("--lib",), profile.cargo_target_args)
                self.assertEqual("trust-runtime", profile.binary_id)
                self.assertEqual("realtime::tests::", profile.test_prefix)

        integration = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/realtime_t0_integration.rs",
        )
        self.assertEqual(
            ("--test", "realtime_t0_integration"),
            integration.cargo_target_args,
        )
        self.assertEqual(
            "trust-runtime::realtime_t0_integration",
            integration.binary_id,
        )
        self.assertEqual("", integration.test_prefix)

    def test_runtime_remaining_profile_resolves_debugger_batch_targets(
        self,
    ) -> None:
        unit_targets = {
            "crates/trust-runtime/src/host/debug/control/tests.rs": (
                "debug::control::tests::"
            ),
            "crates/trust-runtime/src/host/debug/resolve.rs": (
                "debug::resolve::tests::"
            ),
        }
        for path, prefix in unit_targets.items():
            with self.subTest(path=path):
                profile = get_target_profile("runtime-remaining", path=path)
                self.assertEqual(("--lib",), profile.cargo_target_args)
                self.assertEqual("trust-runtime", profile.binary_id)
                self.assertEqual(prefix, profile.test_prefix)

        integration = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/debug_stepping.rs",
        )
        self.assertEqual(
            ("--test", "debug_stepping"),
            integration.cargo_target_args,
        )
        self.assertEqual(
            "trust-runtime::debug_stepping",
            integration.binary_id,
        )
        self.assertEqual("", integration.test_prefix)

    def test_runtime_remaining_profile_resolves_tasks_integration_target(
        self,
    ) -> None:
        profile = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/tasks.rs",
        )
        self.assertEqual(("--test", "tasks"), profile.cargo_target_args)
        self.assertEqual("trust-runtime::tasks", profile.binary_id)
        self.assertEqual("", profile.test_prefix)

    def test_ads_server_profile_resolves_ams_unit_target(self) -> None:
        profile = get_target_profile(
            "ads-server-remaining",
            path="crates/trust-ads-server/src/ams.rs",
        )
        self.assertEqual("trust-ads-server", profile.package)
        self.assertEqual(("--lib",), profile.cargo_target_args)
        self.assertEqual("trust-ads-server", profile.binary_id)
        self.assertEqual("ams::tests::", profile.test_prefix)
        self.assertEqual("rust_unit_test", profile.discovery_source_kind)

    def test_ads_server_profile_resolves_notification_unit_target(self) -> None:
        profile = get_target_profile(
            "ads-server-remaining",
            path="crates/trust-ads-server/src/notify.rs",
        )
        self.assertEqual("trust-ads-server", profile.package)
        self.assertEqual(("--lib",), profile.cargo_target_args)
        self.assertEqual("notify::tests::", profile.test_prefix)

    def test_ads_server_profile_resolves_listener_unit_target(self) -> None:
        profile = get_target_profile(
            "ads-server-remaining",
            path="crates/trust-ads-server/src/listener.rs",
        )
        self.assertEqual("trust-ads-server", profile.package)
        self.assertEqual(("--lib",), profile.cargo_target_args)
        self.assertEqual("listener::tests::", profile.test_prefix)

    def test_ads_server_profile_resolves_symbols_unit_target(self) -> None:
        profile = get_target_profile(
            "ads-server-remaining",
            path="crates/trust-ads-server/src/symbols.rs",
        )
        self.assertEqual("trust-ads-server", profile.package)
        self.assertEqual(("--lib",), profile.cargo_target_args)
        self.assertEqual("symbols::tests::", profile.test_prefix)

    def test_verification_cases_profile_resolves_library_target(self) -> None:
        profile = get_target_profile(
            "verification-cases-remaining",
            path="crates/verification-cases/src/lib.rs",
        )
        self.assertEqual("verification-cases", profile.package)
        self.assertEqual(("--lib",), profile.cargo_target_args)
        self.assertEqual("verification-cases", profile.binary_id)
        self.assertEqual("tests::", profile.test_prefix)

    def test_runtime_remaining_profile_resolves_retain_targets(self) -> None:
        unit = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/src/retain/store_tests.rs",
        )
        integration = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/retain_integrity.rs",
        )

        self.assertEqual(("--lib",), unit.cargo_target_args)
        self.assertEqual("retain::", unit.test_prefix)
        self.assertEqual(
            ("--test", "retain_integrity"),
            integration.cargo_target_args,
        )
        self.assertEqual(
            "trust-runtime::retain_integrity",
            integration.binary_id,
        )

    def test_runtime_remaining_profile_resolves_bytecode_targets(self) -> None:
        self.assertIn("runtime-remaining", TARGET_PROFILES)
        unit = get_target_profile(
            "runtime-remaining",
            path=(
                "crates/trust-runtime/src/bytecode/validate/"
                "resource_limits.rs"
            ),
        )
        nested = get_target_profile(
            "runtime-remaining",
            path=(
                "crates/trust-runtime/tests/bytecode_encoder/"
                "bytecode_encoder_part_03.rs"
            ),
        )
        integration = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/tests/bytecode_sections.rs",
        )

        self.assertEqual(("--lib",), unit.cargo_target_args)
        self.assertEqual(
            "bytecode::validate::resource_limits::tests::",
            unit.test_prefix,
        )
        self.assertEqual(
            ("--test", "bytecode_encoder"),
            nested.cargo_target_args,
        )
        self.assertEqual("bytecode_encoder_part_03::", nested.test_prefix)
        self.assertEqual(
            ("--test", "bytecode_sections"),
            integration.cargo_target_args,
        )

    def test_runtime_remaining_profile_resolves_stdlib_batch_paths(self) -> None:
        paths = {
            "crates/trust-runtime/tests/datetime_profile.rs",
            "crates/trust-runtime/tests/datetime_range.rs",
            "crates/trust-runtime/tests/datetime_types.rs",
            "crates/trust-runtime/tests/expr_calls.rs",
            "crates/trust-runtime/tests/fb_bistable.rs",
            "crates/trust-runtime/tests/fb_counters.rs",
            "crates/trust-runtime/tests/fb_counters_full.rs",
            "crates/trust-runtime/tests/fb_edges.rs",
            "crates/trust-runtime/tests/fb_timers.rs",
            "crates/trust-runtime/tests/iec_counters.rs",
            "crates/trust-runtime/tests/iec_examples.rs",
            "crates/trust-runtime/tests/iec_timers.rs",
            "crates/trust-runtime/tests/stdlib_assertions.rs",
            "crates/trust-runtime/tests/stdlib_bit_full.rs",
            "crates/trust-runtime/tests/stdlib_conv.rs",
            "crates/trust-runtime/tests/stdlib_conv_full.rs",
            "crates/trust-runtime/tests/stdlib_enum_validate.rs",
            "crates/trust-runtime/tests/stdlib_numeric.rs",
            "crates/trust-runtime/tests/stdlib_select_full.rs",
            "crates/trust-runtime/tests/stdlib_split_locals.rs",
            "crates/trust-runtime/tests/stdlib_string.rs",
            "crates/trust-runtime/tests/stdlib_string_full.rs",
        }
        for path in paths:
            with self.subTest(path=path):
                profile = get_target_profile("runtime-remaining", path=path)
                target = path.rsplit("/", 1)[-1].removesuffix(".rs")
                self.assertEqual(("test", target), (profile.target_kind, profile.target_name))
                self.assertEqual("", profile.test_prefix)

    def test_hir_selection_requires_integration_discovery_kind(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            verification = root / "verification"
            verification.mkdir()
            _write_toml(
                verification / "test-catalog-denominator.toml",
                """
                [[reviews]]
                discovery_id = "DISC_HIR"
                discovery_source_kind = "rust_integration_test"
                path = "tests/control_flow.rs"
                name = "one"
                disposition = "reviewed_nonmapping"
                ignore_state = "not_ignored"
                """,
            )
            _write_toml(
                verification / "test-catalog.toml",
                "tests = []",
            )

            selection = derive_frozen_selection(
                root=root,
                path="tests/control_flow.rs",
                expected_reviewed_nonmapping=1,
                expected_catalog_mapped=0,
                profile=HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
            )

            self.assertEqual(
                ("control_flow_and_calls::one",),
                selection.full_names,
            )

    def test_feature_shards_are_derived_from_exact_source_attributes(self) -> None:
        names = (
            f"{TEST_NAME_PREFIX}default_case",
            f"{TEST_NAME_PREFIX}without_wire",
            f"{TEST_NAME_PREFIX}server_case",
        )
        shards = split_feature_shards(
            source=f"""
            #[test]
            fn default_case() {{}}

            {ADS_WIRE_DISABLED_CFG}
            #[test]
            fn without_wire() {{}}

            #[cfg(feature = "ads-server")]
            #[test]
            fn server_case() {{}}
            """,
            expected_full_names=names,
        )

        self.assertEqual(
            ("default", "no-ads-wire"),
            tuple(shard.name for shard in shards),
        )
        self.assertEqual(
            (
                f"{TEST_NAME_PREFIX}default_case",
                f"{TEST_NAME_PREFIX}server_case",
            ),
            shards[0].full_names,
        )
        self.assertEqual(
            (f"{TEST_NAME_PREFIX}without_wire",),
            shards[1].full_names,
        )
        self.assertTrue(shards[1].no_default_features)
        self.assertIn(
            "--no-default-features",
            build_nextest_command(
                shards[1].full_names,
                no_default_features=shards[1].no_default_features,
            ),
        )

    def test_feature_shards_fail_closed_on_unknown_cfg(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsupported cfg attributes"):
            split_feature_shards(
                source="""
                #[cfg(unix)]
                #[test]
                fn platform_case() {}
                """,
                expected_full_names=(f"{TEST_NAME_PREFIX}platform_case",),
            )

    def test_hir_profile_rejects_every_cfg_shard(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsupported cfg attributes"):
            split_feature_shards(
                source="""
                #[cfg(unix)]
                #[test]
                fn platform_case() {}
                """,
                expected_full_names=(
                    "control_flow_and_calls::platform_case",
                ),
                profile=HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
            )

    def test_normalizer_records_exact_passing_set_and_milliseconds(self) -> None:
        raw = "\n".join(
            [
                json.dumps(
                    {"type": "suite", "event": "started", "test_count": 2}
                ),
                json.dumps(
                    {
                        "type": "test",
                        "event": "started",
                        "name": f"{TEST_NAME_PREFIX}first",
                    }
                ),
                json.dumps(
                    {
                        "type": "test",
                        "event": "ok",
                        "name": f"{TEST_NAME_PREFIX}first",
                        "exec_time": 0.0012,
                    }
                ),
                json.dumps(
                    {
                        "type": "test",
                        "event": "ok",
                        "name": f"{TEST_NAME_PREFIX}second",
                        "exec_time": 0,
                    }
                ),
                json.dumps({"type": "suite", "event": "ok"}),
            ]
        )
        tests = normalize_nextest_jsonl(
            raw,
            expected_full_names={
                f"{TEST_NAME_PREFIX}first",
                f"{TEST_NAME_PREFIX}second",
            },
        )

        self.assertEqual(
            [
                {"title": "first", "duration": 2, "state": "passed"},
                {"title": "second", "duration": 0, "state": "passed"},
            ],
            tests,
        )

    def test_normalizer_accepts_nextest_encoded_library_name(self) -> None:
        raw = json.dumps(
            {
                "type": "test",
                "event": "ok",
                "name": (
                    "trust-runtime::trust_runtime$"
                    f"{TEST_NAME_PREFIX}encoded"
                ),
                "exec_time": 0.001,
            }
        )

        self.assertEqual(
            [{"title": "encoded", "duration": 1, "state": "passed"}],
            normalize_nextest_jsonl(
                raw,
                expected_full_names={f"{TEST_NAME_PREFIX}encoded"},
            ),
        )

    def test_normalizer_accepts_only_hir_profile_encoded_name(self) -> None:
        raw = json.dumps(
            {
                "type": "test",
                "event": "ok",
                "name": (
                    "trust-hir::semantic_type_checking$"
                    "control_flow_and_calls::one"
                ),
                "exec_time": 0.001,
            }
        )

        self.assertEqual(
            [{"title": "one", "duration": 1, "state": "passed"}],
            normalize_nextest_jsonl(
                raw,
                expected_full_names={"control_flow_and_calls::one"},
                profile=HIR_SEMANTIC_CONTROL_FLOW_PROFILE,
            ),
        )
        with self.assertRaisesRegex(ValueError, "outside required prefix"):
            normalize_nextest_jsonl(
                raw,
                expected_full_names={f"{TEST_NAME_PREFIX}one"},
            )

    def test_normalizer_rejects_missing_unexpected_duplicate_and_nonpass(self) -> None:
        expected = {f"{TEST_NAME_PREFIX}first"}

        with self.assertRaisesRegex(ValueError, "missing test results"):
            normalize_nextest_jsonl("", expected_full_names=expected)
        with self.assertRaisesRegex(ValueError, "outside required prefix"):
            normalize_nextest_jsonl(
                json.dumps(
                    {
                        "type": "test",
                        "event": "ok",
                        "name": "other::first",
                        "exec_time": 0.1,
                    }
                ),
                expected_full_names=expected,
            )
        unexpected = json.dumps(
            {
                "type": "test",
                "event": "ok",
                "name": f"{TEST_NAME_PREFIX}second",
                "exec_time": 0.1,
            }
        )
        with self.assertRaisesRegex(ValueError, "unexpected test result"):
            normalize_nextest_jsonl(
                unexpected,
                expected_full_names=expected,
            )
        passing = json.dumps(
            {
                "type": "test",
                "event": "ok",
                "name": f"{TEST_NAME_PREFIX}first",
                "exec_time": 0.1,
            }
        )
        with self.assertRaisesRegex(ValueError, "duplicate terminal result"):
            normalize_nextest_jsonl(
                passing + "\n" + passing,
                expected_full_names=expected,
            )
        failed = json.dumps(
            {
                "type": "test",
                "event": "failed",
                "name": f"{TEST_NAME_PREFIX}first",
                "exec_time": 0.1,
            }
        )
        with self.assertRaisesRegex(ValueError, "did not pass"):
            normalize_nextest_jsonl(
                failed,
                expected_full_names=expected,
            )

    def test_normalized_document_matches_rust_catalog_compiler_contract(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_path = Path("crates/trust-runtime/src/control/tests/core.rs")
            raw_path = root / "target/gate-artifacts/control-core.jsonl"
            contents = {
                source_path: b"#[test]\nfn first() {}\n",
                DENOMINATOR_PATH: b"reviews = []\n",
                CATALOG_PATH: b"tests = []\n",
                RUNNER_PATH: b"runner module\n",
                SHIM_PATH: b"runner shim\n",
                raw_path.relative_to(root): (
                    json.dumps(
                        {
                            "type": "test",
                            "event": "ok",
                            "name": f"{TEST_NAME_PREFIX}first",
                            "exec_time": 0.001,
                        }
                    ).encode()
                    + b"\n"
                ),
            }
            for relative, content in contents.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(content)
            selection = FrozenSelection(
                path=source_path.as_posix(),
                reviewed_nonmapping_count=1,
                catalog_mapped_count=0,
                names=("first",),
                full_names=(f"{TEST_NAME_PREFIX}first",),
            )
            command = build_nextest_command(selection.full_names)
            selected_digest = _sha256(
                json.dumps(
                    sorted(selection.full_names),
                    ensure_ascii=False,
                    separators=(",", ":"),
                ).encode()
            )
            metadata = {
                "schema_version": 1,
                "command": shlex.join(command),
                "argv": command,
                "environment": {"NEXTEST_EXPERIMENTAL_LIBTEST_JSON": "1"},
                "nextest_version": "cargo-nextest 0.9.140",
                "exit_code": 0,
                "started_at": "2026-07-26T00:00:00Z",
                "finished_at": "2026-07-26T00:00:01Z",
                "path": source_path.as_posix(),
                "reviewed_nonmapping_count": 1,
                "catalog_mapped_count": 0,
                "selected_test_count": 1,
                "selected_names_sha256": selected_digest,
                "source_sha256": _sha256(contents[source_path]),
                "denominator_sha256": _sha256(contents[DENOMINATOR_PATH]),
                "catalog_sha256": _sha256(contents[CATALOG_PATH]),
                "module_path": RUNNER_PATH.as_posix(),
                "module_sha256": _sha256(contents[RUNNER_PATH]),
                "shim_path": SHIM_PATH.as_posix(),
                "shim_sha256": _sha256(contents[SHIM_PATH]),
                "raw_results_path": raw_path.relative_to(root).as_posix(),
                "raw_results_sha256": _sha256(
                    contents[raw_path.relative_to(root)]
                ),
            }

            document = build_normalized_document(
                root=root,
                selection=selection,
                raw_path=raw_path,
                metadata=metadata,
            )

            self.assertEqual(
                {
                    "kind",
                    "batch_command",
                    "runner_path",
                    "runner_sha256",
                    "source_path",
                    "source_sha256",
                    "raw_results_path",
                    "raw_results_sha256",
                    "nextest_version",
                    "package",
                    "target_kind",
                    "binary_id",
                    "test_prefix",
                },
                set(document["execution"]),
            )
            self.assertEqual(
                {
                    "tests": 1,
                    "passes": 1,
                    "failures": 0,
                    "pending": 0,
                },
                document["stats"],
            )

    def test_normalize_only_reuses_combined_evidence_after_ledger_apply(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_path = Path(
                "crates/trust-runtime/src/control/tests/core.rs"
            )
            raw_path = root / "target/gate-artifacts/combined.jsonl"
            metadata_path = raw_path.with_suffix(".execution.json")
            source_content = b"#[test]\nfn first() {}\n"
            raw_content = (
                json.dumps(
                    {
                        "type": "test",
                        "event": "ok",
                        "name": f"{TEST_NAME_PREFIX}first",
                        "exec_time": 0.001,
                    }
                ).encode()
                + b"\n"
            )
            initial_denominator = b"reviews = []\n"
            initial_catalog = b"tests = []\n"
            contents = {
                source_path: source_content,
                DENOMINATOR_PATH: initial_denominator,
                CATALOG_PATH: initial_catalog,
                RUNNER_PATH: b"runner module\n",
                SHIM_PATH: b"runner shim\n",
                raw_path.relative_to(root): raw_content,
            }
            for relative, content in contents.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(content)

            full_names = (f"{TEST_NAME_PREFIX}first",)
            command = build_nextest_command(full_names)
            selected_digest = _sha256(
                json.dumps(
                    list(full_names),
                    ensure_ascii=False,
                    separators=(",", ":"),
                ).encode()
            )
            missing_shard = Path(
                "target/gate-artifacts/combined.default.jsonl"
            )
            metadata = {
                "schema_version": 2,
                "command": shlex.join(command),
                "argv": [command],
                "environment": {
                    "NEXTEST_EXPERIMENTAL_LIBTEST_JSON": "1"
                },
                "nextest_version": "cargo-nextest 0.9.140",
                "exit_code": 0,
                "started_at": "2026-07-26T00:00:00Z",
                "finished_at": "2026-07-26T00:00:01Z",
                "path": source_path.as_posix(),
                "reviewed_nonmapping_count": 1,
                "catalog_mapped_count": 0,
                "selected_test_count": 1,
                "selected_names_sha256": selected_digest,
                "source_sha256": _sha256(source_content),
                "denominator_sha256": _sha256(initial_denominator),
                "catalog_sha256": _sha256(initial_catalog),
                "module_path": RUNNER_PATH.as_posix(),
                "module_sha256": _sha256(contents[RUNNER_PATH]),
                "shim_path": SHIM_PATH.as_posix(),
                "shim_sha256": _sha256(contents[SHIM_PATH]),
                "raw_results_path": raw_path.relative_to(root).as_posix(),
                "raw_results_sha256": _sha256(raw_content),
                "shards": [
                    {
                        "name": "default",
                        "command": shlex.join(command),
                        "argv": command,
                        "selected_test_count": 1,
                        "selected_names_sha256": selected_digest,
                        "raw_results_path": missing_shard.as_posix(),
                        "raw_results_sha256": "sha256:" + "0" * 64,
                        "exit_code": 0,
                        "started_at": "2026-07-26T00:00:00Z",
                        "finished_at": "2026-07-26T00:00:01Z",
                    }
                ],
            }
            metadata_path.write_text(json.dumps(metadata))
            (root / DENOMINATOR_PATH).write_text(
                '[[reviews]]\ndisposition = "catalog_mapped"\n'
            )
            (root / CATALOG_PATH).write_text(
                '[[tests]]\nstatus = "mapped"\n'
            )

            selection = derive_frozen_selection_from_evidence(
                root=root,
                path=source_path.as_posix(),
                raw_path=raw_path,
                metadata=metadata,
            )
            document = build_normalized_document(
                root=root,
                selection=selection,
                raw_path=raw_path,
                metadata=metadata,
                reuse_frozen_evidence=True,
            )

            self.assertEqual(
                [{"title": "first", "duration": 1, "state": "passed"}],
                document["tests"],
            )
            self.assertFalse((root / missing_shard).exists())
            results_path = Path("target/gate-artifacts/reused.json")
            self.assertEqual(
                0,
                main(
                    [
                        "--root",
                        str(root),
                        "--path",
                        source_path.as_posix(),
                        "--raw-results",
                        raw_path.relative_to(root).as_posix(),
                        "--execution-metadata",
                        metadata_path.relative_to(root).as_posix(),
                        "--results",
                        results_path.as_posix(),
                        "--normalize-only",
                    ]
                ),
            )
            self.assertTrue((root / results_path).is_file())


if __name__ == "__main__":
    unittest.main()
