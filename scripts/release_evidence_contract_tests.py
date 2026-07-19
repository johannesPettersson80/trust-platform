from __future__ import annotations

import unittest
from datetime import date

from scripts.release_evidence_contract import (
    DependencyException,
    ReleaseArtifact,
    ReleaseEvidenceError,
    validate_dependency_exception,
    validate_release_artifacts,
    validate_release_publication,
)


class ReleaseEvidenceContractTests(unittest.TestCase):
    def test_dependency_exception_requires_expiry_within_ninety_days(self) -> None:
        base = dict(
            advisory_id="RUSTSEC-2025-0001",
            owner="runtime",
            rationale="upstream replacement is not yet compatible",
            removal="remove when upstream 2.0 is adopted",
            reviewed=date(2026, 7, 17),
        )
        with self.assertRaisesRegex(ReleaseEvidenceError, "expiry"):
            validate_dependency_exception(DependencyException(**base, expires=None))
        with self.assertRaisesRegex(ReleaseEvidenceError, "90 days"):
            validate_dependency_exception(
                DependencyException(**base, expires=date(2026, 10, 16))
            )
        validate_dependency_exception(
            DependencyException(**base, expires=date(2026, 10, 15))
        )

    def test_release_artifact_inventory_is_exhaustive_and_digest_bound(self) -> None:
        required = {
            "runtime-linux-x64.tar.gz",
            "trust-lsp-linux-x64.tar.gz",
            "trust-lsp-linux-x64.vsix",
            "release-provenance.json",
            "conformance-status.json",
            "conformance-status.md",
            "SHA256SUMS",
        }
        artifacts = [
            ReleaseArtifact(path=path, kind="evidence", platform="all", sha256="a" * 64)
            for path in sorted(required)
        ]
        validate_release_artifacts(artifacts, required_paths=required)
        with self.assertRaisesRegex(ReleaseEvidenceError, "missing"):
            validate_release_artifacts(artifacts[:-1], required_paths=required)
        with self.assertRaisesRegex(ReleaseEvidenceError, "SHA-256"):
            validate_release_artifacts(
                [
                    ReleaseArtifact(row.path, row.kind, row.platform, "bad")
                    if row.path == "SHA256SUMS"
                    else row
                    for row in artifacts
                ],
                required_paths=required,
            )

    def test_release_publication_requires_latest_and_required_assets(self) -> None:
        release = {
            "tag_name": "v0.24.53",
            "draft": False,
            "prerelease": False,
            "assets": [{"name": "SHA256SUMS"}, {"name": "release-provenance.json"}],
        }
        latest = {"tag_name": "v0.24.52"}
        with self.assertRaisesRegex(ReleaseEvidenceError, "Latest"):
            validate_release_publication(
                expected_tag="v0.24.53",
                release=release,
                latest_release=latest,
                required_assets={"SHA256SUMS", "release-provenance.json"},
            )
        with self.assertRaisesRegex(ReleaseEvidenceError, "required assets"):
            validate_release_publication(
                expected_tag="v0.24.53",
                release={**release, "assets": [{"name": "SHA256SUMS"}]},
                latest_release={"tag_name": "v0.24.53"},
                required_assets={"SHA256SUMS", "release-provenance.json"},
            )


if __name__ == "__main__":
    unittest.main()
