from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.generate_conformance_status import build_conformance_status
from scripts.generate_release_provenance import build_release_provenance

TREE = "c" * 40


def release_files(root: Path) -> list[Path]:
    """A package of each kind, the toolchain bundle and the conformance status, as the release folders hold them."""
    files = []
    for name in ("trust-lsp-0.31.0-linux-x64.vsix", "trust-runtime-linux-x64.tar.gz", "trust-lsp-linux-x64.tar.gz",
                 "trust-toolchain-linux-x64.tar.gz", "conformance-status.json", "conformance-status.md"):
        path = root / name
        path.write_bytes(name.encode())
        files.append(path)
    return files


def record(files: list[Path], *, commit: str, run: str, rehearsal: dict | None = None, tree: str = TREE,
           tag: str = "v0.31.0") -> dict:
    return build_release_provenance(
        files=files,
        tag=tag,
        commit=commit,
        tree=tree,
        workflow_run_id=run,
        workflow_run_url=f"https://github.com/o/r/actions/runs/{run}",
        timestamp="2026-11-04T00:13:03+00:00",
        rehearsal=rehearsal,
    )


class ReleaseArtifactGeneratorTests(unittest.TestCase):
    def test_provenance_binds_every_input_artifact_digest(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            files = []
            for name in (
                "trust-runtime-linux-x64.tar.gz",
                "trust-lsp-linux-x64.tar.gz",
                "trust-lsp-0.24.53-linux-x64.vsix",
                "trust-toolchain-darwin-arm64.tar.gz",
                "conformance-status.json",
                "conformance-status.md",
            ):
                path = root / name
                path.write_bytes(name.encode())
                files.append(path)
            payload = build_release_provenance(
                files=files,
                tag="v0.24.53",
                commit="a" * 40,
                tree=TREE,
                workflow_run_id="123",
                workflow_run_url="https://github.com/o/r/actions/runs/123",
                timestamp="2026-07-17T12:00:00+02:00",
            )
            self.assertEqual(len(payload["artifacts"]), 6)
            # Specification 24: the record names the commit's tree, and a release whose run built its packages was not
            # promoted. Catches a record that cannot show which tree its packages came from.
            self.assertEqual(
                (payload["schema_version"], payload["tree"], payload["promoted"], payload["rehearsal"]),
                (2, TREE, False, None),
            )
            # Specification 24: the toolchain bundles (specification 44 §4.6) are released distributables with their own
            # kind; v0.25.0's Publish refused 'trust-toolchain-darwin-arm64.tar.gz' as unreviewed (28 October 2026).
            toolchain = next(
                row for row in payload["artifacts"] if row["path"] == "trust-toolchain-darwin-arm64.tar.gz"
            )
            self.assertEqual((toolchain["kind"], toolchain["platform"]), ("toolchain_bundle", "darwin-arm64"))
            self.assertEqual(
                [row["path"] for row in payload["artifacts"]],
                sorted(path.name for path in files),
            )
            self.assertTrue(all(len(row["sha256"]) == 64 for row in payload["artifacts"]))

    def test_a_promoted_release_names_the_rehearsal_that_built_its_files(self) -> None:
        # Specification 24, "Release after main CI": a promoted release's record names the released commit, its tree,
        # the run that released it, and the rehearsal run and commit that built its packages; it lists the rehearsal's
        # artifacts with their digests. Catches a promoted release whose record says its own run built the packages, or
        # names the rehearsed commit as the released one (verify-release reads the record's commit and run).
        with tempfile.TemporaryDirectory() as raw:
            files = release_files(Path(raw))
            rehearsed = record(files, commit="a" * 40, run="41707764199")
            promoted = record(files, commit="b" * 40, run="41759048708", rehearsal=rehearsed)
            self.assertEqual((promoted["commit"], promoted["tree"], promoted["workflow_run_id"]),
                             ("b" * 40, TREE, "41759048708"))
            self.assertTrue(promoted["promoted"])
            self.assertEqual(promoted["rehearsal"], {
                "workflow_run_id": "41707764199",
                "workflow_run_url": "https://github.com/o/r/actions/runs/41707764199",
                "commit": "a" * 40,
            })
            self.assertEqual(promoted["artifacts"], rehearsed["artifacts"])

    def test_a_promotion_refuses_a_rehearsal_of_another_tree_tag_or_files(self) -> None:
        # Specification 24: a release promotes only files that a rehearsal built from a commit with the released commit's
        # tree, for its tag, byte for byte. Catches a record that vouches for packages of another tree or version, for a
        # file changed after the rehearsal, or for files passed on by another promotion instead of built.
        with tempfile.TemporaryDirectory() as raw:
            files = release_files(Path(raw))
            rehearsed = record(files, commit="a" * 40, run="41707764199")
            cases = [
                (record(files, commit="a" * 40, run="41707764199", tree="d" * 40), "not the released commit's tree"),
                (record(files, commit="a" * 40, run="41707764199", tag="v0.30.0"), "not the release's tag v0.31.0"),
                (record(files, commit="e" * 40, run="7", rehearsal=rehearsed), "not of another promotion"),
                ({**rehearsed, "workflow_run_id": ""}, "must name its workflow run"),
            ]
            for rehearsal, reason in cases:
                with self.assertRaisesRegex(ValueError, reason):
                    record(files, commit="b" * 40, run="41759048708", rehearsal=rehearsal)
            files[0].write_bytes(b"changed after the rehearsal")
            with self.assertRaisesRegex(ValueError, "differ from those the rehearsal recorded"):
                record(files, commit="b" * 40, run="41759048708", rehearsal=rehearsed)

    def test_conformance_status_is_derived_from_summary_and_gap_text(self) -> None:
        summary = {
            "version": 2,
            "runtime": {"name": "trust-runtime", "version": "0.24.53"},
            "summary": {"total": 3, "passed": 2, "failed": 1, "errors": 0, "skipped": 0},
            "results": [
                {"case_id": "a", "status": "passed"},
                {"case_id": "b", "status": "passed"},
                {"case_id": "c", "status": "failed"},
            ],
        }
        payload = build_conformance_status(
            summary=summary,
            known_gaps="# Gaps\n\n- Gap one.\n- Gap two.\n",
            commit="b" * 40,
            toolchain="rustc 1.95.0",
            timestamp="2026-07-17T12:00:00+02:00",
        )
        self.assertEqual(payload["executed"], 3)
        self.assertEqual(payload["passed"], 2)
        self.assertEqual(payload["failed"], 1)
        self.assertEqual(payload["known_gaps"], ["Gap one.", "Gap two."])
        json.dumps(payload, allow_nan=False)


if __name__ == "__main__":
    unittest.main()
