#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
python3 scripts/validate_verification_metadata.py
python3 -m unittest \
  scripts.verification.phase18_scope_tests \
  scripts.verification.metadata_validator.oracle_refs_tests \
  scripts.verification.metadata_validator.schema_contracts_tests \
  scripts.verification.metadata_validator.spec_sources_tests
