from __future__ import annotations

import unittest
from unittest.mock import patch

from scripts.check_dependency_exceptions import GHSA_ID
from scripts.release_claim_contract import (
    ROOT,
    ReleaseEvidenceError,
    _text,
    audit_dependency_policy,
    audit_paths,
    audit_source_build,
)


class ReleaseClaimContractTests(unittest.TestCase):
    # Specification 24: the language-server archive of every platform holds trust-debug beside
    # trust-lsp. Catches a release workflow that packs the language server alone again, on Unix or
    # on Windows, which left a DAP-capable editor outside VS Code without the debug adapter.
    def test_archive_contract_requires_the_debug_adapter_beside_the_language_server(self) -> None:
        release = _text(".github/workflows/release.yml")
        self.assertEqual(audit_paths()["windows_suffix"], ".exe")
        for copy in [
            "cp target/${{ matrix.target }}/release/trust-debug dist/trust-lsp-${{ matrix.platform }}/\n",
            "Copy-Item target/${{ matrix.target }}/release/trust-debug.exe dist/trust-lsp-${{ matrix.platform }}/\n",
        ]:
            self.assertIn(copy, release)
            with patch("scripts.release_claim_contract._text", return_value=release.replace(copy, "")):
                with self.assertRaises(ReleaseEvidenceError):
                    audit_paths()

    def test_source_build_allows_additional_pinned_openot_support_packages(self) -> None:
        result = audit_source_build()
        self.assertFalse(result["sibling_required"])
        self.assertIn("open-ot-definition", result["openot_packages"])
        self.assertIn("open-ot-document", result["openot_packages"])

    @patch("scripts.check_npm_audit.subprocess.run", side_effect=AssertionError("the trace ran npm"))
    @patch("scripts.release_claim_contract.validate_file")
    def test_dependency_policy_checks_both_exception_lists_without_the_network(self, validate_mock, _run_mock) -> None:
        # Specification 24, "Dependency Exceptions": the trace checks the exception records of deny.toml and, by GitHub
        # advisory id, of the VS Code list, and reads no registry: on 8 November 2026 (checklist G80) a macOS runner's
        # failed name lookup failed the trace's test when it ran `npm audit` itself. Catches the trace running npm
        # again, and a trace that leaves the VS Code list unchecked or reads it for RustSec ids.
        validate_mock.side_effect = lambda path, **_: [object()] * (7 if path.name == "deny.toml" else 2)
        self.assertEqual(
            audit_dependency_policy(), {"rust_exceptions": 7, "vscode_exceptions": 2, "max_days": 90}
        )
        checked = [
            (call.args[0].relative_to(ROOT).as_posix(), call.kwargs.get("id_pattern"))
            for call in validate_mock.call_args_list
        ]
        self.assertEqual(checked, [("deny.toml", None), ("editors/vscode/audit-exceptions.toml", GHSA_ID)])

    @patch("scripts.release_claim_contract.validate_file")
    def test_dependency_policy_fails_on_a_vscode_exception_that_breaks_the_policy(self, validate_mock) -> None:
        # Catches a trace that checks the VS Code list but passes when it breaks the policy (an expired exception).
        def validate(path, **_):
            if path.name == "audit-exceptions.toml":
                raise ReleaseEvidenceError("dependency exception GHSA-g3hv-75wg-m249 expired on 2026-10-30")
            return [object()] * 7

        validate_mock.side_effect = validate
        with self.assertRaisesRegex(ReleaseEvidenceError, "expired on 2026-10-30"):
            audit_dependency_policy()

if __name__ == "__main__":
    unittest.main()
