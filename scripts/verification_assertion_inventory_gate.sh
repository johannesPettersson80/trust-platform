#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
python3 -m unittest scripts.verification.mapping_assertions_tests
