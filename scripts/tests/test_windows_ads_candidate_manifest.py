from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.write_windows_ads_candidate_manifest import build_manifest


class WindowsAdsCandidateManifestTests(unittest.TestCase):
    def test_ci_records_workflow_run_identity_in_candidate_manifest(self) -> None:
        workflow = (
            Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"
        ).read_text(encoding="utf-8")
        for marker in (
            '--repository "${{ github.repository }}"',
            '--workflow-run-id "${{ github.run_id }}"',
            '--workflow-run-attempt "${{ github.run_attempt }}"',
            '--workflow-run-head-sha "${{ github.event.pull_request.head.sha || github.sha }}"',
            '--workflow-event "${{ github.event_name }}"',
        ):
            self.assertIn(marker, workflow)

    def test_ci_proves_real_windows_vscode_cli_before_packaging(self) -> None:
        workflow = (
            Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"
        ).read_text(encoding="utf-8")
        focused = workflow.index("- name: Run focused Windows ADS extension tests")
        cli_probe = workflow.index("- name: Prove Windows VS Code CLI boundary")
        package = workflow.index("- name: Package win32-x64 VSIX")
        self.assertLess(focused, cli_probe)
        self.assertLess(cli_probe, package)
        probe = workflow[cli_probe:package]
        self.assertIn("Resolve-VscodeCliLayout", probe)
        self.assertIn("Invoke-VscodeCli", probe)
        self.assertIn("$layout.cli_script", probe)
        self.assertIn("$layout.package_json", probe)
        self.assertIn("-TimeoutSeconds 10", probe)

    def test_manifest_binds_ci_commit_version_and_exact_vsix_bytes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="trust-candidate-manifest-") as temp:
            vsix = Path(temp) / "trust-lsp-0.24.33-win32-x64.vsix"
            vsix.write_bytes(b"synthetic candidate bytes")
            candidate = "a" * 40
            manifest = build_manifest(
                vsix,
                candidate,
                "0.24.33",
                repository="example/trust-platform",
                workflow_run_id=123,
                workflow_run_attempt=2,
                workflow_run_head_sha=candidate,
                workflow_event="push",
            )
            self.assertEqual(manifest["schema_version"], 2)
            self.assertEqual(manifest["candidate_commit_sha"], candidate)
            self.assertEqual(
                manifest["artifact_name"], f"windows-ads-msvc-candidate-{candidate}"
            )
            self.assertEqual(manifest["version"], "0.24.33")
            self.assertEqual(manifest["target_platform"], "win32-x64")
            self.assertRegex(str(manifest["vsix_sha256"]), r"^[0-9a-f]{64}$")
            self.assertEqual(
                manifest["workflow_provenance"]["workflow_run_id"], 123
            )

    def test_invalid_commit_or_missing_vsix_fails(self) -> None:
        with tempfile.TemporaryDirectory(prefix="trust-candidate-manifest-") as temp:
            root = Path(temp)
            vsix = root / "candidate.vsix"
            vsix.write_bytes(b"candidate")
            with self.assertRaisesRegex(ValueError, "40 hex"):
                build_manifest(
                    vsix,
                    "not-a-commit",
                    "0.24.33",
                    repository="example/trust-platform",
                    workflow_run_id=1,
                    workflow_run_attempt=1,
                    workflow_run_head_sha="a" * 40,
                    workflow_event="push",
                )
            with self.assertRaisesRegex(ValueError, "missing or empty"):
                build_manifest(
                    root / "missing.vsix",
                    "a" * 40,
                    "0.24.33",
                    repository="example/trust-platform",
                    workflow_run_id=1,
                    workflow_run_attempt=1,
                    workflow_run_head_sha="a" * 40,
                    workflow_event="push",
                )

    def test_pull_request_candidate_is_the_exact_workflow_run_head(self) -> None:
        with tempfile.TemporaryDirectory(prefix="trust-candidate-manifest-") as temp:
            vsix = Path(temp) / "candidate.vsix"
            vsix.write_bytes(b"candidate")
            manifest = build_manifest(
                vsix,
                "a" * 40,
                "0.24.33",
                repository="example/trust-platform",
                workflow_run_id=7,
                workflow_run_attempt=1,
                workflow_run_head_sha="a" * 40,
                workflow_event="pull_request",
            )
            self.assertEqual(
                manifest["workflow_provenance"]["candidate_source"],
                "pull_request_head",
            )
            with self.assertRaisesRegex(ValueError, "workflow run head"):
                build_manifest(
                    vsix,
                    "a" * 40,
                    "0.24.33",
                    repository="example/trust-platform",
                    workflow_run_id=7,
                    workflow_run_attempt=1,
                    workflow_run_head_sha="b" * 40,
                    workflow_event="pull_request",
                )


if __name__ == "__main__":
    unittest.main()
