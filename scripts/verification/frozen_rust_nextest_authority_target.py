"""Exact Phase 18 authority-route extensions for frozen Rust targets."""

from __future__ import annotations

from collections.abc import Mapping


_AUTHORITY_ROUTE_EXTENSION_PATHS = frozenset(
    """
crates/trust-debug/src/adapter/lifecycle_contract_tests.rs
crates/trust-dev/src/agent/contract_tests.rs
crates/trust-dev/src/agent/harness_contract_tests.rs
crates/trust-dev/src/docs/tests/discovery_contract.rs
crates/trust-dev/src/docs/tests/publication_contract.rs
crates/trust-dev/src/workflow/contract_tests.rs
crates/trust-hir/src/db/diagnostics/complexity/contract_tests.rs
crates/trust-hir/src/db/diagnostics/configuration/contract_tests.rs
crates/trust-hir/src/db/diagnostics/nondeterminism/contract_tests.rs
crates/trust-hir/src/db/diagnostics/shared_globals/contract_tests.rs
crates/trust-hir/src/db/diagnostics/unreachable/contract_tests.rs
crates/trust-hir/src/db/diagnostics/unused/contract_tests.rs
crates/trust-hir/src/diagnostics/contract_tests.rs
crates/trust-hir/src/ident/contract_tests.rs
crates/trust-hir/src/project/contract_tests.rs
crates/trust-hir/src/semantic/contract_tests.rs
crates/trust-hir/src/symbols/table_contract_tests.rs
crates/trust-hir/src/types/contract_tests.rs
crates/trust-hir/tests/semantic_actions.rs
crates/trust-hir/tests/semantic_type_checking/call_binding_contract_acceptance.rs
crates/trust-hir/tests/semantic_type_checking/call_binding_contract_rejection.rs
crates/trust-hir/tests/semantic_type_checking/control_flow_contract_acceptance.rs
crates/trust-hir/tests/semantic_type_checking/control_flow_contract_rejection.rs
crates/trust-hir/tests/semantic_type_checking/edge_declaration_contract.rs
crates/trust-hir/tests/semantic_type_checking/expression_operator_contract_acceptance.rs
crates/trust-hir/tests/semantic_type_checking/expression_operator_contract_rejection.rs
crates/trust-hir/tests/semantic_type_checking/generic_type_contract.rs
crates/trust-hir/tests/semantic_type_checking/member_access_acceptance.rs
crates/trust-hir/tests/semantic_type_checking/member_access_declaration_rejection.rs
crates/trust-hir/tests/semantic_type_checking/member_access_use_rejection.rs
crates/trust-hir/tests/semantic_type_checking/partial_access_acceptance.rs
crates/trust-hir/tests/semantic_type_checking/partial_access_rejection.rs
crates/trust-hir/tests/semantic_type_checking/reference_contract_acceptance.rs
crates/trust-hir/tests/semantic_type_checking/reference_contract_rejection.rs
crates/trust-hir/tests/semantic_type_checking/string_index_contract.rs
crates/trust-hir/tests/semantic_type_checking/user_type_contract_acceptance.rs
crates/trust-hir/tests/semantic_type_checking/user_type_contract_rejection.rs
crates/trust-hir/tests/semantic_type_checking/variable_section_qualifier_acceptance.rs
crates/trust-hir/tests/semantic_type_checking/variable_section_qualifier_rejection.rs
crates/trust-plcopen/src/plcopen/tests/contracts.rs
crates/trust-plcopen/src/plcopen/tests/global_var_contract.rs
crates/trust-plcopen/src/plcopen/tests/project_model_contract.rs
crates/trust-plcopen/src/plcopen/tests/semantic_export_contract.rs
crates/trust-plcopen/src/plcopen/tests/semantic_import_contract.rs
crates/trust-plcopen/src/plcopen/tests/shims_metrics.rs
crates/trust-plcopen/src/plcopen/tests/transaction_contract.rs
crates/trust-runtime/src/bytecode/encoder/codegen.rs
crates/trust-runtime/src/bytecode/encoder/mod.rs
crates/trust-runtime/src/bytecode/encoder/refs.rs
crates/trust-runtime/src/bytecode/validate.rs
crates/trust-runtime/src/config/contracts/contract_tests.rs
crates/trust-runtime/src/control/auth/contract_tests.rs
crates/trust-runtime/src/control/comm_handlers/apply/runtime_file/contract_tests.rs
crates/trust-runtime/src/control/comm_handlers/browse_symbols/contract_tests.rs
crates/trust-runtime/src/control/comm_handlers/capability_contract_tests.rs
crates/trust-runtime/src/control/comm_handlers/discover/contract_tests.rs
crates/trust-runtime/src/control/comm_handlers/discovery_probe/contract_tests.rs
crates/trust-runtime/src/control/comm_handlers/probe/contract_tests.rs
crates/trust-runtime/src/control/comm_handlers/schema/contract_tests.rs
crates/trust-runtime/src/control/comm_handlers/schema/contract_tests/instance_projection.rs
crates/trust-runtime/src/control/config_handlers/handler_contract_tests.rs
crates/trust-runtime/src/control/config_handlers/validation_contract_tests.rs
crates/trust-runtime/src/control/fleet_handlers/host_contract_tests.rs
crates/trust-runtime/src/control/fleet_handlers/io_contract_tests.rs
crates/trust-runtime/src/control/fleet_handlers/link_contract_tests.rs
crates/trust-runtime/src/control/fleet_handlers/offline/contract_tests.rs
crates/trust-runtime/src/control/fleet_handlers/protocol_contract_tests.rs
crates/trust-runtime/src/control/fleet_handlers/topology_contract_tests.rs
crates/trust-runtime/src/control/io_handlers/contract_tests.rs
crates/trust-runtime/src/control/operation_registry/contract_tests.rs
crates/trust-runtime/src/control/status_handlers/event_contract_tests.rs
crates/trust-runtime/src/control/status_handlers/historian_contract_tests.rs
crates/trust-runtime/src/control/status_handlers/projection_contract_tests.rs
crates/trust-runtime/src/control/tests/control_endpoint_source_contract.rs
crates/trust-runtime/src/control/tests/control_request_boundary_contract.rs
crates/trust-runtime/src/control/variable_handlers/contract_tests.rs
crates/trust-runtime/src/host/boundary/error_contract_tests.rs
crates/trust-runtime/src/host/boundary/protocol_envelope_contract_tests.rs
crates/trust-runtime/src/host/boundary/resolver_contract_tests.rs
crates/trust-runtime/src/host/debug/breakpoints/contract_tests.rs
crates/trust-runtime/src/host/debug/control/hook/contract_tests.rs
crates/trust-runtime/src/host/debug/control/tests/lifecycle_contract.rs
crates/trust-runtime/src/host/debug/control/tests/mutation_contract.rs
crates/trust-runtime/src/host/debug/control/tests/state_stream_contract.rs
crates/trust-runtime/src/host/debug/dap/contract_tests.rs
crates/trust-runtime/src/host/debug/hook/contract_tests.rs
crates/trust-runtime/src/host/debug/resolve/contract_tests.rs
crates/trust-runtime/src/host/debug/trace/contract_tests.rs
crates/trust-runtime/src/host/debug/types/contract_tests.rs
crates/trust-runtime/src/host/harness/api_contract_tests.rs
crates/trust-runtime/src/host/harness/coerce_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/action_profile_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/constant_initializer_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/default_initializer_dependency_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/derived_type_constant_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/generic_type_profile_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/pou_call_binding_runtime_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/pou_function_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/pou_initializer_dependency_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/pou_initializer_lifetime_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/pou_member_access_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/pou_object_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/pou_variable_qualifier_projection_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/pou_variable_section_acceptance_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/pou_variable_section_rejection_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/project_assembly_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/type_contract_tests.rs
crates/trust-runtime/src/host/harness/compiler/variable_contract_tests.rs
crates/trust-runtime/src/host/harness/config_access_contract_tests.rs
crates/trust-runtime/src/host/harness/config_contract_tests.rs
crates/trust-runtime/src/host/harness/harness_contract_tests.rs
crates/trust-runtime/src/host/harness/initializer_contract_tests.rs
crates/trust-runtime/src/host/harness/io/io_contract_tests.rs
crates/trust-runtime/src/host/harness/lower/expression_contract_tests.rs
crates/trust-runtime/src/host/harness/lower/expression_operator_runtime_contract_tests.rs
crates/trust-runtime/src/host/harness/lower/partial_access_runtime_contract_tests.rs
crates/trust-runtime/src/host/harness/lower/reference_runtime_contract_tests.rs
crates/trust-runtime/src/host/harness/lower/runtime_clock_source_contract_tests.rs
crates/trust-runtime/src/host/harness/lower/statement_contract_tests.rs
crates/trust-runtime/src/host/harness/lower/statement_control_flow_contract_tests.rs
crates/trust-runtime/src/host/harness/lower/string_index_runtime_contract_tests.rs
crates/trust-runtime/src/host/harness/parse_contract_tests.rs
crates/trust-runtime/src/host/harness/protocol_contract_tests.rs
crates/trust-runtime/src/host/helper_eval/const_expr/contract_tests.rs
crates/trust-runtime/src/host/helper_eval/storage_expr/contract_tests.rs
crates/trust-runtime/src/host/helper_eval/storage_lvalue/contract_tests.rs
crates/trust-runtime/src/host/metrics_contract_tests.rs
crates/trust-runtime/src/host/registry/registry_integrity_contract_tests.rs
crates/trust-runtime/src/host/security/contract_tests.rs
crates/trust-runtime/src/host/security/pairing/contract_tests.rs
crates/trust-runtime/src/host/simulation_contract_tests.rs
crates/trust-runtime/src/host/ui/commands/contract_tests.rs
crates/trust-runtime/src/host/ui/console_contract_tests.rs
crates/trust-runtime/src/io/gpio_contract_tests.rs
crates/trust-runtime/src/io/interface_contract_tests.rs
crates/trust-runtime/src/io/modbus/tests/point_map_contract.rs
crates/trust-runtime/src/io/mqtt/tests/point_map_contract.rs
crates/trust-runtime/src/io/provenance_contract_tests.rs
crates/trust-runtime/src/io/registry_contract_tests.rs
crates/trust-runtime/src/program_model/stmt/contract_tests.rs
crates/trust-runtime/src/program_model/types/contract_tests.rs
crates/trust-runtime/src/runtime/bytecode.rs
crates/trust-runtime/src/runtime/core/accessor_contract_tests.rs
crates/trust-runtime/src/runtime/core/tests.rs
crates/trust-runtime/src/runtime/cycle.rs
crates/trust-runtime/src/runtime/io_subsystem.rs
crates/trust-runtime/src/runtime/mesh.rs
crates/trust-runtime/src/runtime/metadata.rs
crates/trust-runtime/src/runtime/metrics_subsystem.rs
crates/trust-runtime/src/runtime/retain_snapshot.rs
crates/trust-runtime/src/runtime/vm/debug_map.rs
crates/trust-runtime/src/runtime/vm/dispatch.rs
crates/trust-runtime/src/runtime/vm/limits.rs
crates/trust-runtime/src/runtime/vm/local_init.rs
crates/trust-runtime/src/runtime/vm/local_init/expr.rs
crates/trust-runtime/src/runtime/vm/mod.rs
crates/trust-runtime/src/stdlib/time.rs
crates/trust-runtime/src/value/display/contract_tests.rs
crates/trust-runtime/src/value/reference/contract_tests.rs
crates/trust-runtime/src/web/auth_helpers/contract_tests.rs
crates/trust-runtime/src/web/config_ui_routes/lifecycle/contract_tests.rs
crates/trust-runtime/src/web/config_ui_routes/live/contract_tests.rs
crates/trust-runtime/src/web/config_ui_routes/responses/contract_tests.rs
crates/trust-runtime/src/web/config_ui_routes/workspace/contract_tests.rs
crates/trust-runtime/src/web/http_post_policy/contract_tests.rs
crates/trust-runtime/src/web/ide/utils/analysis/contract_tests.rs
crates/trust-runtime/src/web/ide/utils/formatting/contract_tests.rs
crates/trust-runtime/src/web/ide/utils/fs/contract_tests.rs
crates/trust-runtime/src/web/ide/utils/paths/contract_tests.rs
crates/trust-runtime/src/web/ide/utils/session/contract_tests.rs
crates/trust-runtime/src/web/setup_support/contract_tests.rs
crates/trust-runtime/src/web/web_helpers/contract_tests.rs
crates/trust-runtime/tests/edge_declaration_contract.rs
crates/trust-runtime/tests/lexical_literal_runtime_contract.rs
crates/trust-runtime/tests/stdlib_conversion_contract.rs
crates/trust-runtime/tests/stdlib_core_contract.rs
crates/trust-runtime/tests/stdlib_fb_contract.rs
crates/trust-runtime/tests/stdlib_helper_contract.rs
crates/trust-runtime/tests/user_type_runtime_contract.rs
crates/trust-syntax/tests/lexer_lexical_contract.rs
crates/trust-syntax/tests/parser_action_contract.rs
crates/trust-syntax/tests/parser_call_binding_contract.rs
crates/trust-syntax/tests/parser_control_flow_contract.rs
crates/trust-syntax/tests/parser_edge_declarations.rs
crates/trust-syntax/tests/parser_generic_type_contract.rs
crates/trust-syntax/tests/parser_lexical_contract.rs
crates/trust-syntax/tests/parser_partial_access_contract.rs
crates/trust-syntax/tests/parser_reference_contract.rs
crates/trust-syntax/tests/parser_string_index_contract.rs
crates/trust-syntax/tests/parser_user_type_contract.rs
""".strip().splitlines()
)

_PROFILE_PACKAGES: Mapping[str, str] = {
    "debug-remaining": "trust-debug",
    "dev-remaining": "trust-dev",
    "hir-remaining": "trust-hir",
    "plcopen-remaining": "trust-plcopen",
    "runtime-remaining": "trust-runtime",
    "runtime-core-remaining": "trust-runtime-core",
    "syntax-remaining": "trust-syntax",
}

_UNIX_CFG = "#[cfg(unix)]"
_ADS_WIRE_DISABLED_CFG = '#[cfg(not(feature = "ads-wire"))]'
_ADS_WIRE_ENABLED_CFG = '#[cfg(feature = "ads-wire")]'
_OPCUA_WIRE_DISABLED_CFG = '#[cfg(not(feature = "opcua-wire"))]'
_OPCUA_WIRE_ENABLED_CFG = '#[cfg(feature = "opcua-wire")]'

_INLINE_TEST_MODULES: Mapping[str, str] = {
    "crates/trust-runtime/src/bytecode/encoder/codegen.rs": "tests",
    "crates/trust-runtime/src/bytecode/encoder/mod.rs": "tests",
    "crates/trust-runtime/src/bytecode/encoder/refs.rs": "tests",
    "crates/trust-runtime/src/bytecode/validate.rs": "tests",
    "crates/trust-runtime/src/runtime/bytecode.rs": "tests",
    "crates/trust-runtime/src/runtime/cycle.rs": "tests",
    "crates/trust-runtime/src/runtime/io_subsystem.rs": "tests",
    "crates/trust-runtime/src/runtime/mesh.rs": "tests",
    "crates/trust-runtime/src/runtime/metadata.rs": "tests",
    "crates/trust-runtime/src/runtime/metrics_subsystem.rs": "tests",
    "crates/trust-runtime/src/runtime/retain_snapshot.rs": "contract_tests",
    "crates/trust-runtime/src/runtime/vm/debug_map.rs": "tests",
    "crates/trust-runtime/src/runtime/vm/dispatch.rs": "tests",
    "crates/trust-runtime/src/runtime/vm/limits.rs": "tests",
    "crates/trust-runtime/src/runtime/vm/local_init.rs": "tests",
    "crates/trust-runtime/src/runtime/vm/local_init/expr.rs": "tests",
    "crates/trust-runtime/src/runtime/vm/mod.rs": "tests",
    "crates/trust-runtime/src/stdlib/time.rs": "tests",
}

_UNIT_PREFIX_OVERRIDES: Mapping[str, str] = {
    "crates/trust-plcopen/src/plcopen/tests/contracts.rs": "tests::",
    "crates/trust-plcopen/src/plcopen/tests/global_var_contract.rs": "tests::",
    "crates/trust-plcopen/src/plcopen/tests/project_model_contract.rs": "tests::",
    "crates/trust-plcopen/src/plcopen/tests/semantic_export_contract.rs": "tests::",
    "crates/trust-plcopen/src/plcopen/tests/semantic_import_contract.rs": "tests::",
    "crates/trust-plcopen/src/plcopen/tests/shims_metrics.rs": "tests::",
    "crates/trust-plcopen/src/plcopen/tests/transaction_contract.rs": "tests::",
    "crates/trust-debug/src/adapter/lifecycle_contract_tests.rs": (
        "adapter::tests::lifecycle_contract_tests::"
    ),
    "crates/trust-dev/src/agent/harness_contract_tests.rs": (
        "agent::harness::harness_contract_tests::"
    ),
    "crates/trust-hir/src/symbols/table_contract_tests.rs": (
        "symbols::table::contract_tests::"
    ),
    "crates/trust-runtime/src/config/contracts/contract_tests.rs": (
        "config::contract_tests::"
    ),
    "crates/trust-runtime/src/control/tests/control_endpoint_source_contract.rs": (
        "control::tests::"
    ),
    "crates/trust-runtime/src/control/tests/control_request_boundary_contract.rs": (
        "control::tests::"
    ),
    "crates/trust-runtime/src/host/boundary/error_contract_tests.rs": (
        "boundary::error::contract_tests::"
    ),
    "crates/trust-runtime/src/host/boundary/protocol_envelope_contract_tests.rs": (
        "boundary::protocol_envelope::contract_tests::"
    ),
    "crates/trust-runtime/src/host/boundary/resolver_contract_tests.rs": (
        "boundary::resolver::contract_tests::"
    ),
    "crates/trust-runtime/src/host/debug/breakpoints/contract_tests.rs": (
        "debug::breakpoints::contract_tests::"
    ),
    "crates/trust-runtime/src/host/debug/control/hook/contract_tests.rs": (
        "debug::control::contract_tests::"
    ),
    "crates/trust-runtime/src/host/debug/control/tests/lifecycle_contract.rs": (
        "debug::control::tests::lifecycle_contract::"
    ),
    "crates/trust-runtime/src/host/debug/control/tests/mutation_contract.rs": (
        "debug::control::tests::mutation_contract::"
    ),
    "crates/trust-runtime/src/host/debug/control/tests/state_stream_contract.rs": (
        "debug::control::tests::state_stream_contract::"
    ),
    "crates/trust-runtime/src/host/debug/dap/contract_tests.rs": (
        "debug::dap::contract_tests::"
    ),
    "crates/trust-runtime/src/host/debug/hook/contract_tests.rs": (
        "debug::hook::contract_tests::"
    ),
    "crates/trust-runtime/src/host/harness/api_contract_tests.rs": (
        "harness::api::contract_tests::"
    ),
    "crates/trust-runtime/src/host/harness/coerce_contract_tests.rs": (
        "harness::coerce::contract_tests::"
    ),
    "crates/trust-runtime/src/host/harness/config_access_contract_tests.rs": (
        "harness::build::access_contract_tests::"
    ),
    "crates/trust-runtime/src/host/harness/config_contract_tests.rs": (
        "harness::build::contract_tests::"
    ),
    "crates/trust-runtime/src/host/harness/harness_contract_tests.rs": (
        "harness::harness::contract_tests::"
    ),
    "crates/trust-runtime/src/host/harness/initializer_contract_tests.rs": (
        "harness::initializer::contract_tests::"
    ),
    "crates/trust-runtime/src/host/harness/parse_contract_tests.rs": (
        "harness::parse::contract_tests::"
    ),
    "crates/trust-runtime/src/host/harness/protocol_contract_tests.rs": (
        "harness::protocol::contract_tests::"
    ),
    "crates/trust-runtime/src/host/registry/registry_integrity_contract_tests.rs": (
        "registry::tests::"
    ),
    "crates/trust-runtime/src/host/metrics_contract_tests.rs": (
        "metrics::metrics_contract_tests::"
    ),
    "crates/trust-runtime/src/host/simulation_contract_tests.rs": (
        "simulation::contract_tests::"
    ),
    "crates/trust-runtime/src/io/gpio_contract_tests.rs": (
        "io::gpio::contract_tests::"
    ),
    "crates/trust-runtime/src/io/modbus/tests/point_map_contract.rs": (
        "io::modbus::point_map::tests::"
    ),
    "crates/trust-runtime/src/io/mqtt/tests/point_map_contract.rs": (
        "io::mqtt::point_map_nonfinite_tests::"
    ),
    "crates/trust-runtime/src/io/registry_contract_tests.rs": (
        "io::registry::contract_tests::"
    ),
}


def authority_route_profile_fields(
    key: str,
    path: str | None,
) -> dict[str, object] | None:
    """Return exact target fields only for the frozen extension inventory."""

    if path not in _AUTHORITY_ROUTE_EXTENSION_PATHS:
        return None
    parts = str(path).split("/")
    if len(parts) < 4 or parts[0] != "crates":
        raise ValueError(f"invalid frozen authority-route path {path!r}")
    package = parts[1]
    if _PROFILE_PACKAGES.get(key) != package:
        return None
    if parts[2] == "src":
        target_kind = "bin" if package == "trust-dev" else "lib"
        target_name = "trust-dev" if package == "trust-dev" else None
        binary_id = (
            "trust-dev::bin/trust-dev"
            if package == "trust-dev"
            else package
        )
        encoded_target = "trust-dev" if package == "trust-dev" else package.replace("-", "_")
        prefix = _unit_prefix(str(path), parts[3:])
    elif parts[2] == "tests":
        target_kind = "test"
        target_name = parts[3].removesuffix(".rs")
        binary_id = f"{package}::{target_name}"
        encoded_target = target_name
        nested = parts[4:]
        prefix = (
            "::".join([value.removesuffix(".rs") for value in nested]) + "::"
            if nested
            else ""
        )
    else:
        raise ValueError(f"invalid frozen authority-route path {path!r}")
    return {
        "key": key,
        "package": package,
        "target_kind": target_kind,
        "target_name": target_name,
        "binary_id": binary_id,
        "test_prefix": prefix,
        "encoded_test_name_prefix": f"{package}::{encoded_target}$",
        "discovery_source_kind": (
            "rust_unit_test" if target_kind != "test" else "rust_integration_test"
        ),
        "allowed_cfg_attributes": _allowed_cfg_attributes(package),
        "no_default_features_cfg": (
            _ADS_WIRE_DISABLED_CFG if package == "trust-runtime" else None
        ),
        "no_default_features_shard_name": (
            "no-ads-wire" if package == "trust-runtime" else None
        ),
        "additional_no_default_features_cfgs": (
            frozenset({_OPCUA_WIRE_DISABLED_CFG})
            if package == "trust-runtime"
            else frozenset()
        ),
    }


def _unit_prefix(path: str, relative_parts: list[str]) -> str:
    override = _UNIT_PREFIX_OVERRIDES.get(path)
    if override is not None:
        return override
    modules = [value.removesuffix(".rs") for value in relative_parts]
    if modules and modules[0] == "host":
        modules.pop(0)
    if modules[-1] == "mod":
        modules.pop()
    inline = _INLINE_TEST_MODULES.get(path)
    if inline is not None:
        modules.append(inline)
    return "::".join(modules) + "::"


def _allowed_cfg_attributes(package: str) -> frozenset[str]:
    if package == "trust-runtime":
        return frozenset(
            {
                _UNIX_CFG,
                _ADS_WIRE_DISABLED_CFG,
                _ADS_WIRE_ENABLED_CFG,
                _OPCUA_WIRE_DISABLED_CFG,
                _OPCUA_WIRE_ENABLED_CFG,
            }
        )
    if package in {"trust-dev", "trust-lsp", "trust-plcopen"}:
        return frozenset({_UNIX_CFG})
    return frozenset()
