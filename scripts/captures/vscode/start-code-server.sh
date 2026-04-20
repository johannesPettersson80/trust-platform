#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
CAPTURE_CACHE_DIR="$ROOT_DIR/scripts/captures/.cache"
CONTAINER_NAME="${TRUST_CAPTURE_CODESERVER_CONTAINER:-trust-docs-code-server}"
PORT="${TRUST_CAPTURE_CODESERVER_PORT:-8080}"
IMAGE="${TRUST_CAPTURE_CODESERVER_IMAGE:-codercom/code-server:4.116.0}"
WORKSPACE_PATH_IN_CONTAINER="/workspaces/trust-platform/manual-tests/trust-lsp-smoke.code-workspace"
VSIX_PATH="$CAPTURE_CACHE_DIR/trust-lsp.vsix"
SETTINGS_PATH="$CAPTURE_CACHE_DIR/code-server-settings.json"

if ! command -v docker >/dev/null 2>&1; then
  echo "Missing required command: docker" >&2
  exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "Missing required command: npm" >&2
  exit 1
fi

mkdir -p "$CAPTURE_CACHE_DIR"

cat >"$SETTINGS_PATH" <<'EOF'
{
  "workbench.colorTheme": "Default Dark Modern",
  "security.workspace.trust.enabled": false,
  "workbench.startupEditor": "none",
  "workbench.welcome.enabled": false,
  "workbench.tips.enabled": false,
  "window.commandCenter": false,
  "chat.commandCenter.enabled": false,
  "workbench.sideBar.location": "left",
  "workbench.activityBar.location": "left",
  "workbench.editor.showTabs": "multiple",
  "workbench.editor.enablePreview": false,
  "workbench.secondarySideBar.defaultVisibility": "hidden",
  "editor.minimap.enabled": false,
  "editor.wordWrap": "off",
  "breadcrumbs.enabled": false,
  "problems.showCurrentInStatus": false,
  "workbench.statusBar.visible": true,
  "workbench.panel.defaultLocation": "bottom"
}
EOF

docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true

VSCE_LOG="/tmp/trust-doc-captures-vsce.log"
if ! (
  cd "$ROOT_DIR/editors/vscode"
  npx --yes @vscode/vsce@2.27.0 package --skip-license --out "$VSIX_PATH"
) >"$VSCE_LOG" 2>&1; then
  cat "$VSCE_LOG" >&2
  exit 1
fi

exec docker run \
  --rm \
  --name "$CONTAINER_NAME" \
  --entrypoint sh \
  -p "127.0.0.1:${PORT}:8080" \
  -v "$ROOT_DIR:/workspaces/trust-platform" \
  "$IMAGE" \
  -lc "USER_DATA_DIR=/tmp/trust-docs-code-server-user-data; EXTENSIONS_DIR=/tmp/trust-docs-code-server-extensions; mkdir -p \"\${USER_DATA_DIR}/User\" \"\${EXTENSIONS_DIR}\"; cp /workspaces/trust-platform/scripts/captures/.cache/code-server-settings.json \"\${USER_DATA_DIR}/User/settings.json\"; code-server --install-extension /workspaces/trust-platform/scripts/captures/.cache/trust-lsp.vsix --force --user-data-dir \"\${USER_DATA_DIR}\" --extensions-dir \"\${EXTENSIONS_DIR}\" >/tmp/trust-install.log 2>&1 || { cat /tmp/trust-install.log >&2; exit 1; }; exec code-server --bind-addr 0.0.0.0:8080 --auth none --disable-telemetry --disable-workspace-trust --user-data-dir \"\${USER_DATA_DIR}\" --extensions-dir \"\${EXTENSIONS_DIR}\" ${WORKSPACE_PATH_IN_CONTAINER}"
