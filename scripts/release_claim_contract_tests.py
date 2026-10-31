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


# The two advisories the repository's exceptions own (31 October 2026), as `npm audit --json` reports them.
OWNED_REPORT = {
    "vulnerabilities": {
        "minimatch": {"name": "minimatch", "via": [
            {"name": "minimatch", "url": "https://github.com/advisories/GHSA-g3hv-75wg-m249"}]},
        "glob": {"name": "glob", "via": [
            {"name": "glob", "url": "https://github.com/advisories/GHSA-3g3m-2fjr-6pw4"}, "minimatch"]},
    }
}


class ReleaseClaimContractTests(unittest.TestCase):
    def test_source_build_allows_additional_pinned_openot_support_packages(self) -> None:
        result = audit_source_build()
        self.assertFalse(result["sibling_required"])
        self.assertIn("open-ot-definition", result["openot_packages"])
        self.assertIn("open-ot-document", result["openot_packages"])

    @patch("scripts.release_claim_contract.validate_file", return_value=[object()] * 7)
    @patch("scripts.check_npm_audit.subprocess.run")
    def test_dependency_policy_accepts_only_owned_vscode_advisories(self, run_mock, _validate_mock) -> None:
        # Specification 24, "Dependency Exceptions": every advisory npm reports has an owned exception in
        # editors/vscode/audit-exceptions.toml. Catches an audit that passes an advisory nobody owns.
        owned = json.loads(json.dumps(OWNED_REPORT))
        run_mock.return_value = subprocess.CompletedProcess(
            args=["npm", "audit"], returncode=1, stdout=json.dumps(owned), stderr=""
        )
        result = audit_dependency_policy()
        self.assertEqual((result["vscode_unowned_advisories"], result["rust_exceptions"]), (0, 7))
        owned["vulnerabilities"]["left-pad"] = {
            "name": "left-pad",
            "via": [{"name": "left-pad", "url": "https://github.com/advisories/GHSA-2222-3333-4444"}],
        }
        run_mock.return_value = subprocess.CompletedProcess(
            args=["npm", "audit"], returncode=1, stdout=json.dumps(owned), stderr=""
        )
        with self.assertRaisesRegex(ReleaseEvidenceError, "GHSA-2222-3333-4444 \\(left-pad\\) has no owned exception"):
            audit_dependency_policy()

    @patch("scripts.check_npm_audit.sys.platform", "win32")
    @patch("scripts.release_claim_contract.validate_file", return_value=[object()] * 7)
    @patch("scripts.check_npm_audit.subprocess.run")
    def test_dependency_policy_uses_windows_npm_launcher(self, run_mock, _validate_mock) -> None:
        run_mock.return_value = subprocess.CompletedProcess(
            args=["npm.cmd", "audit"], returncode=0, stdout=json.dumps({"vulnerabilities": {}}), stderr=""
        )
        audit_dependency_policy()
        self.assertEqual(run_mock.call_args.args[0][0], "npm.cmd")

    @patch("scripts.release_claim_contract.validate_file", return_value=[object()] * 7)
    @patch("scripts.check_npm_audit.subprocess.run")
    def test_dependency_policy_rejects_an_audit_without_its_report(self, run_mock, _validate_mock) -> None:
        # Catches a failed npm request read as a clean audit.
        run_mock.return_value = subprocess.CompletedProcess(
            args=["npm", "audit"], returncode=1, stdout=json.dumps({"error": {"code": "ENOTFOUND"}}), stderr=""
        )
        with self.assertRaisesRegex(ReleaseEvidenceError, "npm audit did not answer its report"):
            audit_dependency_policy()


if __name__ == "__main__":
    unittest.main()
