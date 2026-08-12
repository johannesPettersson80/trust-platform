#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

python3 -m unittest \
  scripts.release_version_preflight_contract_tests \
  scripts.check_version_release_evidence_tests \
  scripts.release_evidence_contract_tests

python3 - <<'PY'
import json
import os
import tomllib
from pathlib import Path

keys = (
    "TRUST_VERIFY_TEST_ID",
    "TRUST_VERIFY_RUN_ID",
    "TRUST_VERIFY_ARTIFACT_DIR",
    "TRUST_VERIFY_CASE_FILE_DIGEST",
)
present = [key for key in keys if os.environ.get(key)]
if not present:
    raise SystemExit(0)
if len(present) != len(keys):
    raise RuntimeError("partial TRUST_VERIFY environment")

case_file = Path("verification/cases/release/REL_VERSION_GUARDS_001.toml")
case_data = tomllib.loads(case_file.read_text())
test_id = os.environ["TRUST_VERIFY_TEST_ID"]
artifact_dir = Path(os.environ["TRUST_VERIFY_ARTIFACT_DIR"])
artifact_dir.mkdir(parents=True, exist_ok=True)
cases = [
    {
        "id": case["id"],
        "family": case["family"],
        "result": "passed",
        "spec_gap_ref": None,
        "observed_error": None,
        "observed_status": "release_evidence_contract_checked",
        "state_delta": "not_applicable",
        "before": None,
        "after": None,
    }
    for case in case_data["case"]
]
document = {
    "schema_version": 1,
    "test_id": test_id,
    "case_file": case_file.as_posix(),
    "case_file_digest": os.environ["TRUST_VERIFY_CASE_FILE_DIGEST"],
    "helper_version": "verification-cases v1",
    "case_provenance_kind": case_data["case_provenance_kind"],
    "trace_definition_digest": case_data["trace_definition_digest"],
    "trust_verify_test_id": test_id,
    "trust_verify_run_id": os.environ["TRUST_VERIFY_RUN_ID"],
    "trust_verify_case_file_digest": os.environ["TRUST_VERIFY_CASE_FILE_DIGEST"],
    "trust_verify_artifact_dir": os.environ["TRUST_VERIFY_ARTIFACT_DIR"],
    "cases": cases,
}
(artifact_dir / f"{test_id}.json").write_text(
    json.dumps(document, indent=2, sort_keys=True) + "\n"
)
PY
