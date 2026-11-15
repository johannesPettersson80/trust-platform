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

# The captured workspace names its language server as target/debug/trust-lsp (its trust-lsp.server.path),
# which the container sees as the build that starts in the code-server image, a Debian stable system
# (build-container-programs.sh; specification 22, "Docs Captures"). A language server that does not
# start leaves every view that needs it empty, so it starts once in the image first, and the
# captures stop when it does not.
programs=$("$ROOT_DIR/scripts/captures/build-container-programs.sh")
# The folder the build is mounted over, made here so that Docker does not make it as root.
mkdir -p "$ROOT_DIR/target/debug"
if ! docker run --rm \
  -v "$programs:/workspaces/trust-platform/target/debug:ro" \
  --entrypoint /workspaces/trust-platform/target/debug/trust-lsp \
  "$IMAGE" \
  command --help >/dev/null; then
  echo "trust-lsp does not start in $IMAGE" >&2
  exit 1
fi

# The user settings name the language server, the runtime, the debug adapter and the test runner of
# that build for every folder a capture opens, not only for the smoke workspace, which names its own.
# The pictures show truST, so the image's own AI features are off: its chat, its status bar item and
# the AI light bulb in the editor.
cat >"$SETTINGS_PATH" <<'EOF'
{
  "trust.languageServer.executablePath": "/workspaces/trust-platform/target/debug/trust-lsp",
  "trust.runtime.executablePath": "/workspaces/trust-platform/target/debug/trust-runtime",
  "trust.debugAdapter.executablePath": "/workspaces/trust-platform/target/debug/trust-debug",
  "trust.testRunner.executablePath": "/workspaces/trust-platform/target/debug/trust-dev",
  "chat.disableAIFeatures": true,
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
  npx --no-install vsce package --skip-license --out "$VSIX_PATH"
) >"$VSCE_LOG" 2>&1; then
  cat "$VSCE_LOG" >&2
  exit 1
fi

# The captures that compile or run a program need the toolchain of specification 44 §4.6, as the
# terminal captures do (run-terminal-captures.sh): the container sees the folder TRUST_TOOLCHAIN names,
# and the Rust toolchain its compiler and linker link into, at the same paths. Without TRUST_TOOLCHAIN
# the list stays empty, and it is expanded only when it has elements (Bash 3.2 calls an empty array
# unbound under `set -u`).
toolchain_args=()
if [[ -n "${TRUST_TOOLCHAIN:-}" ]]; then
  sysroot=$(rustc --print sysroot)
  toolchain_args=(
    -e "TRUST_TOOLCHAIN=$TRUST_TOOLCHAIN"
    -v "$TRUST_TOOLCHAIN:$TRUST_TOOLCHAIN:ro"
    -v "$sysroot:$sysroot:ro"
  )
fi

# code-server runs without its update check, whose notice of a newer release would cover the corner of
# a picture. It runs as this computer's user, as the terminal captures' container does, so that what it
# writes into the repository, such as the project that Create Project makes, belongs to that user: as
# the image's own user it could write nothing into a checkout that another user owns, as the runner's
# is. Its home is in the container's /tmp, where that user can write.
exec docker run \
  --rm \
  --name "$CONTAINER_NAME" \
  --entrypoint sh \
  -u "$(id -u):$(id -g)" \
  -e HOME=/tmp/trust-docs-code-server-home \
  -p "127.0.0.1:${PORT}:8080" \
  -v "$ROOT_DIR:/workspaces/trust-platform" \
  -v "$programs:/workspaces/trust-platform/target/debug:ro" \
  ${toolchain_args[@]+"${toolchain_args[@]}"} \
  "$IMAGE" \
  -lc "USER_DATA_DIR=/tmp/trust-docs-code-server-user-data; EXTENSIONS_DIR=/tmp/trust-docs-code-server-extensions; mkdir -p \"\${HOME}\" \"\${USER_DATA_DIR}/User\" \"\${EXTENSIONS_DIR}\"; cp /workspaces/trust-platform/scripts/captures/.cache/code-server-settings.json \"\${USER_DATA_DIR}/User/settings.json\"; code-server --install-extension /workspaces/trust-platform/scripts/captures/.cache/trust-lsp.vsix --force --user-data-dir \"\${USER_DATA_DIR}\" --extensions-dir \"\${EXTENSIONS_DIR}\" >/tmp/trust-install.log 2>&1 || { cat /tmp/trust-install.log >&2; exit 1; }; exec code-server --bind-addr 0.0.0.0:8080 --auth none --disable-telemetry --disable-update-check --disable-workspace-trust --user-data-dir \"\${USER_DATA_DIR}\" --extensions-dir \"\${EXTENSIONS_DIR}\" ${WORKSPACE_PATH_IN_CONTAINER}"
