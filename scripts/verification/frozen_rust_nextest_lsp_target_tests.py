from __future__ import annotations

import unittest

from scripts.verification.frozen_rust_nextest_target import get_target_profile


class FrozenRustNextestLspTargetTests(unittest.TestCase):
    def test_lsp_bin_paths_use_the_trust_lsp_binary_target(self) -> None:
        expected = {
            "crates/trust-lsp/src/config/configuration_contract_tests.rs":
                "config::configuration_contract_tests::",
            "crates/trust-lsp/src/config/dependency_contract_tests.rs":
                "config::dependency_contract_tests::",
            "crates/trust-lsp/src/external_diagnostics/contract_tests.rs":
                "external_diagnostics::contract_tests::",
            "crates/trust-lsp/src/handlers/runtime_values/contract_tests.rs":
                "handlers::runtime_values::contract_tests::",
            "crates/trust-lsp/src/index_cache/contract_tests.rs":
                "index_cache::contract_tests::",
            "crates/trust-lsp/src/library_docs/contract_tests.rs":
                "library_docs::contract_tests::",
            "crates/trust-lsp/src/library_graph/contract_tests.rs":
                "library_graph::contract_tests::",
            "crates/trust-lsp/src/state/state_contract_tests.rs":
                "state::state_contract_tests::",
            "crates/trust-lsp/src/telemetry/contract_tests.rs":
                "telemetry::contract_tests::",
        }

        for path, prefix in expected.items():
            with self.subTest(path=path):
                profile = get_target_profile("lsp-remaining", path=path)
                self.assertEqual("bin", profile.target_kind)
                self.assertEqual("trust-lsp", profile.target_name)
                self.assertEqual("trust-lsp::bin/trust-lsp", profile.binary_id)
                self.assertEqual("trust-lsp::trust-lsp$", profile.encoded_test_name_prefix)
                self.assertEqual(prefix, profile.test_prefix)

    def test_document_text_path_remains_on_the_lsp_library_target(self) -> None:
        profile = get_target_profile(
            "lsp-remaining",
            path="crates/trust-lsp/src/document_text/contract_tests.rs",
        )

        self.assertEqual("lib", profile.target_kind)
        self.assertIsNone(profile.target_name)
        self.assertEqual("trust-lsp", profile.binary_id)
        self.assertEqual("trust-lsp::trust_lsp$", profile.encoded_test_name_prefix)
        self.assertEqual("document_text::contract_tests::", profile.test_prefix)

    def test_lsp_unit_paths_resolve_to_exact_module_prefixes(self) -> None:
        profile = get_target_profile(
            "lsp-remaining",
            path=(
                "crates/trust-lsp/src/config/"
                "configuration_contract_tests.rs"
            ),
        )

        self.assertEqual("trust-lsp", profile.package)
        self.assertEqual("bin", profile.target_kind)
        self.assertEqual("trust-lsp", profile.target_name)
        self.assertEqual("trust-lsp::bin/trust-lsp", profile.binary_id)
        self.assertEqual("trust-lsp::trust-lsp$", profile.encoded_test_name_prefix)
        self.assertEqual("rust_unit_test", profile.discovery_source_kind)
        self.assertEqual(
            "config::configuration_contract_tests::",
            profile.test_prefix,
        )
        self.assertIn("#[cfg(unix)]", profile.allowed_cfg_attributes)

    def test_lsp_profile_rejects_paths_outside_closed_inventory(self) -> None:
        with self.assertRaisesRegex(ValueError, "closed LSP remaining path set"):
            get_target_profile(
                "lsp-remaining",
                path="crates/trust-lsp/src/unreviewed.rs",
            )


if __name__ == "__main__":
    unittest.main()
