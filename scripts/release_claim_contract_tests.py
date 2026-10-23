from __future__ import annotations

import json
import subprocess
import unittest
from unittest.mock import patch

from scripts.release_claim_contract import (
    ReleaseEvidenceError,
    audit_dependency_policy,
    audit_source_build,
)


class ReleaseClaimContractTests(unittest.TestCase):
    def test_source_build_allows_additional_pinned_openot_support_packages(self) -> None:
        result = audit_source_build()
        self.assertFalse(result["sibling_required"])
        self.assertIn("open-ot-definition", result["openot_packages"])
        self.assertIn("open-ot-document", result["openot_packages"])

    @patch("scripts.release_claim_contract.validate_file", return_value=[object()] * 7)
    @patch("scripts.release_claim_contract.subprocess.run")
    def test_dependency_policy_requires_zero_vscode_vulnerabilities(
        self, run_mock, _validate_mock
    ) -> None:
        run_mock.return_value = subprocess.CompletedProcess(
            args=["npm", "audit"],
            returncode=0,
            stdout=json.dumps({"metadata": {"vulnerabilities": {"total": 0}}}),
            stderr="",
        )
        self.assertEqual(audit_dependency_policy()["vscode_vulnerabilities"], 0)

    @patch("scripts.release_claim_contract.sys.platform", "win32")
    @patch("scripts.release_claim_contract.validate_file", return_value=[object()] * 7)
    @patch("scripts.release_claim_contract.subprocess.run")
    def test_dependency_policy_uses_windows_npm_launcher(
        self, run_mock, _validate_mock
    ) -> None:
        run_mock.return_value = subprocess.CompletedProcess(
            args=["npm.cmd", "audit"],
            returncode=0,
            stdout=json.dumps({"metadata": {"vulnerabilities": {"total": 0}}}),
            stderr="",
        )

        audit_dependency_policy()

        self.assertEqual(run_mock.call_args.args[0][0], "npm.cmd")

    @patch("scripts.release_claim_contract.validate_file", return_value=[object()] * 7)
    @patch("scripts.release_claim_contract.subprocess.run")
    def test_dependency_policy_rejects_a_nonzero_vscode_audit(
        self, run_mock, _validate_mock
    ) -> None:
        run_mock.return_value = subprocess.CompletedProcess(
            args=["npm", "audit"],
            returncode=1,
            stdout=json.dumps({"metadata": {"vulnerabilities": {"total": 1}}}),
            stderr="",
        )
        with self.assertRaisesRegex(
            ReleaseEvidenceError, "VS Code dependency audit found 1 vulnerabilities"
        ):
            audit_dependency_policy()


if __name__ == "__main__":
    unittest.main()
