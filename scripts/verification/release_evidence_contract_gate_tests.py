from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CASE_FILE = Path("verification/cases/release/REL_VERSION_GUARDS_001.toml")
TEST_ID = "TEST_RELEASE_EVIDENCE_CONTRACT_GUARD_001"


class ReleaseEvidenceContractGateTests(unittest.TestCase):
    def test_gate_emits_prover_bound_case_artifact(self) -> None:
        case_digest = "sha256:" + hashlib.sha256(
            (ROOT / CASE_FILE).read_bytes()
        ).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            env = os.environ.copy()
            env.update(
                {
                    "TRUST_VERIFY_TEST_ID": TEST_ID,
                    "TRUST_VERIFY_RUN_ID": "release-gate-artifact-test",
                    "TRUST_VERIFY_ARTIFACT_DIR": directory,
                    "TRUST_VERIFY_CASE_FILE_DIGEST": case_digest,
                }
            )
            completed = subprocess.run(
                [str(ROOT / "scripts/release_evidence_contract_gate.sh")],
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            artifact_path = Path(directory) / f"{TEST_ID}.json"
            self.assertTrue(artifact_path.is_file(), "gate omitted its case artifact")
            artifact = json.loads(artifact_path.read_text())

        self.assertEqual(artifact["test_id"], TEST_ID)
        self.assertEqual(artifact["trust_verify_run_id"], "release-gate-artifact-test")
        self.assertEqual(artifact["case_file"], CASE_FILE.as_posix())
        self.assertEqual(artifact["case_file_digest"], case_digest)
        self.assertEqual(
            [(case["id"], case["result"]) for case in artifact["cases"]],
            [
                ("REL_RELEASE_EVIDENCE_GUARD_001_EXACT", "passed"),
                ("REL_RELEASE_EVIDENCE_GUARD_001_NONEXACT_REJECTED", "passed"),
            ],
        )


if __name__ == "__main__":
    unittest.main()
