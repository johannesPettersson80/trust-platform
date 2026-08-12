from __future__ import annotations

import unittest

from scripts.verification.frozen_rust_nextest_target import (
    get_target_profile,
    split_feature_shards,
)


class FrozenRustNextestAuthorityTargetTests(unittest.TestCase):
    def test_runtime_include_and_path_module_owners_are_exact(self) -> None:
        expected = {
            "crates/trust-runtime/src/config/contracts/contract_tests.rs":
                "config::contract_tests::",
            "crates/trust-runtime/src/control/tests/control_endpoint_source_contract.rs":
                "control::tests::",
            "crates/trust-runtime/src/control/tests/control_request_boundary_contract.rs":
                "control::tests::",
            "crates/trust-runtime/src/host/boundary/error_contract_tests.rs":
                "boundary::error::contract_tests::",
            "crates/trust-runtime/src/host/boundary/protocol_envelope_contract_tests.rs":
                "boundary::protocol_envelope::contract_tests::",
            "crates/trust-runtime/src/host/boundary/resolver_contract_tests.rs":
                "boundary::resolver::contract_tests::",
            "crates/trust-runtime/src/host/debug/breakpoints/contract_tests.rs":
                "debug::breakpoints::contract_tests::",
            "crates/trust-runtime/src/host/debug/control/hook/contract_tests.rs":
                "debug::control::contract_tests::",
            "crates/trust-runtime/src/host/debug/control/tests/lifecycle_contract.rs":
                "debug::control::tests::lifecycle_contract::",
            "crates/trust-runtime/src/host/debug/control/tests/mutation_contract.rs":
                "debug::control::tests::mutation_contract::",
            "crates/trust-runtime/src/host/debug/control/tests/state_stream_contract.rs":
                "debug::control::tests::state_stream_contract::",
            "crates/trust-runtime/src/host/debug/dap/contract_tests.rs":
                "debug::dap::contract_tests::",
            "crates/trust-runtime/src/host/debug/hook/contract_tests.rs":
                "debug::hook::contract_tests::",
            "crates/trust-runtime/src/host/debug/resolve/contract_tests.rs":
                "debug::resolve::contract_tests::",
            "crates/trust-runtime/src/host/debug/trace/contract_tests.rs":
                "debug::trace::contract_tests::",
            "crates/trust-runtime/src/host/debug/types/contract_tests.rs":
                "debug::types::contract_tests::",
            "crates/trust-runtime/src/host/harness/api_contract_tests.rs":
                "harness::api::contract_tests::",
            "crates/trust-runtime/src/host/harness/coerce_contract_tests.rs":
                "harness::coerce::contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/action_profile_contract_tests.rs":
                "harness::compiler::action_profile_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/constant_initializer_contract_tests.rs":
                "harness::compiler::constant_initializer_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/default_initializer_dependency_contract_tests.rs":
                "harness::compiler::default_initializer_dependency_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/derived_type_constant_contract_tests.rs":
                "harness::compiler::derived_type_constant_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/generic_type_profile_contract_tests.rs":
                "harness::compiler::generic_type_profile_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/pou_call_binding_runtime_contract_tests.rs":
                "harness::compiler::pou_call_binding_runtime_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/pou_function_contract_tests.rs":
                "harness::compiler::pou_function_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/pou_initializer_dependency_contract_tests.rs":
                "harness::compiler::pou_initializer_dependency_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/pou_initializer_lifetime_contract_tests.rs":
                "harness::compiler::pou_initializer_lifetime_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/pou_member_access_contract_tests.rs":
                "harness::compiler::pou_member_access_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/pou_object_contract_tests.rs":
                "harness::compiler::pou_object_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/pou_variable_qualifier_projection_contract_tests.rs":
                "harness::compiler::pou_variable_qualifier_projection_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/pou_variable_section_acceptance_contract_tests.rs":
                "harness::compiler::pou_variable_section_acceptance_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/pou_variable_section_rejection_contract_tests.rs":
                "harness::compiler::pou_variable_section_rejection_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/project_assembly_contract_tests.rs":
                "harness::compiler::project_assembly_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/type_contract_tests.rs":
                "harness::compiler::type_contract_tests::",
            "crates/trust-runtime/src/host/harness/compiler/variable_contract_tests.rs":
                "harness::compiler::variable_contract_tests::",
            "crates/trust-runtime/src/host/harness/config_access_contract_tests.rs":
                "harness::build::access_contract_tests::",
            "crates/trust-runtime/src/host/harness/config_contract_tests.rs":
                "harness::build::contract_tests::",
            "crates/trust-runtime/src/host/harness/harness_contract_tests.rs":
                "harness::harness::contract_tests::",
            "crates/trust-runtime/src/host/harness/initializer_contract_tests.rs":
                "harness::initializer::contract_tests::",
            "crates/trust-runtime/src/host/harness/io/io_contract_tests.rs":
                "harness::io::io_contract_tests::",
            "crates/trust-runtime/src/host/harness/lower/expression_contract_tests.rs":
                "harness::lower::expression_contract_tests::",
            "crates/trust-runtime/src/host/harness/lower/expression_operator_runtime_contract_tests.rs":
                "harness::lower::expression_operator_runtime_contract_tests::",
            "crates/trust-runtime/src/host/harness/lower/partial_access_runtime_contract_tests.rs":
                "harness::lower::partial_access_runtime_contract_tests::",
            "crates/trust-runtime/src/host/harness/lower/reference_runtime_contract_tests.rs":
                "harness::lower::reference_runtime_contract_tests::",
            "crates/trust-runtime/src/host/harness/lower/runtime_clock_source_contract_tests.rs":
                "harness::lower::runtime_clock_source_contract_tests::",
            "crates/trust-runtime/src/host/harness/lower/statement_contract_tests.rs":
                "harness::lower::statement_contract_tests::",
            "crates/trust-runtime/src/host/harness/lower/statement_control_flow_contract_tests.rs":
                "harness::lower::statement_control_flow_contract_tests::",
            "crates/trust-runtime/src/host/harness/lower/string_index_runtime_contract_tests.rs":
                "harness::lower::string_index_runtime_contract_tests::",
            "crates/trust-runtime/src/host/harness/parse_contract_tests.rs":
                "harness::parse::contract_tests::",
            "crates/trust-runtime/src/host/harness/protocol_contract_tests.rs":
                "harness::protocol::contract_tests::",
            "crates/trust-runtime/src/host/helper_eval/const_expr/contract_tests.rs":
                "helper_eval::const_expr::contract_tests::",
            "crates/trust-runtime/src/host/helper_eval/storage_expr/contract_tests.rs":
                "helper_eval::storage_expr::contract_tests::",
            "crates/trust-runtime/src/host/helper_eval/storage_lvalue/contract_tests.rs":
                "helper_eval::storage_lvalue::contract_tests::",
            "crates/trust-runtime/src/host/metrics_contract_tests.rs":
                "metrics::metrics_contract_tests::",
            "crates/trust-runtime/src/host/registry/registry_integrity_contract_tests.rs":
                "registry::tests::",
            "crates/trust-runtime/src/host/security/contract_tests.rs":
                "security::contract_tests::",
            "crates/trust-runtime/src/host/security/pairing/contract_tests.rs":
                "security::pairing::contract_tests::",
            "crates/trust-runtime/src/host/simulation_contract_tests.rs":
                "simulation::contract_tests::",
            "crates/trust-runtime/src/host/ui/commands/contract_tests.rs":
                "ui::commands::contract_tests::",
            "crates/trust-runtime/src/host/ui/console_contract_tests.rs":
                "ui::console_contract_tests::",
            "crates/trust-runtime/src/io/gpio_contract_tests.rs":
                "io::gpio::contract_tests::",
            "crates/trust-runtime/src/io/modbus/tests/point_map_contract.rs":
                "io::modbus::point_map::tests::",
            "crates/trust-runtime/src/io/mqtt/tests/point_map_contract.rs": (
                "io::mqtt::point_map_nonfinite_tests::"
            ),
            "crates/trust-runtime/src/io/registry_contract_tests.rs": (
                "io::registry::contract_tests::"
            ),
        }

        for path, prefix in expected.items():
            with self.subTest(path=path):
                profile = get_target_profile("runtime-remaining", path=path)
                self.assertEqual("lib", profile.target_kind)
                self.assertEqual("trust-runtime", profile.binary_id)
                self.assertEqual(prefix, profile.test_prefix)

    def test_plcopen_include_paths_share_the_tests_wrapper(self) -> None:
        paths = [
            "crates/trust-plcopen/src/plcopen/tests/contracts.rs",
            "crates/trust-plcopen/src/plcopen/tests/global_var_contract.rs",
            "crates/trust-plcopen/src/plcopen/tests/project_model_contract.rs",
            "crates/trust-plcopen/src/plcopen/tests/semantic_export_contract.rs",
            "crates/trust-plcopen/src/plcopen/tests/semantic_import_contract.rs",
            "crates/trust-plcopen/src/plcopen/tests/shims_metrics.rs",
            "crates/trust-plcopen/src/plcopen/tests/transaction_contract.rs",
        ]

        for path in paths:
            with self.subTest(path=path):
                profile = get_target_profile("plcopen-remaining", path=path)
                self.assertEqual("lib", profile.target_kind)
                self.assertIsNone(profile.target_name)
                self.assertEqual("trust-plcopen", profile.binary_id)
                self.assertEqual("tests::", profile.test_prefix)
                self.assertEqual(
                    "trust-plcopen::trust_plcopen$",
                    profile.encoded_test_name_prefix,
                )

    def test_path_attribute_nested_under_tests_module_keeps_wrapper(self) -> None:
        profile = get_target_profile(
            "debug-remaining",
            path=(
                "crates/trust-debug/src/adapter/"
                "lifecycle_contract_tests.rs"
            ),
        )

        self.assertEqual(
            "adapter::tests::lifecycle_contract_tests::",
            profile.test_prefix,
        )

    def test_harness_contract_is_nested_with_private_helper_owner(self) -> None:
        profile = get_target_profile(
            "dev-remaining",
            path=(
                "crates/trust-dev/src/agent/"
                "harness_contract_tests.rs"
            ),
        )

        self.assertEqual(
            "agent::harness::harness_contract_tests::",
            profile.test_prefix,
        )

    def test_symbol_table_contract_is_nested_with_private_table_owner(self) -> None:
        profile = get_target_profile(
            "hir-remaining",
            path="crates/trust-hir/src/symbols/table_contract_tests.rs",
        )

        self.assertEqual(
            "symbols::table::contract_tests::",
            profile.test_prefix,
        )

    def test_contract_module_path_has_exact_unit_prefix(self) -> None:
        profile = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/src/control/auth/contract_tests.rs",
        )

        self.assertEqual("lib", profile.target_kind)
        self.assertEqual(
            "control::auth::contract_tests::",
            profile.test_prefix,
        )

    def test_inline_test_module_is_appended_to_source_module(self) -> None:
        profile = get_target_profile(
            "runtime-remaining",
            path="crates/trust-runtime/src/bytecode/encoder/codegen.rs",
        )

        self.assertEqual(
            "bytecode::encoder::codegen::tests::",
            profile.test_prefix,
        )

    def test_nested_hir_integration_path_selects_parent_target(self) -> None:
        profile = get_target_profile(
            "hir-remaining",
            path=(
                "crates/trust-hir/tests/semantic_type_checking/"
                "call_binding_contract_acceptance.rs"
            ),
        )

        self.assertEqual("test", profile.target_kind)
        self.assertEqual("semantic_type_checking", profile.target_name)
        self.assertEqual(
            "call_binding_contract_acceptance::",
            profile.test_prefix,
        )

    def test_top_level_syntax_integration_path_selects_own_target(self) -> None:
        profile = get_target_profile(
            "syntax-remaining",
            path="crates/trust-syntax/tests/lexer_lexical_contract.rs",
        )

        self.assertEqual("lexer_lexical_contract", profile.target_name)
        self.assertEqual("", profile.test_prefix)

    def test_runtime_feature_guard_gets_a_closed_no_default_shard(self) -> None:
        profile = get_target_profile(
            "runtime-remaining",
            path=(
                "crates/trust-runtime/src/control/comm_handlers/"
                "capability_contract_tests.rs"
            ),
        )
        full_name = (
            "control::comm_handlers::capability_contract_tests::"
            "disabled_ads_case"
        )
        enabled_name = (
            "control::comm_handlers::capability_contract_tests::"
            "enabled_ads_case"
        )
        disabled_opcua_name = (
            "control::comm_handlers::capability_contract_tests::"
            "disabled_opcua_case"
        )

        shards = split_feature_shards(
            profile=profile,
            source=(
                '#[cfg(not(feature = "ads-wire"))]\n'
                "#[test]\n"
                "fn disabled_ads_case() {}\n"
                '#[cfg(feature = "ads-wire")]\n'
                "#[test]\n"
                "fn enabled_ads_case() {}\n"
                '#[cfg(not(feature = "opcua-wire"))]\n'
                "#[test]\n"
                "fn disabled_opcua_case() {}\n"
            ),
            expected_full_names=[
                full_name,
                enabled_name,
                disabled_opcua_name,
            ],
        )

        self.assertEqual(["default", "no-ads-wire"], [row.name for row in shards])
        self.assertFalse(shards[0].no_default_features)
        self.assertTrue(shards[1].no_default_features)
        self.assertEqual(2, len(shards[1].full_names))


if __name__ == "__main__":
    unittest.main()
