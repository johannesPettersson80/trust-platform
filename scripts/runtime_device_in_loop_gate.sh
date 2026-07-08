#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

OUT_DIR="${OUT_DIR:-target/gate-artifacts/device-in-the-loop}"
mkdir -p "${OUT_DIR}"
OUT_DIR="$(cd "${OUT_DIR}" && pwd -P)"

echo "[device-in-loop] artifacts: ${OUT_DIR}"
TRUST_DIT_ARTIFACT_DIR="${OUT_DIR}" \
  cargo test -p trust-runtime --test device_in_the_loop -- --ignored --nocapture

echo "[device-in-loop] artifact summary"
find "${OUT_DIR}" -maxdepth 1 -type f -name '*.json' -print | sort
