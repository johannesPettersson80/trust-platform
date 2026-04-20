#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ASSET_DIR="$ROOT_DIR/editors/vscode/assets"
EXTENSION_DEV_PATH="$ROOT_DIR/editors/vscode"
USER_DATA_DIR="/tmp/trust-public-docs-vscode-user-data"
EXTENSIONS_DIR="$USER_DATA_DIR/extensions"
DISPLAY_OUTPUT="${TRUST_SCREEN_OUTPUT:-}"
WINDOW_SETTLE_SECS="${TRUST_WINDOW_SETTLE_SECS:-8}"
DO_BUILD_EXTENSION=1

usage() {
  cat <<'EOF'
Capture public-doc visual-editor screenshots automatically.

Usage:
  scripts/capture-public-docs-visual-editors.sh [--assets-dir <path>] [--extension-dev-path <path>] [--output <display-name>] [--no-build-extension]
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --assets-dir)
      ASSET_DIR="${2:-}"
      shift 2
      ;;
    --extension-dev-path)
      EXTENSION_DEV_PATH="${2:-}"
      shift 2
      ;;
    --output)
      DISPLAY_OUTPUT="${2:-}"
      shift 2
      ;;
    --no-build-extension)
      DO_BUILD_EXTENSION=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ "$ASSET_DIR" != /* ]]; then
  ASSET_DIR="$ROOT_DIR/$ASSET_DIR"
fi

if [[ "$EXTENSION_DEV_PATH" != /* ]]; then
  EXTENSION_DEV_PATH="$ROOT_DIR/$EXTENSION_DEV_PATH"
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd code
require_cmd grim
require_cmd wlr-randr
require_cmd wlrctl
require_cmd npm

if [[ -z "$DISPLAY_OUTPUT" ]]; then
  DISPLAY_OUTPUT=$(wlr-randr | awk 'NR==1 {print $1; exit}')
fi

if [[ -z "$DISPLAY_OUTPUT" ]]; then
  echo "Could not detect a Wayland output for grim capture." >&2
  exit 1
fi

mkdir -p "$ASSET_DIR"

prepare_profile() {
  rm -rf "$USER_DATA_DIR"
  mkdir -p "$USER_DATA_DIR/User" "$EXTENSIONS_DIR"
  cat >"$USER_DATA_DIR/User/settings.json" <<'JSON'
{
  "security.workspace.trust.enabled": false,
  "workbench.startupEditor": "none",
  "workbench.welcome.enabled": false,
  "workbench.tips.enabled": false,
  "window.commandCenter": false,
  "chat.commandCenter.enabled": false,
  "workbench.editor.enablePreview": false,
  "trust-lsp.visual.autoOpenCustomEditors": true
}
JSON
}

build_extension() {
  if (( DO_BUILD_EXTENSION == 1 )); then
    npm --prefix "$EXTENSION_DEV_PATH" run compile >/tmp/trust-public-docs-visual-build.log 2>&1
  fi
}

wait_for_code_window() {
  for _ in $(seq 1 60); do
    if wlrctl toplevel find app_id:code >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "Timed out waiting for VS Code window." >&2
  exit 1
}

focus_and_fullscreen_code() {
  wlrctl toplevel focus app_id:code || true
  sleep 0.3
  wlrctl toplevel fullscreen app_id:code || true
  sleep 1
}

launch_code() {
  local file_path="$1"
  wlrctl toplevel close app_id:code >/dev/null 2>&1 || true
  sleep 1
  code --new-window \
    --user-data-dir "$USER_DATA_DIR" \
    --extensions-dir "$EXTENSIONS_DIR" \
    --extensionDevelopmentPath "$EXTENSION_DEV_PATH" \
    "$ROOT_DIR" \
    -g "$file_path" >/tmp/trust-public-docs-visual-code.log 2>&1 &
  wait_for_code_window
  sleep "$WINDOW_SETTLE_SECS"
  focus_and_fullscreen_code
}

capture() {
  local path="$1"
  grim -o "$DISPLAY_OUTPUT" "$path"
  echo "Captured $path"
}

close_window() {
  wlrctl toplevel close app_id:code >/dev/null 2>&1 || true
  sleep 1
}

capture_editor() {
  local file_path="$1"
  local out="$2"
  launch_code "$file_path"
  capture "$out"
  close_window
}

echo "Using display output: $DISPLAY_OUTPUT"
prepare_profile
build_extension
capture_editor "$ROOT_DIR/examples/ladder/simple-start-stop.ladder.json:1" "$ASSET_DIR/screenshot-ladder-editor.png"
capture_editor "$ROOT_DIR/examples/statecharts/traffic-light.statechart.json:1" "$ASSET_DIR/screenshot-statechart-editor.png"
capture_editor "$ROOT_DIR/examples/blockly/snake-simple-v2.blockly.json:1" "$ASSET_DIR/screenshot-blockly-editor.png"
capture_editor "$ROOT_DIR/examples/sfc/ethercat-snake-simple.sfc.json:1" "$ASSET_DIR/screenshot-sfc-editor.png"

echo "Done. Visual-editor screenshots written to $ASSET_DIR"
