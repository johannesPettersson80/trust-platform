"""The regression-test-first check of a release pull request (specification 22, "Change and release workflow")."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_regression_test_first as check  # noqa: E402

# What `scripts/bump_version.py` changed for 0.29.0, and the words of that release pull request.
BUMP_FILES = [
    "CHANGELOG.md",
    "Cargo.lock",
    "Cargo.toml",
    "changelog.d/actuator-kind-kept.md",
    "editors/vscode/package-lock.json",
    "editors/vscode/package.json",
    "firmware/trust-esp32c6/Cargo.lock",
    "firmware/trust-esp32c6/Cargo.toml",
    "firmware/trust-stm32u5a9/Cargo.lock",
]
RELEASE_TEXT = "Release 0.29.0\n\n0.28.0 deletes the actuator of a simulated plant ..., fixed by #202."


class RegressionTestFirstTests(unittest.TestCase):
    # Catches the 0.29.0 release pull request's failure (#207): its description named the fix it
    # released, and the check asked a version bump for a test.
    def test_a_version_bump_needs_no_test(self) -> None:
        findings = check.findings_for_inputs(
            changed=BUMP_FILES, text=RELEASE_TEXT, inline_test_added=False, version_bump=True
        )
        self.assertEqual(findings, [])

    # Catches the exemption reaching code: a bump that also changes a source file is judged as
    # every other change is.
    def test_a_version_bump_that_changes_code_needs_a_test(self) -> None:
        findings = check.findings_for_inputs(
            changed=[*BUMP_FILES, "crates/trust-lsp/src/check.rs"],
            text=RELEASE_TEXT,
            inline_test_added=False,
            version_bump=True,
        )
        self.assertNotEqual(findings, [])

    # Catches the exemption granted by the files alone: a manifest change that does not move the
    # workspace version is no release.
    def test_a_manifest_fix_without_a_version_move_needs_a_test(self) -> None:
        findings = check.findings_for_inputs(
            changed=["Cargo.toml", "Cargo.lock"],
            text="fix: pin the parser dependency",
            inline_test_added=False,
            version_bump=False,
        )
        self.assertNotEqual(findings, [])

    # Catches the version read from another section: only `[workspace.package]` names the
    # workspace version.
    def test_the_workspace_version_comes_from_the_workspace_package(self) -> None:
        manifest = (
            '[package]\nname = "x"\nversion = "9.9.9"\n\n'
            '[workspace.package]\nedition = "2024"\nversion = "0.29.1"\n\n'
            '[dependencies]\nserde = { version = "1" }\n'
        )
        self.assertEqual(check.parse_workspace_version(manifest), "0.29.1")
        self.assertIsNone(check.parse_workspace_version('[package]\nversion = "1.0.0"\n'))


if __name__ == "__main__":
    unittest.main()
