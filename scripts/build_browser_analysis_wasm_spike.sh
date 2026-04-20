#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${ROOT_DIR}/target/browser-analysis-wasm"
PKG_DIR="${OUT_DIR}/pkg"
WEB_SRC="${ROOT_DIR}/docs/internal/prototypes/browser_analysis_wasm_spike/web"
WEB_OUT="${OUT_DIR}/web"
RUNTIME_UI_DIR="${ROOT_DIR}/crates/trust-runtime/src/web/ui"
RUNTIME_ASSETS_DIR="${RUNTIME_UI_DIR}/assets"
RUNTIME_BASE_CSS_DIR="${RUNTIME_UI_DIR}/chunks/base-css"
RUNTIME_IDE_CSS_DIR="${RUNTIME_UI_DIR}/chunks/ide-css"

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "error: wasm-pack is required."
  echo "install: cargo install wasm-pack"
  exit 1
fi

if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown >/dev/null
fi

rm -rf "${PKG_DIR}" "${WEB_OUT}"
mkdir -p "${PKG_DIR}" "${WEB_OUT}"

(
  cd "${ROOT_DIR}/crates/trust-wasm-analysis"
  wasm-pack build \
    --target web \
    --out-dir "${PKG_DIR}" \
    --out-name trust_wasm_analysis \
    -- \
    --features wasm
)

cp -R "${WEB_SRC}/." "${WEB_OUT}/"
cp "${RUNTIME_ASSETS_DIR}/favicon.svg" "${WEB_OUT}/favicon.svg"
cp "${RUNTIME_ASSETS_DIR}/logo.svg" "${WEB_OUT}/logo.svg"

RUNTIME_STYLES_PATH="${WEB_OUT}/runtime-styles.css"
: > "${RUNTIME_STYLES_PATH}"
for css_file in "${RUNTIME_BASE_CSS_DIR}"/*.css "${RUNTIME_IDE_CSS_DIR}"/*.css; do
  if [[ -f "${css_file}" ]]; then
    cat "${css_file}" >> "${RUNTIME_STYLES_PATH}"
    printf '\n' >> "${RUNTIME_STYLES_PATH}"
  fi
done

cat <<EOF
WASM browser analysis spike build complete.
- Package: ${PKG_DIR}
- Host: ${WEB_OUT}

To run locally:
  python3 -m http.server 4173 --directory "${OUT_DIR}"
Then open:
  http://127.0.0.1:4173/web/
OpenPLC shell integration demo:
  http://127.0.0.1:4173/web/openplc-shell.html
EOF
