#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
CAPTURE_CACHE_DIR="$ROOT_DIR/scripts/captures/.cache"
CONTAINER_NAME="${TRUST_CAPTURE_CODESERVER_CONTAINER:-trust-docs-code-server}"
PORT="${TRUST_CAPTURE_CODESERVER_PORT:-8080}"
IMAGE="${TRUST_CAPTURE_CODESERVER_IMAGE:-codercom/code-server:4.116.0}"
WORKSPACE_PATH_IN_CONTAINER="/workspaces/trust-platform/manual-tests/trust-lsp-smoke.code-workspace"
VSIX_PATH="$CAPTURE_CACHE_DIR/trust-lsp.vsix"

if ! command -v docker >/dev/null 2>&1; then
  echo "Missing required command: docker" >&2
  exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "Missing required command: npm" >&2
  exit 1
fi

mkdir -p "$CAPTURE_CACHE_DIR"

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
  -lc "code-server --install-extension /workspaces/trust-platform/scripts/captures/.cache/trust-lsp.vsix --force >/tmp/trust-install.log 2>&1 || { cat /tmp/trust-install.log >&2; exit 1; }; exec code-server --bind-addr 0.0.0.0:8080 --auth none --disable-telemetry --disable-workspace-trust ${WORKSPACE_PATH_IN_CONTAINER}"
