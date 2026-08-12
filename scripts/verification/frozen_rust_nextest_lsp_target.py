"""Closed path inventory for frozen trust-lsp unit-test execution."""

from __future__ import annotations

from collections.abc import Mapping


LSP_REMAINING_PROFILE_KEY = "lsp-remaining"

LSP_REMAINING_UNIT_PREFIXES: Mapping[str, str] = {
    (
        "crates/trust-lsp/src/config/"
        "configuration_contract_tests.rs"
    ): "config::configuration_contract_tests::",
    (
        "crates/trust-lsp/src/config/dependency_contract_tests.rs"
    ): "config::dependency_contract_tests::",
    (
        "crates/trust-lsp/src/document_text/contract_tests.rs"
    ): "document_text::contract_tests::",
    (
        "crates/trust-lsp/src/external_diagnostics/contract_tests.rs"
    ): "external_diagnostics::contract_tests::",
    (
        "crates/trust-lsp/src/handlers/runtime_values/contract_tests.rs"
    ): "handlers::runtime_values::contract_tests::",
    (
        "crates/trust-lsp/src/index_cache/contract_tests.rs"
    ): "index_cache::contract_tests::",
    (
        "crates/trust-lsp/src/library_docs/contract_tests.rs"
    ): "library_docs::contract_tests::",
    (
        "crates/trust-lsp/src/library_graph/contract_tests.rs"
    ): "library_graph::contract_tests::",
    (
        "crates/trust-lsp/src/state/state_contract_tests.rs"
    ): "state::state_contract_tests::",
    (
        "crates/trust-lsp/src/telemetry/contract_tests.rs"
    ): "telemetry::contract_tests::",
}

LSP_REMAINING_LIB_PATHS = frozenset(
    {
        "crates/trust-lsp/src/document_text/contract_tests.rs",
    }
)


def lsp_remaining_profile_fields(path: str | None) -> dict[str, object]:
    """Return closed target fields for one reviewed trust-lsp source path."""

    try:
        prefix = LSP_REMAINING_UNIT_PREFIXES[path]  # type: ignore[index]
    except KeyError as error:
        raise ValueError(
            f"{path!r} is outside the closed LSP remaining path set"
        ) from error
    is_library = path in LSP_REMAINING_LIB_PATHS
    return {
        "key": LSP_REMAINING_PROFILE_KEY,
        "package": "trust-lsp",
        "target_kind": "lib" if is_library else "bin",
        "target_name": None if is_library else "trust-lsp",
        "binary_id": "trust-lsp" if is_library else "trust-lsp::bin/trust-lsp",
        "test_prefix": prefix,
        "encoded_test_name_prefix": (
            "trust-lsp::trust_lsp$"
            if is_library
            else "trust-lsp::trust-lsp$"
        ),
        "discovery_source_kind": "rust_unit_test",
        "allowed_cfg_attributes": frozenset({"#[cfg(unix)]"}),
        "no_default_features_cfg": None,
        "no_default_features_shard_name": None,
    }
