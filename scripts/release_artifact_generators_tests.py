from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.generate_conformance_status import build_conformance_status
from scripts.generate_release_provenance import build_release_provenance


class ReleaseArtifactGeneratorTests(unittest.TestCase):
    def test_provenance_binds_every_input_artifact_digest(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            files = []
            for name in (
                "trust-runtime-linux-x64.tar.gz",
                "trust-lsp-linux-x64.tar.gz",
                "trust-lsp-0.24.53-linux-x64.vsix",
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
                workflow_run_id="123",
                workflow_run_url="https://github.com/o/r/actions/runs/123",
                timestamp="2026-07-17T12:00:00+02:00",
            )
            self.assertEqual(len(payload["artifacts"]), 5)
            self.assertEqual(
                [row["path"] for row in payload["artifacts"]],
                sorted(path.name for path in files),
            )
            self.assertTrue(all(len(row["sha256"]) == 64 for row in payload["artifacts"]))

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
