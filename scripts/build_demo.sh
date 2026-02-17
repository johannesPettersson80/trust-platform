#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_DIR="${ROOT_DIR}/docs/demo"
PKG_DIR="${ROOT_DIR}/target/browser-analysis-wasm/pkg"
RUNTIME_UI_DIR="${ROOT_DIR}/crates/trust-runtime/src/web/ui"

echo "==> Building WASM analysis module..."
"${ROOT_DIR}/scripts/build_browser_analysis_wasm_spike.sh"

echo ""
echo "==> Copying assets to ${DEMO_DIR}/ ..."

mkdir -p "${DEMO_DIR}/wasm" "${DEMO_DIR}/assets"
touch "${DEMO_DIR}/.nojekyll"

cp "${PKG_DIR}/trust_wasm_analysis.js"      "${DEMO_DIR}/wasm/"
cp "${PKG_DIR}/trust_wasm_analysis_bg.wasm" "${DEMO_DIR}/wasm/"
cp "${RUNTIME_UI_DIR}/wasm/analysis-client.js" "${DEMO_DIR}/wasm/"
cp "${RUNTIME_UI_DIR}/assets/ide-monaco.20260215.js" "${DEMO_DIR}/assets/"

echo ""
echo "==> Asset sizes:"
du -sh "${DEMO_DIR}/wasm/trust_wasm_analysis_bg.wasm"
du -sh "${DEMO_DIR}/wasm/trust_wasm_analysis.js"
du -sh "${DEMO_DIR}/wasm/analysis-client.js"
du -sh "${DEMO_DIR}/wasm/worker.js"
du -sh "${DEMO_DIR}/assets/ide-monaco.20260215.js"
du -sh "${DEMO_DIR}/demo.js"
du -sh "${DEMO_DIR}/demo.css"
du -sh "${DEMO_DIR}/index.html"

TOTAL=$(du -sh "${DEMO_DIR}" | cut -f1)
echo ""
echo "==> Total demo size: ${TOTAL}"
echo ""
echo "Demo build complete."
echo "To serve locally:"
echo "  scripts/run_demo_local_replica.sh --port 4175 --repo trust-platform"
echo "or:"
echo "  python3 -m http.server 8000 -d ${DEMO_DIR}"
echo "Then open:"
echo "  http://127.0.0.1:4175/trust-platform/  (replica)"
echo "  http://127.0.0.1:8000/                (direct)"
